//! Classifies a `#[state]` struct: plain, compound (`#[substates]`) or
//! parallel (two or more `#[region]` fields). Spec 3.1 and 3.3.

use syn::spanned::Spanned;
use syn::{Ident, ItemStruct, Type};

use super::clean::has_marker;
use crate::model::Diagnostic;

/// A field that holds a child enum.
pub struct ChildField {
    /// Field name.
    pub name: Ident,
    /// Declared type.
    pub field_type: Type,
}

/// What a `#[state]` struct is.
pub enum Composition {
    /// No `#[substates]` and no `#[region]`: an atomic state with data.
    Plain,
    /// One `#[substates]` field.
    Compound(Box<ChildField>),
    /// `#[region]` fields, in declaration order.
    Parallel(Vec<ChildField>),
}

/// Looks at the `#[substates]` and `#[region]` fields. Structural errors
/// (E0013, E0014, E0017) are reported and the closest valid shape returned,
/// so the rest of the tree is still built.
pub fn classify(declaration: &ItemStruct, diagnostics: &mut Vec<Diagnostic>) -> Composition {
    let struct_name = &declaration.ident;
    let mut substates = Vec::new();
    let mut regions = Vec::new();
    for field in &declaration.fields {
        let is_substates = has_marker(&field.attrs, "substates");
        let is_region = has_marker(&field.attrs, "region");
        if !is_substates && !is_region {
            continue;
        }
        let Some(name) = field.ident.clone() else {
            diagnostics.push(Diagnostic::new(
                "E0017",
                field.ty.span(),
                "`#[substates]` and `#[region]` need a named field",
            ));
            continue;
        };
        let child = ChildField {
            name,
            field_type: field.ty.clone(),
        };
        if is_substates && is_region {
            diagnostics.push(Diagnostic::new(
                "E0013",
                child.name.span(),
                "a field cannot be both `#[substates]` and `#[region]`",
            ));
        }
        if is_substates {
            substates.push(child);
        } else {
            regions.push(child);
        }
    }

    if !substates.is_empty() && !regions.is_empty() {
        diagnostics.push(
            Diagnostic::new(
                "E0013",
                regions[0].name.span(),
                format!("`{struct_name}` has both `#[substates]` and `#[region]` fields"),
            )
            .with_help(
                "a state is compound or parallel, never both; wrap the regions in a substate",
            )
            .with_note(substates[0].name.span(), "the `#[substates]` field"),
        );
    } else if substates.len() > 1 {
        diagnostics.push(
            Diagnostic::new(
                "E0013",
                substates[1].name.span(),
                format!("`{struct_name}` has more than one `#[substates]` field"),
            )
            .with_help(
                "a compound state has exactly one; use `#[region]` fields for parallel states",
            )
            .with_note(substates[0].name.span(), "the first `#[substates]` field"),
        );
    }

    if let Some(first) = substates.into_iter().next() {
        return Composition::Compound(Box::new(first));
    }
    if regions.len() == 1 {
        diagnostics.push(
            Diagnostic::new(
                "E0014",
                regions[0].name.span(),
                format!("`{struct_name}` has a single `#[region]` field; a parallel state needs at least two"),
            )
            .with_help("use `#[substates]` for one level of substates, or add a second region"),
        );
    }
    if regions.is_empty() {
        Composition::Plain
    } else {
        Composition::Parallel(regions)
    }
}
