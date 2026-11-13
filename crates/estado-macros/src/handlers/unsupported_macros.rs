//! `goto_dyn!` and `edges!` are exported by the facade but not supported yet.
//!
//! Left alone inside a handler they would expand to the facade stubs and
//! report E0040, "only valid inside a `#[handlers]` function", which is
//! wrong: they are inside one. They are reported here as E0090 instead and
//! replaced by neutral code, so no second error follows.

use syn::visit_mut::{self, VisitMut};
use syn::{Expr, ExprMacro, ImplItemFn, Macro, Stmt, Type, parse_quote};

use crate::model::{Diagnostic, Extent};
use crate::naming::facade_name;

/// Reports every `goto_dyn!` and `edges!` in `function` (signature and body)
/// as E0090 and replaces it: an expression by `unreachable!()`, a type by
/// `NoEdge`.
pub fn reject_unsupported_macros(function: &mut ImplItemFn, diagnostics: &mut Vec<Diagnostic>) {
    let mut rejector = Rejector { diagnostics };
    rejector.visit_signature_mut(&mut function.sig);
    rejector.visit_block_mut(&mut function.block);
}

struct Rejector<'diagnostics> {
    diagnostics: &'diagnostics mut Vec<Diagnostic>,
}

/// The name of `mac` when it is one of the unsupported macros.
fn unsupported_name(mac: &Macro) -> Option<String> {
    let name = facade_name(&mac.path)?.to_string();
    matches!(name.as_str(), "goto_dyn" | "edges").then_some(name)
}

impl Rejector<'_> {
    fn report(&mut self, name: &str, mac: &Macro) {
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
            if let Some(name) = unsupported_name(mac) {
                self.report(&name, mac);
                *expression = parse_quote!(::core::unreachable!());
                return;
            }
        }
        visit_mut::visit_expr_mut(self, expression);
    }

    fn visit_stmt_mut(&mut self, statement: &mut Stmt) {
        if let Stmt::Macro(statement_macro) = statement {
            if let Some(name) = unsupported_name(&statement_macro.mac) {
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
            if let Some(name) = unsupported_name(&type_macro.mac) {
                self.report(&name, &type_macro.mac);
                *ty = parse_quote!(__estado_core::NoEdge);
                return;
            }
        }
        visit_mut::visit_type_mut(self, ty);
    }
}
