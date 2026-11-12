//! Parsing of the `#[machine]` module into the [`MachineModel`] (ADR 0010).
//!
//! Structure errors found while parsing (E0001, E0013-E0017) are returned
//! with the model; the semantic checks live in `analysis`.

mod attributes;
mod clean;
mod composite;
mod items;
pub mod options;
mod states;
#[cfg(test)]
mod tests;

use proc_macro2::TokenStream;
use syn::{Item, ItemMod};

use crate::model::{Diagnostic, MachineModel, StateTree, TypeDecl};

use clean::{clean_enum, clean_handlers, clean_struct};
use items::{RawType, sort_items};

/// Result of parsing: a model when the module has a usable `#[root]` enum.
pub struct Parsed {
    /// `None` when there is no root enum.
    pub model: Option<MachineModel>,
    /// When there is no model: the module items as written, minus the marker
    /// attributes, to emit so rust-analyzer still sees the types (spec 6.6).
    pub fallback_items: Vec<Item>,
    /// Errors found while parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses an inline module whose body is `module_items`.
pub fn parse_machine(attribute: TokenStream, module: &ItemMod, module_items: Vec<Item>) -> Parsed {
    let (options, mut diagnostics) = options::parse_options(attribute);
    let (items, sort_diagnostics) = sort_items(module_items);
    diagnostics.extend(sort_diagnostics);

    let roots = items.roots();
    let Some(root) = roots.first().copied() else {
        diagnostics.push(
            Diagnostic::new(
                "E0001",
                module.ident.span(),
                format!("machine module `{}` has no `#[root]` enum", module.ident),
            )
            .with_help("mark the enum of the top-level states with `#[root]`"),
        );
        return Parsed {
            model: None,
            fallback_items: items.into_cleaned_items(),
            diagnostics,
        };
    };
    for extra in &roots[1..] {
        diagnostics.push(
            Diagnostic::new(
                "E0001",
                extra.ident.span(),
                "a machine has exactly one `#[root]` enum",
            )
            .with_help(
                "a module holds a single machine; move this enum to another `#[machine]` module",
            )
            .with_note(root.ident.span(), "the first `#[root]` enum"),
        );
    }

    let nodes = states::build_nodes(&items, root, &mut diagnostics);
    let types = items
        .types
        .iter()
        .map(|raw| match raw {
            RawType::Root(declaration) | RawType::StateEnum(declaration) => {
                TypeDecl::Enum(clean_enum(declaration))
            }
            RawType::StateStruct(declaration) => TypeDecl::Struct(clean_struct(declaration)),
        })
        .collect();
    let model = MachineModel {
        options,
        module_attributes: module.attrs.clone(),
        module_visibility: module.vis.clone(),
        module_name: module.ident.clone(),
        tree: StateTree { nodes, types },
        handlers: items.handlers.iter().map(clean_handlers).collect(),
        analysed_handlers: Vec::new(),
        passthrough: items.passthrough,
    };
    Parsed {
        model: Some(model),
        fallback_items: Vec::new(),
        diagnostics,
    }
}
