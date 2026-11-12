//! The role attributes of the functions of a `#[handlers]` impl (spec 6.1):
//! `#[on(S)]`, `#[entry(S)]`, `#[exit(S)]`, `#[always(S)]`, `#[on_done(S)]`.

use syn::{Attribute, Ident, ImplItemFn};

use crate::model::{Diagnostic, Extent, HandlerRole};
use crate::naming::facade_name;

/// Attributes that are part of the macro language but not supported yet.
const UNSUPPORTED_ROLES: [&str; 6] = [
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
    facade_name(attribute.path()).map(ToString::to_string)
}

fn role_of(name: &str) -> Option<HandlerRole> {
    Some(match name {
        "on" => HandlerRole::On,
        "entry" => HandlerRole::Entry,
        "exit" => HandlerRole::Exit,
        "on_done" => HandlerRole::OnDone,
        "always" => HandlerRole::Always,
        _ => return None,
    })
}

/// Removes the role attributes (`#[on]`, `#[entry]`, ..) from an item that
/// is not a function, reporting each one: left in place, rustc would say
/// "cannot find attribute `on` in this scope" and nothing more.
pub fn reject_role_attributes(
    attributes: &mut Vec<Attribute>,
    item: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    attributes.retain(|attribute| {
        let Some(name) = attribute_name(attribute) else {
            return true;
        };
        if role_of(&name).is_none() && !UNSUPPORTED_ROLES.contains(&name.as_str()) {
            return true;
        }
        diagnostics.push(
            Diagnostic::new(
                "E0017",
                Extent::of(attribute),
                format!("`#[{name}]` must be on a function, not on {item}"),
            )
            .with_help(
                "handlers, entries, exits and the like are functions of the `#[handlers]` impl",
            ),
        );
        false
    });
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
                    Extent::of(attribute),
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
