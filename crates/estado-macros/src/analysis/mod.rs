//! Validation of a parsed [`MachineModel`]: the checks of spec 6.7 that need
//! the whole tree. Each returns diagnostics; none aborts the expansion.

mod levels;
mod names;
mod options;

use crate::model::{Diagnostic, MachineModel};

/// Maximum nesting depth in levels (spec 3.2, const `MAX_DEPTH`). A region
/// counts as a level because it is a node of the graph.
pub const MAX_DEPTH: usize = 16;

/// Runs every structural and option check.
pub fn validate(model: &MachineModel) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    options::check(model, &mut diagnostics);
    names::check_unique_names(&model.tree, &mut diagnostics);
    levels::check_depth(&model.tree, &mut diagnostics);
    levels::check_initials(&model.tree, &mut diagnostics);
    levels::check_counts(&model.tree, &mut diagnostics);
    diagnostics
}
