//! A constructor closure runs after its handler returned (spec 5.3 step 5),
//! so it cannot use what the handler borrows: the data of its state, the
//! `*Fields` of its ancestors. Rust reports that as an opaque-type lifetime
//! error (E0700) whose advice cannot be followed; estado reports it as E0046
//! with the fix instead (ADR 0013 item 11).
//!
//! The check is syntactic but scope-aware: a name that a `let`, a closure
//! parameter, a match arm or an `if let` binds again (in the closure, or in
//! the handler before the `goto!`) is a different value (ADR 0013 amendment
//! 15).

use proc_macro2::{Spacing, TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{
    Arm, Block, ExprClosure, ExprForLoop, ExprIf, ExprLet, ExprPath, ExprWhile, Ident, Local, Macro,
};

use super::scopes::Scopes;

/// The first identifier of `names` that `closure` uses, if any. A name the
/// closure binds itself (a parameter, a `let`, a match arm) is not the
/// handler's; neither is one the caller already knows to be shadowed.
pub fn captured_binding(closure: &ExprClosure, names: &[&Ident]) -> Option<Ident> {
    let mut finder = Finder {
        names: names.to_vec(),
        scopes: Scopes::new(),
        found: None,
    };
    finder.visit_expr_closure(closure);
    finder.found
}

struct Finder<'names> {
    names: Vec<&'names Ident>,
    scopes: Scopes,
    found: Option<Ident>,
}

impl Finder<'_> {
    fn note(&mut self, ident: &Ident) {
        if self.found.is_none() && self.names.contains(&ident) && !self.scopes.shadows(ident) {
            self.found = Some(ident.clone());
        }
    }

    /// The tokens of a macro call: identifiers that follow a `.` (a field)
    /// or precede a single `:` or `=` (a field initialiser, a named
    /// argument) are not uses of a binding; a string literal may name one
    /// inline (`"{name}"`) unless a named argument of the call takes the name.
    fn scan_tokens(&mut self, tokens: TokenStream) {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let named: Vec<Ident> = trees
            .windows(2)
            .filter_map(|pair| match (&pair[0], &pair[1]) {
                (TokenTree::Ident(ident), next) if is_single(next, '=') => Some(ident.clone()),
                _ => None,
            })
            .collect();
        let mut after_dot = false;
        for (position, tree) in trees.iter().enumerate() {
            let mut is_dot = false;
            match tree {
                TokenTree::Ident(ident) => {
                    let next = trees.get(position + 1);
                    let names_a_field = after_dot
                        || next.is_some_and(|next| is_single(next, ':') || is_single(next, '='));
                    if !names_a_field {
                        self.note(ident);
                    }
                }
                TokenTree::Group(group) => self.scan_tokens(group.stream()),
                TokenTree::Literal(literal) => {
                    self.scan_inline_arguments(&literal.to_string(), &named);
                }
                TokenTree::Punct(punct) => is_dot = punct.as_char() == '.',
            }
            after_dot = is_dot;
        }
    }

    fn scan_inline_arguments(&mut self, literal: &str, named: &[Ident]) {
        if !literal.starts_with('"') {
            return;
        }
        let text = literal.replace("{{", "").replace("}}", "");
        let candidates: Vec<Ident> = self
            .names
            .iter()
            .filter(|name| !named.contains(name))
            .map(|name| (*name).clone())
            .collect();
        for name in candidates {
            let plain = format!("{{{name}}}");
            let formatted = format!("{{{name}:");
            if text.contains(&plain) || text.contains(&formatted) {
                self.note(&name);
            }
        }
    }
}

/// Whether `tree` is the punctuation `symbol` on its own (not the start of
/// `::`, `==`, `=>`, ...).
fn is_single(tree: &TokenTree, symbol: char) -> bool {
    matches!(tree, TokenTree::Punct(punct)
        if punct.as_char() == symbol && punct.spacing() == Spacing::Alone)
}

impl<'ast> Visit<'ast> for Finder<'_> {
    fn visit_expr_path(&mut self, path: &'ast ExprPath) {
        if path.qself.is_none() {
            if let Some(ident) = path.path.get_ident() {
                self.note(ident);
            }
        }
        visit::visit_expr_path(self, path);
    }

    /// `format!("{}", state.x)` and friends: the arguments are tokens.
    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.scan_tokens(mac.tokens.clone());
    }

    fn visit_block(&mut self, block: &'ast Block) {
        self.scopes.push();
        visit::visit_block(self, block);
        self.scopes.pop();
    }

    /// The initialiser sees the name as it was before the `let`.
    fn visit_local(&mut self, local: &'ast Local) {
        if let Some(initializer) = &local.init {
            self.visit_expr(&initializer.expr);
            if let Some((_, diverging)) = &initializer.diverge {
                self.visit_expr(diverging);
            }
        }
        self.scopes.bind(&local.pat);
    }

    fn visit_expr_closure(&mut self, closure: &'ast ExprClosure) {
        self.scopes.push();
        for input in &closure.inputs {
            self.scopes.bind(input);
        }
        self.visit_expr(&closure.body);
        self.scopes.pop();
    }

    fn visit_arm(&mut self, arm: &'ast Arm) {
        self.scopes.push();
        self.scopes.bind(&arm.pat);
        if let Some((_, guard)) = &arm.guard {
            self.visit_expr(guard);
        }
        self.visit_expr(&arm.body);
        self.scopes.pop();
    }

    fn visit_expr_if(&mut self, expression: &'ast ExprIf) {
        self.scopes.push();
        self.visit_expr(&expression.cond);
        self.visit_block(&expression.then_branch);
        self.scopes.pop();
        if let Some((_, otherwise)) = &expression.else_branch {
            self.visit_expr(otherwise);
        }
    }

    fn visit_expr_while(&mut self, expression: &'ast ExprWhile) {
        self.scopes.push();
        self.visit_expr(&expression.cond);
        self.visit_block(&expression.body);
        self.scopes.pop();
    }

    fn visit_expr_for_loop(&mut self, expression: &'ast ExprForLoop) {
        self.visit_expr(&expression.expr);
        self.scopes.push();
        self.scopes.bind(&expression.pat);
        self.visit_block(&expression.body);
        self.scopes.pop();
    }

    /// `if let PATTERN = scrutinee`: the pattern binds for the rest of the
    /// condition and the block (the scope the `if` opened).
    fn visit_expr_let(&mut self, expression: &'ast ExprLet) {
        self.visit_expr(&expression.expr);
        self.scopes.bind(&expression.pat);
    }
}
