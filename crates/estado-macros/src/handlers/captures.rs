//! A constructor closure runs after its handler returned (spec 5.3 step 5),
//! so it cannot use what the handler borrows: the data of its state, the
//! `*Fields` of its ancestors. Rust reports that as an opaque-type lifetime
//! error (E0700) whose advice cannot be followed; estado reports it as E0046
//! with the fix instead (ADR 0013 item 11).

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{ExprClosure, ExprPath, Ident, Macro, Pat};

/// The first identifier of `names` that `closure` uses, if any. Names the
/// closure binds itself (parameters) are not the handler's.
pub fn captured_binding(closure: &ExprClosure, names: &[Ident]) -> Option<Ident> {
    let mut bound = Vec::new();
    for input in &closure.inputs {
        collect_bound(input, &mut bound);
    }
    let mut finder = Finder {
        names: names.iter().filter(|name| !bound.contains(name)).collect(),
        found: None,
    };
    finder.visit_expr(&closure.body);
    finder.found
}

fn collect_bound(pattern: &Pat, bound: &mut Vec<Ident>) {
    match pattern {
        Pat::Ident(inner) => bound.push(inner.ident.clone()),
        Pat::Type(inner) => collect_bound(&inner.pat, bound),
        Pat::Reference(inner) => collect_bound(&inner.pat, bound),
        Pat::Tuple(inner) => inner.elems.iter().for_each(|pat| collect_bound(pat, bound)),
        Pat::TupleStruct(inner) => inner.elems.iter().for_each(|pat| collect_bound(pat, bound)),
        Pat::Struct(inner) => inner
            .fields
            .iter()
            .for_each(|field| collect_bound(&field.pat, bound)),
        _ => {}
    }
}

struct Finder<'names> {
    names: Vec<&'names Ident>,
    found: Option<Ident>,
}

impl Finder<'_> {
    fn note(&mut self, ident: &Ident) {
        if self.found.is_none() && self.names.contains(&ident) {
            self.found = Some(ident.clone());
        }
    }

    fn scan_tokens(&mut self, tokens: TokenStream) {
        for tree in tokens {
            match tree {
                TokenTree::Ident(ident) => self.note(&ident),
                TokenTree::Group(group) => self.scan_tokens(group.stream()),
                _ => {}
            }
        }
    }
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
}
