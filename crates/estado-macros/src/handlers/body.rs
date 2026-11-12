//! Scanning and rewriting the body of a handler (spec 6.3): every transition
//! macro becomes the corresponding `Next` expression, `goto!` becomes a
//! variant of the handler's edge enum, whatever the nesting (closures,
//! blocks, match arms).

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::visit_mut::{self, VisitMut};
use syn::{Arm, Block, Expr, ExprMacro, Ident, Macro, Pat, Stmt, parse_quote};

use super::captures::captured_binding;
use super::goto_resolve::{ResolvedPayload, resolve_goto};
use super::goto_syntax::parse_goto;
use crate::construct::exiting_type;
use crate::model::{
    Diagnostic, Extent, GotoForm, GotoOccurrence, HandlerRole, HandlerUses, StateId, StateTree,
};
use crate::naming::{facade_name, plain_name};

/// How the body is rewritten.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BodyMode {
    /// Every `goto!` becomes an edge variant; the machine is error-free.
    Full,
    /// The machine has errors and no edge enums exist: every `goto!` becomes
    /// `unreachable!()` so the rest of the code still type-checks.
    Degraded,
}

/// What the rewrite needs to know.
pub struct BodyEnvironment<'model> {
    /// The tree.
    pub tree: &'model StateTree,
    /// The state of the handler.
    pub state: StateId,
    /// The role of the function.
    pub role: HandlerRole,
    /// The name of the handler's edge enum.
    pub edges_enum: &'model Ident,
    /// The mode.
    pub mode: BodyMode,
    /// What the handler borrows by name: a constructor may not use it.
    pub borrowed_names: &'model [Ident],
}

/// What the scan found.
pub struct BodyReport {
    /// The `goto!`s, in source order.
    pub gotos: Vec<GotoOccurrence>,
    /// Features used.
    pub uses: HandlerUses,
    /// Problems.
    pub diagnostics: Vec<Diagnostic>,
}

/// Rewrites `body` in place and reports what it found.
pub fn rewrite_body(body: &mut Block, environment: &BodyEnvironment<'_>) -> BodyReport {
    let mut rewriter = BodyRewriter {
        environment,
        report: BodyReport {
            gotos: Vec::new(),
            uses: HandlerUses::default(),
            diagnostics: Vec::new(),
        },
        hints: Vec::new(),
    };
    rewriter.visit_block_mut(body);
    rewriter.report
}

#[derive(Clone, Copy)]
enum TransitionMacro {
    Goto,
    Stay,
    Bubble,
    Unhandled,
    Defer,
    Reenter,
}

fn transition_macro(mac: &Macro) -> Option<TransitionMacro> {
    let name = facade_name(&mac.path)?;
    Some(match name.to_string().as_str() {
        "goto" => TransitionMacro::Goto,
        "stay" => TransitionMacro::Stay,
        "bubble" => TransitionMacro::Bubble,
        "unhandled" => TransitionMacro::Unhandled,
        "defer" => TransitionMacro::Defer,
        "reenter" => TransitionMacro::Reenter,
        _ => return None,
    })
}

struct BodyRewriter<'environment, 'model> {
    environment: &'environment BodyEnvironment<'model>,
    report: BodyReport,
    /// Event patterns of the enclosing `match` arms, innermost last.
    hints: Vec<Option<String>>,
}

impl BodyRewriter<'_, '_> {
    fn current_hint(&self) -> Option<String> {
        self.hints.iter().rev().find_map(Clone::clone)
    }

    fn fail(&mut self, diagnostic: Diagnostic) -> Expr {
        self.report.diagnostics.push(diagnostic);
        parse_quote!(::core::unreachable!())
    }

