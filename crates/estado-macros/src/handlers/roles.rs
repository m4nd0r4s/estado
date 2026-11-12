//! The role attributes of the functions of a `#[handlers]` impl (spec 6.1):
//! `#[on(S)]`, `#[entry(S)]`, `#[exit(S)]`, `#[always(S)]`, `#[on_done(S)]`.

use syn::spanned::Spanned;
use syn::{Attribute, Ident, ImplItemFn};

use crate::model::{Diagnostic, HandlerRole};

/// Facade crates an attribute may be qualified with (`#[estado::on(..)]`).
const FACADE_CRATES: [&str; 2] = ["estado", "estado_macros"];

/// Attributes that are part of the macro language but not supported yet.
const UNSUPPORTED_ROLES: [&str; 7] = [
    "always",
    "choice",
    "on_child_done",
    "on_resume",
    "invariant",
    "edges",
    "on_error",
];

/// What the attributes of a function say.
pub struct RoleAttribute {
    /// The role.
    pub role: HandlerRole,
    /// The state named in the attribute.
    pub state: Ident,
}

/// The result of reading the attributes of one function.
pub enum FunctionRole {
    /// A plain helper function: nothing to do.
    Helper,
    /// A handler of some role.
    Handler(RoleAttribute),
    /// A broken or unsupported role attribute; already reported.
    Invalid,
}

fn attribute_name(attribute: &Attribute) -> Option<String> {
    let path = attribute.path();
    let segment = match path.segments.len() {
        1 => &path.segments[0],
        2 if FACADE_CRATES
            .iter()
            .any(|facade| path.segments[0].ident == facade) =>
        {
            &path.segments[1]
        }
        _ => return None,
    };
    Some(segment.ident.to_string())
}

fn role_of(name: &str) -> Option<HandlerRole> {
    Some(match name {
        "on" => HandlerRole::On,
        "entry" => HandlerRole::Entry,
        "exit" => HandlerRole::Exit,
        "on_done" => HandlerRole::OnDone,
        _ => return None,
    })
}

/// Reads and removes the role attributes of `function`.
pub fn take_role(function: &mut ImplItemFn, diagnostics: &mut Vec<Diagnostic>) -> FunctionRole {
    let mut found: Vec<RoleAttribute> = Vec::new();
    let mut invalid = false;
    function.attrs.retain(|attribute| {
        let Some(name) = attribute_name(attribute) else {
            return true;
        };
        if UNSUPPORTED_ROLES.contains(&name.as_str()) {
            diagnostics.push(
                Diagnostic::new(
                    "E0090",
                    attribute.path().span(),
                    format!("`#[{name}]` is not supported yet by this version of `estado`"),
                )
                .with_help("it is part of the planned macro language (spec 6.1)"),
            );
            invalid = true;
            return false;
        }
        let Some(role) = role_of(&name) else {
            return true;
        };
        match attribute.parse_args::<Ident>() {
            Ok(state) => found.push(RoleAttribute { role, state }),
            Err(error) => {
                diagnostics.push(
                    Diagnostic::new(
                        "E0006",
                        error.span(),
                        format!("`#[{name}]` needs the name of a state"),
                    )
                    .with_help(format!("write `#[{name}(StateName)]`")),
                );
                invalid = true;
            }
        }
        false
    });
    if invalid {
        return FunctionRole::Invalid;
    }
    let mut roles = found.into_iter();
    let Some(first) = roles.next() else {
        return FunctionRole::Helper;
    };
    if let Some(second) = roles.next() {
        diagnostics.push(
            Diagnostic::new(
                "E0005",
                second.state.span(),
                "a function has exactly one role attribute",
            )
            .with_help("split it into one function per role"),
        );
        return FunctionRole::Invalid;
    }
    FunctionRole::Handler(first)
}
