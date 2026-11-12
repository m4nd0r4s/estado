//! State names must be unique across the machine, regions included (E0012).

use std::collections::HashMap;

use crate::model::{Diagnostic, StateTree};
use crate::naming::state_constant_name;

/// Two states clash when they have the same name, or when their different
/// names produce the same `StateId` constant (`Link` and a region `link`).
pub fn check_unique_names(tree: &StateTree, diagnostics: &mut Vec<Diagnostic>) {
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