    fn replace(&mut self, kind: TransitionMacro, mac: &Macro) -> Expr {
        let environment = self.environment;
        if matches!(environment.role, HandlerRole::Entry | HandlerRole::Exit) {
            let diagnostic = Diagnostic::new(
                "E0042",
                Extent::of(mac),
                "entries and exits cannot transition or return a `Next`",
            )
            .with_help("do the work here and let a handler decide the transition (spec 5.3)");
            return self.fail(diagnostic);
        }
        let reacts_to_the_state =
            matches!(environment.role, HandlerRole::Always | HandlerRole::OnDone);
        if reacts_to_the_state && !matches!(kind, TransitionMacro::Stay | TransitionMacro::Goto) {
            let name = mac
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .unwrap_or_default();
            let diagnostic = Diagnostic::new(
                "E0045",
                Extent::of(mac),
                format!("`{name}!` has no meaning in `#[always]` and `#[on_done]`"),
            )
            .with_help(
                "these handlers react to their state, not to an event: only `stay!()` and `goto!` are allowed",
            );
            return self.fail(diagnostic);
        }
        match kind {
            TransitionMacro::Stay => parse_quote!(__estado_core::Next::Stay),
            TransitionMacro::Bubble => parse_quote!(__estado_core::Next::Bubble),
            TransitionMacro::Unhandled => parse_quote!(__estado_core::Next::Unhandled),
            TransitionMacro::Defer => {
                self.report.uses.defer = true;
                parse_quote!(__estado_core::Next::Defer)
            }
            TransitionMacro::Reenter => {
                self.report.uses.reenter = true;
                parse_quote!(__estado_core::Next::Reenter)
            }
            TransitionMacro::Goto => self.replace_goto(mac),
        }
    }

