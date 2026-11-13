//! `goto_dyn!` and `edges!` are exported by the facade but not supported yet.
//!
//! Left alone inside a handler they would expand to the facade stubs and
//! report E0040, "only valid inside a `#[handlers]` function", which is
//! wrong: they are inside one. They are reported here as E0090 instead and
//! replaced by neutral code, so no second error follows.
//!
//! The same goes for a transition macro (`goto!`, `stay!`, ..) in a plain
//! helper function of the impl, one without a role attribute. `#[machine]`
//! rewrites a transition against the state of its handler, which a helper does
//! not have, so it cannot be rewritten; left alone it would also report E0040.

use syn::visit_mut::{self, VisitMut};
use syn::{Expr, ExprMacro, ImplItemFn, Macro, Stmt, Type, parse_quote};

use crate::model::{Diagnostic, Extent};
use crate::naming::facade_name;

/// Reports every `goto_dyn!` and `edges!` in `function` (signature and body)
/// as E0090 and replaces it: an expression by `unreachable!()`, a type by
/// `NoEdge`.
pub fn reject_unsupported_macros(function: &mut ImplItemFn, diagnostics: &mut Vec<Diagnostic>) {
    reject(function, diagnostics, MacroSet::Unsupported);
}

/// Reports every transition macro (`goto!`, `stay!`, `bubble!`, `unhandled!`,
/// `defer!`, `reenter!`) in a plain helper function as E0090 and replaces it
/// like [`reject_unsupported_macros`]. Call it on functions without a role.
pub fn reject_transitions_in_helper(function: &mut ImplItemFn, diagnostics: &mut Vec<Diagnostic>) {
    reject(function, diagnostics, MacroSet::Transitions);
}

fn reject(function: &mut ImplItemFn, diagnostics: &mut Vec<Diagnostic>, set: MacroSet) {
    let mut rejector = Rejector { diagnostics, set };
    rejector.visit_signature_mut(&mut function.sig);
    rejector.visit_block_mut(&mut function.block);
}

/// Which macros a pass rejects.
#[derive(Clone, Copy)]
enum MacroSet {
    /// `goto_dyn!` and `edges!`.
    Unsupported,
    /// The transition macros a handler body is rewritten around.
    Transitions,
}

struct Rejector<'diagnostics> {
    diagnostics: &'diagnostics mut Vec<Diagnostic>,
    set: MacroSet,
}

impl MacroSet {
    fn contains(self, name: &str) -> bool {
        match self {
            Self::Unsupported => matches!(name, "goto_dyn" | "edges"),
            Self::Transitions => matches!(
                name,
                "goto" | "stay" | "bubble" | "unhandled" | "defer" | "reenter"
            ),
        }
    }
}

impl Rejector<'_> {
    /// The name of `mac` when this pass rejects it.
    fn unsupported_name(&self, mac: &Macro) -> Option<String> {
        let name = facade_name(&mac.path)?.to_string();
        self.set.contains(&name).then_some(name)
    }

    fn report(&mut self, name: &str, mac: &Macro) {
        if matches!(self.set, MacroSet::Transitions) {
            self.diagnostics.push(
                Diagnostic::new(
                    "E0090",
                    Extent::of(mac),
                    format!("`{name}!` in a helper function is not supported yet by this version of `estado`"),
                )
                .with_help(
                    "a transition is rewritten against the state of its handler, and a helper \
                     has none: put the macro in the `#[on(..)]` handler and return data \
                     (an enum or a `bool`) from the helper",
                ),
            );
            return;
        }
        let help = if name == "goto_dyn" {
            "it is part of the planned macro language; use `goto!` with a state of the machine"
        } else {
            "it belongs to `#[edges]` helpers, which are not supported yet; \
             return `Next<Self>` from a handler"
        };
        self.diagnostics.push(
            Diagnostic::new(
                "E0090",
                Extent::of(mac),
                format!("`{name}!` is not supported yet by this version of `estado`"),
            )
            .with_help(help),
        );
    }
}

impl VisitMut for Rejector<'_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Macro(ExprMacro { mac, .. }) = expression {
            if let Some(name) = self.unsupported_name(mac) {
                self.report(&name, mac);
                *expression = parse_quote!(::core::unreachable!());
                return;
            }
        }
        visit_mut::visit_expr_mut(self, expression);
    }

    fn visit_stmt_mut(&mut self, statement: &mut Stmt) {
        if let Stmt::Macro(statement_macro) = statement {
            if let Some(name) = self.unsupported_name(&statement_macro.mac) {
                self.report(&name, &statement_macro.mac);
                *statement = Stmt::Expr(
                    parse_quote!(::core::unreachable!()),
                    statement_macro.semi_token,
                );
                return;
            }
        }
        visit_mut::visit_stmt_mut(self, statement);
    }

    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Macro(type_macro) = ty {
            if let Some(name) = self.unsupported_name(&type_macro.mac) {
                self.report(&name, &type_macro.mac);
                *ty = parse_quote!(__estado_core::NoEdge);
                return;
            }
        }
        visit_mut::visit_type_mut(self, ty);
    }
}
