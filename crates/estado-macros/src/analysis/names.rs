//! State names must be unique across the machine, regions included, and must
//! not be their own id constant (E0012).

use std::collections::HashMap;

use syn::Ident;
use syn::ext::IdentExt;

use crate::model::{Diagnostic, StateTree};
use crate::naming::state_constant_name;

/// Two states clash when they have the same name, or when their different
/// names produce the same `StateId` constant (`Link` and a region `link`).
pub fn check_unique_names(tree: &StateTree, diagnostics: &mut Vec<Diagnostic>) {
    check_constants_against_variants(tree, diagnostics);
    let mut first_by_constant = HashMap::new();
    for node in tree.nodes.iter().skip(1) {
        let constant = state_constant_name(&node.name);
        let Some(&first_id) = first_by_constant.get(&constant) else {
            first_by_constant.insert(constant, node.id);
            continue;
        };
        let first = tree.node(first_id);
        let message = if first.name == node.name {
            format!("duplicate state name `{}`", node.name)
        } else {
            format!(
                "state `{}` clashes with `{}`: both generate the id constant `{constant}`",
                node.name, first.name
            )
        };
        diagnostics.push(
            Diagnostic::new("E0012", node.name.span(), message)
                .with_help("state names are unique in a machine so that `goto!(Name)` is unambiguous; rename one")
                .with_note(first.name.span(), format!("`{}` is first declared here", first.name)),
        );
    }
}

/// The id constants are associated constants of the root type, so a constant
/// named like a variant of the root enum (`IO` for a state called `IO`)
/// would be both at once: reported here instead of as a type error in the
/// generated code.
fn check_constants_against_variants(tree: &StateTree, diagnostics: &mut Vec<Diagnostic>) {
    let root_variants: Vec<(String, &Ident)> = tree
        .root()
        .children
        .iter()
        .map(|&id| &tree.node(id).name)
        .map(|name| (name.unraw().to_string(), name))
        .collect();
    let root = &tree.root().name;
    for node in tree.nodes.iter().skip(1) {
        let constant = state_constant_name(&node.name);
        let Some((_, variant)) = root_variants.iter().find(|(name, _)| *name == constant) else {
            continue;
        };
        let message = if node.name == **variant {
            format!(
                "state `{}` is named like its own id constant `{root}::{constant}`",
                node.name
            )
        } else {
            format!(
                "the id constant `{root}::{constant}` of state `{}` is also the variant `{variant}`",
                node.name
            )
        };
        diagnostics.push(
            Diagnostic::new("E0012", node.name.span(), message)
                .with_help("name states in CamelCase (`Io` instead of `IO`) so that the id constant, in SCREAMING_SNAKE_CASE, is a different name"),
        );
    }
}
