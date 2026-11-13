//! Validation of a parsed [`MachineModel`]: the checks of spec 6.7 that need
//! the whole tree. Each returns diagnostics; none aborts the expansion.

mod configuration;
mod levels;
mod names;
mod options;

use crate::model::{Diagnostic, MachineModel};

/// Maximum nesting depth in levels (spec 3.2, const `MAX_DEPTH`). A region
/// counts as a level because it is a node of the graph.
pub const MAX_DEPTH: usize = 16;

/// Maximum number of simultaneously active states, regions included: the
/// capacity of the runtime's `StatePath`. It must equal
/// `estado_core::MAX_ACTIVE_STATES` (a test keeps them in step).
pub const MAX_ACTIVE_STATES: usize = 32;

/// Runs every structural and option check.
pub fn validate(model: &MachineModel) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    options::check(model, &mut diagnostics);
    names::check_unique_names(&model.tree, &mut diagnostics);
    levels::check_depth(&model.tree, &mut diagnostics);
    levels::check_initials(&model.tree, &mut diagnostics);
    configuration::check_active_configuration(&model.tree, &mut diagnostics);
    levels::check_counts(&model.tree, &mut diagnostics);
    diagnostics
}
