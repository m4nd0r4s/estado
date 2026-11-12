//! Building values of states as Rust expressions: the value of a level from
//! the data a `goto!` supplies, and the `#[initial]` value of a level.
//!
//! Shared by the handler rewrite (the decision phase builds the value an
//! edge carries) and the generators (`Spec::initial`, reentering a compound).

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, FieldValue};

use crate::model::{HistoryKind, Payload, StateId, StateKind, StateTree};

/// Data the user wrote for one level of a `goto!` path.
pub enum SuppliedData {
    /// A bare name: `goto!(Paused)`.
    None,
    /// `T(a, b)`.
    Tuple(Vec<Expr>),
    /// `T { a, b: expression, ..base }`.
    Fields {
        values: Vec<FieldValue>,
        base: Option<Box<Expr>>,
    },
}

/// Fields a level needs and the user did not supply (E0020).
pub struct MissingData {
    /// The level.
    pub node: StateId,
    /// Names of the missing fields (`0`, `1` for tuple fields).
    pub fields: Vec<String>,
    /// Whether the level is entered through its `#[initial]` (no `goto!`
    /// data can reach it, `#[initial(with = ..)]` is the remedy).
    pub through_initial: bool,
}

/// The value of `node` as a variant of the enum that owns it. `below` is the
/// value of the children enum of a compound state (its `#[substates]` field).
pub fn variant_value(
    tree: &StateTree,
    node: StateId,
    data: &SuppliedData,
    below: Option<TokenStream>,
    through_initial: bool,
) -> Result<TokenStream, MissingData> {
    let state = tree.node(node);
    let owner = state
        .owner_enum
        .as_ref()
        .expect("a state has an owner enum");
    let variant = &state.name;
    let missing = |fields: Vec<String>| MissingData {
        node,
        fields,
        through_initial,
    };

    if state.composite_struct.is_some() && state.kind == StateKind::Compound {
        return composite_value(tree, node, data, below, through_initial);
    }
    match (&state.payload, data) {
        (Payload::Unit | Payload::None, _) => Ok(quote! { #owner::#variant }),
        (Payload::Newtype(_), SuppliedData::Tuple(arguments)) if arguments.len() == 1 => {
            Ok(quote! { #owner::#variant(#(#arguments),*) })
        }
        (Payload::Tuple(fields), SuppliedData::Tuple(arguments))
            if arguments.len() == fields.len() =>
        {
            Ok(quote! { #owner::#variant(#(#arguments),*) })
        }
        (Payload::Newtype(_), _) => Err(missing(vec!["0".to_owned()])),
        (Payload::Tuple(fields), _) => Err(missing(
            (0..fields.len()).map(|index| index.to_string()).collect(),
        )),
        (Payload::Struct(fields), SuppliedData::Fields { values, base }) => {
            let missing_fields = missing_field_names(
                fields.iter().filter_map(|field| field.ident.as_ref()),
                values,
            );
            if base.is_none() && !missing_fields.is_empty() {
                return Err(missing(missing_fields));
            }
            let base = base.as_ref().map(|base| quote! { ..#base });
            Ok(quote! { #owner::#variant { #(#values,)* #base } })
        }
        (Payload::Struct(fields), _) => Err(missing(
            fields
                .iter()
                .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
                .collect(),
        )),
    }
}

/// A compound state: `Owner::Variant(Struct { shared fields, sub: below })`.
fn composite_value(
    tree: &StateTree,
    node: StateId,
    data: &SuppliedData,
    below: Option<TokenStream>,
    through_initial: bool,
) -> Result<TokenStream, MissingData> {
    let state = tree.node(node);
    let owner = state.owner_enum.as_ref().expect("owner enum");
    let variant = &state.name;
    let struct_name = state.composite_struct.as_ref().expect("composite struct");
    let children_field = state.children_field.as_ref().expect("substates field");
    let shared = tree.shared_fields(node);
    let (values, base): (&[FieldValue], Option<&Expr>) = match data {
        SuppliedData::Fields { values, base } => (values, base.as_deref()),
        _ => (&[], None),
    };
    let missing_fields = missing_field_names(
        shared.iter().filter_map(|field| field.ident.as_ref()),
        values,
    );
    if base.is_none() && !missing_fields.is_empty() {
        return Err(MissingData {
            node,
            fields: missing_fields,
            through_initial,
        });
    }
    let below = below.expect("a compound level has a value below it");
    let base = base.map(|base| quote! { ..#base });
    Ok(quote! {
        #owner::#variant(#struct_name { #(#values,)* #children_field: #below, #base })
    })
}

fn missing_field_names<'a>(
    required: impl Iterator<Item = &'a syn::Ident>,
    supplied: &[FieldValue],
) -> Vec<String> {
    let given: Vec<String> = supplied
        .iter()
        .filter_map(|value| match &value.member {
            syn::Member::Named(name) => Some(name.to_string()),
            syn::Member::Unnamed(_) => None,
        })
        .collect();
    required
        .map(ToString::to_string)
        .filter(|name| !given.contains(name))
        .collect()
}

/// The value of the children enum of `owner` (a compound state, a region or
/// the root): its `#[initial]` child, with the initials below it.
pub fn initial_value(tree: &StateTree, owner: StateId) -> Result<TokenStream, MissingData> {
    let Some(initial) = tree.node(owner).initial else {
        // E0010 reports a level without `#[initial]`; keep expanding.
        return Ok(quote! { ::core::unreachable!() });
    };
    let state = tree.node(initial);
    if let Some(constructor) = &state.initial_with {
        return Ok(quote! { #constructor() });
    }
    let below = match state.kind {
        StateKind::Compound => Some(initial_value(tree, initial)?),
        _ => None,
    };
    variant_value(tree, initial, &SuppliedData::None, below, true)
}

/// The type of `Exiting![X]` (spec 6.3, ADR 0007): the owned value of the
/// state `X` that exits through a constructor.
pub fn exiting_type(tree: &StateTree, node: StateId) -> TokenStream {
    let state = tree.node(node);
    let variant = &state.name;
    match &state.payload {
        // Deep history takes the substates: the constructor gets the rest
        // (errata 5).
        Payload::Newtype(_) if state.history == HistoryKind::DeepKeep => {
            let struct_name = state.composite_struct.as_ref().expect("a compound");
            let name = quote::format_ident!("{}Exiting", struct_name);
            quote! { #name }
        }
        Payload::Newtype(payload_type) => quote! { #payload_type },
        Payload::Struct(_) | Payload::Tuple(_) => {
            let name = quote::format_ident!("{}Exiting", variant);
            quote! { #name }
        }
        Payload::Unit | Payload::None => quote! { () },
    }
}
