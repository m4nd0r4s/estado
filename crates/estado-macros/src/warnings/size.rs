//! W0004: a variant whose data is larger than `size_warn` bytes.
//!
//! The size is only known to rustc, so each candidate variant becomes a
//! conditional warning on `size_of::<(field types)>() > size_warn`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Field, Ident, Type};

use super::code::WarningCode;
use super::emit::Warning;
use crate::model::{MachineModel, Payload, StateKind, StateNode, TypeDecl};

/// One conditional warning per variant that carries data.
///
/// The measured type is the tuple of the variant's own fields. A compound or
/// parallel state is measured without its `#[substates]` and `#[region]`
/// fields: those are separate variants, reported on their own, so a large
/// leaf does not make every ancestor warn as well.
pub fn oversized_variants(model: &MachineModel) -> Vec<Warning> {
    let limit = model.options.size_warn;
    model
        .tree
        .nodes
        .iter()
        .skip(1)
        .filter(|node| node.kind != StateKind::Region && node.owner_enum.is_some())
        .filter_map(|node| {
            let measured = measured_type(model, node)?;
            Some(Warning {
                code: WarningCode::OversizedVariant,
                span: node.name.span(),
                subject: crate::naming::plain_name(&node.name),
                message: format!(
                    "variant `{}` holds more than {limit} bytes (`size_warn`); \
                     consider boxing its data with `Box<..>`, or raise `size_warn`",
                    node.name
                ),
                condition: Some(quote! { ::core::mem::size_of::<#measured>() > #limit }),
            })
        })
        .collect()
}

/// The type whose size is the variant's size; `None` for a variant without data.
fn measured_type(model: &MachineModel, node: &StateNode) -> Option<TokenStream> {
    if let Some(composite) = &node.composite_struct {
        let own_fields = own_fields_of_composite(model, node, composite);
        return (!own_fields.is_empty())
            .then(|| tuple_of(own_fields.iter().map(|field| &field.ty)));
    }
    match &node.payload {
        Payload::Unit | Payload::None => None,
        Payload::Newtype(ty) => Some(quote! { #ty }),
        Payload::Tuple(fields) | Payload::Struct(fields) => {
            Some(tuple_of(fields.iter().map(|field| &field.ty)))
        }
    }
}

/// Fields of a compound or parallel struct that are not substates or regions.
fn own_fields_of_composite<'model>(
    model: &'model MachineModel,
    node: &StateNode,
    composite: &Ident,
) -> Vec<&'model Field> {
    let declaration = model
        .tree
        .types
        .iter()
        .find_map(|declaration| match declaration {
            TypeDecl::Struct(declaration) if declaration.ident == *composite => Some(declaration),
            _ => None,
        });
    let Some(declaration) = declaration else {
        return Vec::new();
    };
    let is_child_field = |field: &Field| {
        field.ident.as_ref().is_some_and(|name| {
            node.children_field.as_ref() == Some(name)
                || node
                    .children
                    .iter()
                    .any(|&child| model.tree.node(child).name == *name)
        })
    };
    declaration
        .fields
        .iter()
        .filter(|field| !is_child_field(field))
        .collect()
}

fn tuple_of<'field>(types: impl Iterator<Item = &'field Type>) -> TokenStream {
    let types: Vec<_> = types.collect();
    quote! { (#(#types,)*) }
}
