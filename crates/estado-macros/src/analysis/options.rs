//! Checks of the `#[machine(..)]` options against the tree (E0002, E0050).

use crate::model::{Diagnostic, MachineModel, StateKind};

pub fn check(model: &MachineModel, diagnostics: &mut Vec<Diagnostic>) {
    if !model.options.event_declared {
        diagnostics.push(
            Diagnostic::new(
                "E0002",
                model.options.span,
                "`#[machine]` does not declare its event type",
            )
            .with_help("add `event = MyEvent`, e.g. `#[machine(event = MyEvent)]`"),
        );
    }
    if model.options.typestate {
        let first_region = model
            .tree
            .nodes
            .iter()
            .find(|node| node.kind == StateKind::Region);
        if let Some(region) = first_region {
            diagnostics.push(
                Diagnostic::new(
                    "E0050",
                    region.name.span(),
                    "regions are not supported in `typestate` mode",
                )
                .with_help("remove `typestate`, or split the regions into separate machines"),
            );
        }
    }
}
