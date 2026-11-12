//! The syntax of `goto!(..)` (spec 6.3): a target path with data, then an
//! optional `external`, `history` or constructor closure.

use proc_macro2::TokenStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, ExprClosure, Ident, Token};

use crate::construct::SuppliedData;
use crate::model::Diagnostic;

/// Code for a malformed transition (not in the spec table).
pub const MALFORMED_TRANSITION: &str = "E0043";

/// One name of the target path with the data written for it.
pub struct LevelSyntax {
    /// The state name.
    pub name: Ident,
    /// `T`, `T(a)` or `T { a }`.
    pub data: SuppliedData,
}

/// What follows the target.
pub enum GotoExtra {
    /// Nothing.
    None,
    /// `external`.
    External(Ident),
    /// `history`.
    History(Ident),
    /// `|old: Exiting![X]| expression`.
    Constructor(Box<ExprClosure>),
}

/// A parsed `goto!` invocation.
pub struct GotoSyntax {
    /// The path `A { .. } / B { .. }`, outermost first; a single level for
    /// the common form.
    pub levels: Vec<LevelSyntax>,
    /// Option after the target.
    pub extra: GotoExtra,
}

fn malformed(span: proc_macro2::Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(MALFORMED_TRANSITION, span, message).with_help(
        "write `goto!(State)`, `goto!(State { field: value })`, `goto!(State(value))`, \
         `goto!(Outer { .. } / Inner { .. })`, `goto!(State, external)`, \
         `goto!(State, history)` or `goto!(State, |old: Exiting![Source]| new_value)`",
    )
}

/// Parses the tokens inside `goto!(..)`.
pub fn parse_goto(tokens: TokenStream) -> Result<GotoSyntax, Diagnostic> {
    let arguments = syn::parse::Parser::parse2(
        Punctuated::<Expr, Token![,]>::parse_terminated,
        tokens.clone(),
    )
    .map_err(|error| malformed(error.span(), format!("cannot read this `goto!`: {error}")))?;
    let mut arguments = arguments.into_iter();
    let Some(target) = arguments.next() else {
        return Err(malformed(
            proc_macro2::Span::call_site(),
            "`goto!` needs a target state",
        ));
    };
    let mut levels = Vec::new();
    flatten_path(&target, &mut levels)?;
    let extra = match arguments.next() {
        None => GotoExtra::None,
        Some(Expr::Closure(closure)) => GotoExtra::Constructor(Box::new(closure)),
        Some(Expr::Path(path)) if path.path.is_ident("external") => {
            GotoExtra::External(path.path.segments[0].ident.clone())
        }
        Some(Expr::Path(path)) if path.path.is_ident("history") => {
            GotoExtra::History(path.path.segments[0].ident.clone())
        }
        Some(other) => {
            return Err(malformed(
                other.span(),
                "expected `external`, `history` or a constructor closure after the target",
            ));
        }
    };
    if let Some(unexpected) = arguments.next() {
        return Err(malformed(
            unexpected.span(),
            "too many arguments to `goto!`",
        ));
    }
    Ok(GotoSyntax { levels, extra })
}

/// `A { .. } / B { .. }` is a division of two expressions in Rust syntax;
/// flatten it, left to right.
fn flatten_path(expression: &Expr, levels: &mut Vec<LevelSyntax>) -> Result<(), Diagnostic> {
    match expression {
        Expr::Binary(binary) if matches!(binary.op, BinOp::Div(_)) => {
            flatten_path(&binary.left, levels)?;
            flatten_path(&binary.right, levels)
        }
        Expr::Paren(inner) => flatten_path(&inner.expr, levels),
        Expr::Group(inner) => flatten_path(&inner.expr, levels),
        Expr::Path(path) if path.qself.is_none() => {
            levels.push(LevelSyntax {
                name: last_ident(&path.path, path.span())?,
                data: SuppliedData::None,
            });
            Ok(())
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return Err(malformed(call.func.span(), "expected a state name"));
            };
            levels.push(LevelSyntax {
                name: last_ident(&path.path, path.span())?,
                data: SuppliedData::Tuple(call.args.iter().cloned().collect()),
            });
            Ok(())
        }
        Expr::Struct(structure) => {
            levels.push(LevelSyntax {
                name: last_ident(&structure.path, structure.path.span())?,
                data: SuppliedData::Fields {
                    values: structure.fields.iter().cloned().collect(),
                    base: structure.rest.clone(),
                },
            });
            Ok(())
        }
        other => Err(malformed(other.span(), "expected a state name")),
    }
}

fn last_ident(path: &syn::Path, span: proc_macro2::Span) -> Result<Ident, Diagnostic> {
    path.segments
        .last()
        .map(|segment| segment.ident.clone())
        .ok_or_else(|| malformed(span, "expected a state name"))
}
