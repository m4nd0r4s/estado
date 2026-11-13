//! Checks of the `#[machine(..)]` options against the tree (E0002, E0050,
//! E0090).

use crate::model::{Diagnostic, MachineModel, StateKind};
use crate::suggest::did_you_mean;
use crate::warnings::WarningCode;

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
    let first_region = model
        .tree
        .nodes
        .iter()
        .find(|node| node.kind == StateKind::Region);
    for option in &model.options.unsupported {
        if option.name == "typestate" {
            if let Some(region) = first_region {
                diagnostics.push(
                    Diagnostic::new(
                        "E0050",
                        region.name.span(),
                        "regions are not supported in `typestate` mode",
                    )
                    .with_help("remove `typestate`, or split the regions into separate machines"),
                );
                continue;
            }
        }
        diagnostics.push(
            Diagnostic::new(
                "E0090",
                option.span,
                format!(
                    "the option `{}` is not supported yet by this version of `estado`",
                    option.name
                ),
            )
            .with_help("it is part of the planned macro language; remove it for now"),
        );
    }
    check_allowed_warnings(model, diagnostics);
}

/// `allow(W....)` must name a warning that exists (E0016).
fn check_allowed_warnings(model: &MachineModel, diagnostics: &mut Vec<Diagnostic>) {
    for code in &model.options.allowed_warnings {
        let written = code.to_string();
        if WarningCode::from_label(&written).is_some() {
            continue;
        }
        let help = match did_you_mean(&written, WarningCode::labels()) {
            Some(suggestion) => format!("did you mean `{suggestion}`?"),
            None => format!(
                "the warnings are: {}",
                WarningCode::labels().collect::<Vec<_>>().join(", ")
            ),
        };
        diagnostics.push(
            Diagnostic::new(
                "E0016",
                code.span(),
                format!("unknown warning code `{written}` in `allow(..)`"),
            )
            .with_help(help),
        );
    }
}