    fn replace_goto(&mut self, mac: &Macro) -> Expr {
        let environment = self.environment;
        if environment.mode == BodyMode::Degraded {
            return parse_quote!(::core::unreachable!());
        }
        let tree = environment.tree;
        let macro_span = Extent::of(mac);
        let syntax = match parse_goto(mac.tokens.clone(), macro_span) {
            Ok(syntax) => syntax,
            Err(diagnostic) => return self.fail(diagnostic),
        };
        let exiting = |node: StateId| exiting_type(tree, node);
        let resolved = match resolve_goto(tree, environment.state, syntax, macro_span, &exiting) {
            Ok(resolved) => resolved,
            Err(diagnostic) => return self.fail(diagnostic),
        };
        if tree.node(environment.state).is_final {
            let diagnostic = Diagnostic::new(
                "E0022",
                macro_span,
                format!(
                    "`{}` is a terminal state and cannot transition out",
                    tree.node(environment.state).name
                ),
            )
            .with_help("remove the `goto!`, or remove `#[terminal]` from the state");
            return self.fail(diagnostic);
        }

        if let ResolvedPayload::Constructor { closure, .. } = &resolved.payload {
            if let Some(captured) = captured_binding(closure, environment.borrowed_names) {
                let diagnostic = Diagnostic::new(
                    "E0046",
                    captured.span(),
                    format!(
                        "the constructor of this `goto!` uses `{captured}`, which the handler borrows"
                    ),
                )
                .with_help(
                    "the constructor runs after the handler returned: copy what it needs \
                     into a local before the `goto!` and use a `move` closure",
                )
                .with_note(macro_span, "the constructor form of this `goto!`");
                return self.fail(diagnostic);
            }
        }

        let target_name = tree.node(resolved.target).name.clone();
        let used: Vec<String> = self
            .report
            .gotos
            .iter()
            .map(|occurrence| occurrence.variant.to_string())
            .collect();
        let variant = Ident::new(&variant_name(tree, &target_name, &used), target_name.span());
        let edges = environment.edges_enum;
        let argument: TokenStream = match &resolved.payload {
            ResolvedPayload::Value(value) => quote! { #value },
            ResolvedPayload::Constructor { closure, .. } => quote! { #closure },
        };
        let form = match resolved.payload {
            ResolvedPayload::Value(_) => GotoForm::Value,
            ResolvedPayload::Constructor { exiting, .. } => GotoForm::Constructor { exiting },
        };
        self.report.gotos.push(GotoOccurrence {
            variant: variant.clone(),
            target: resolved.target,
            lca: resolved.lca,
            form,
            kind: resolved.kind,
            compares_leaf: tree.is_ancestor_or_self(resolved.target, environment.state),
            event_hint: self.current_hint(),
        });
        parse_quote!(__estado_core::Next::Goto(__estado::#edges::#variant(#argument)))
    }

    fn macro_expression(&mut self, attrs: Vec<syn::Attribute>, mac: &Macro) -> Option<Expr> {
        let kind = transition_macro(mac)?;
        if !matches!(kind, TransitionMacro::Goto) && !mac.tokens.is_empty() {
            let name = mac
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .unwrap_or_default();
            let diagnostic = Diagnostic::new(
                "E0043",
                Extent::of(mac),
                format!("`{name}!` takes no arguments"),
            );
            return Some(self.fail(diagnostic));
        }
        let mut replacement = self.replace(kind, mac);
        // Data the user wrote inside the macro may hold further macros.
        self.visit_expr_mut(&mut replacement);
        if let Expr::Call(call) = &mut replacement {
            call.attrs.extend(attrs);
        }
        Some(replacement)
    }
}

impl VisitMut for BodyRewriter<'_, '_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Macro(ExprMacro { attrs, mac }) = expression {
            if let Some(replacement) = self.macro_expression(std::mem::take(attrs), mac) {
                *expression = replacement;
                return;
            }
        }
        visit_mut::visit_expr_mut(self, expression);
    }

    fn visit_stmt_mut(&mut self, statement: &mut Stmt) {
        if let Stmt::Macro(statement_macro) = statement {
            if transition_macro(&statement_macro.mac).is_some() {
                let expression = Expr::Macro(ExprMacro {
                    attrs: statement_macro.attrs.clone(),
                    mac: statement_macro.mac.clone(),
                });
                *statement = Stmt::Expr(expression, statement_macro.semi_token);
            }
        }
        visit_mut::visit_stmt_mut(self, statement);
    }

    fn visit_expr_method_call_mut(&mut self, call: &mut syn::ExprMethodCall) {
        if call.method == "raise" {
            self.report.uses.raise = true;
        } else if call.method == "in_state" {
            self.report.uses.in_state = true;
        }
        visit_mut::visit_expr_method_call_mut(self, call);
    }

    fn visit_arm_mut(&mut self, arm: &mut Arm) {
        self.hints.push(pattern_hint(&arm.pat));
        visit_mut::visit_arm_mut(self, arm);
        self.hints.pop();
    }
}

/// The edge variant of a `goto!` to `target` given the variants the handler
/// already has: `ToFoo` for the first one, then `ToFoo2`, `ToFoo3`, ... A
/// numbered name never equals the plain name another state would get
/// (`ToFoo2` for a state called `Foo2`): that number is skipped (ADR 0013
/// item 10).
fn variant_name(tree: &StateTree, target: &Ident, used: &[String]) -> String {
    let plain = format!("To{}", plain_name(target));
    if !used.contains(&plain) {
        return plain;
    }
    let state_variants: HashSet<String> = tree
        .nodes
        .iter()
        .skip(1)
        .map(|node| format!("To{}", plain_name(&node.name)))
        .collect();
    (2..)
        .map(|occurrence| format!("{plain}{occurrence}"))
        .find(|candidate| !state_variants.contains(candidate) && !used.contains(candidate))
        .expect("an unused number exists")
}

fn path_text(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// The event a match arm is about, e.g. `Event::Play` for
/// `Event::Play(track)` (spec 8.1, `event_hint`).
fn pattern_hint(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::TupleStruct(inner) => Some(path_text(&inner.path)),
        Pat::Struct(inner) => Some(path_text(&inner.path)),
        Pat::Path(inner) => Some(path_text(&inner.path)),
        Pat::Reference(inner) => pattern_hint(&inner.pat),
        Pat::Paren(inner) => pattern_hint(&inner.pat),
        Pat::Or(inner) => {
            let parts: Option<Vec<String>> = inner.cases.iter().map(pattern_hint).collect();
            parts.map(|parts| parts.join(" | "))
        }
        _ => None,
    }
}
