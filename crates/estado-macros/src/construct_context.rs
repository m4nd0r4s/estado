//! Building values of states when a `Cx` is in scope (errata 15).
//!
//! `#[initial(with = f)]` where `f` takes a `&mut Cx<..>` and the `*Fields`
//! of the parent state must run when the state is entered, after the exits
//! of the transition, so the value that contains it cannot be built in the
//! decision phase. The expressions of this module are evaluated where `cx`
//! is a `&mut Cx` binding: the start of the machine (`Spec::initial_in`),
//! `reenter!()` and the `apply` of an edge.
//!
//! A level that has shared fields cannot be written as one struct literal
//! when its substates need those fields: the fields are bound to locals
//! first, handed to the initializer as a `*Fields` view of `&mut`
//! references, and moved into the struct afterwards.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::construct::{MissingData, SuppliedData, variant_value};
use crate::generate::naming::fields_type;
use crate::model::{InitialArgument, StateId, StateKind, StateNode, StateTree};

/// Whether building the `#[initial]` value of the children of `owner` calls
/// an initializer that needs `cx` (the root, a compound state or a region).
pub fn needs_context(tree: &StateTree, owner: StateId) -> bool {
    let Some(initial) = tree.node(owner).initial else {
        return false;
    };
    let state = tree.node(initial);
    if state.initial_with.is_some() {
        return !state.initial_arguments.is_empty();
    }
    match state.kind {
        StateKind::Compound => needs_context(tree, initial),
        StateKind::Parallel => state
            .children
            .iter()
            .any(|&region| needs_context(tree, region)),
        _ => false,
    }
}

/// `__estado_part_0`: the binding of the first value an edge carries when its
/// value is built in `apply`.
pub fn part_ident(index: usize) -> Ident {
    format_ident!("__estado_part_{index}")
}

/// `ActiveFields { volume: &mut <place>.volume }`: the `*Fields` view of the
/// compound or parallel state `node`, borrowing the fields from `place`
/// (an expression that names the struct, for example a binding).
pub fn fields_view_of(tree: &StateTree, node: StateId, place: &TokenStream) -> TokenStream {
    let view = fields_type(tree, node);
    let names: Vec<&Ident> = tree
        .shared_fields(node)
        .into_iter()
        .filter_map(|field| field.ident.as_ref())
        .collect();
    if names.is_empty() {
        return quote! { #view { _lifetime: ::core::marker::PhantomData } };
    }
    quote! { #view { #(#names: &mut #place.#names),* } }
}

/// The local that holds the shared field `name` while a level is built.
fn field_local(name: &Ident) -> Ident {
    format_ident!(
        "__estado_field_{}",
        name.to_string().trim_start_matches("r#")
    )
}

/// The value of the children enum of `owner` (the root, a compound state or
/// a region) through its `#[initial]` chain. `parent_fields` is the `*Fields`
/// view of the state whose substates or region this is; `None` at the root.
pub fn initial_value_in(
    tree: &StateTree,
    owner: StateId,
    parent_fields: Option<TokenStream>,
) -> Result<TokenStream, MissingData> {
    let Some(initial) = tree.node(owner).initial else {
        // E0010 reports a level without `#[initial]`; keep expanding.
        return Ok(quote! { ::core::unreachable!() });
    };
    let state = tree.node(initial);
    if let Some(constructor) = &state.initial_with {
        return Ok(constructor_call(state, constructor, parent_fields));
    }
    match state.kind {
        StateKind::Compound | StateKind::Parallel => {
            composite_value_in(tree, initial, &SuppliedData::None, None, &[], true)
        }
        _ => variant_value(tree, initial, &SuppliedData::None, None, &[], true),
    }
}

/// `function(cx, fields)`: the arguments in the order the function declares.
fn constructor_call(
    state: &StateNode,
    constructor: &syn::Path,
    parent_fields: Option<TokenStream>,
) -> TokenStream {
    let arguments = state
        .initial_arguments
        .iter()
        .map(|argument| match argument {
            InitialArgument::Context => quote! { cx },
            InitialArgument::ParentFields => parent_fields
                .clone()
                .expect("the analysis rejects fields where there is no parent state"),
        });
    quote! { #constructor(#(#arguments),*) }
}

/// A compound or parallel level as `Owner::Variant(Struct { .. })`, with a
/// `cx` in scope. `below` is the value of its substates when a path chose
/// one (`None`: its `#[initial]`); `regions` the regions a path chose.
pub fn composite_value_in(
    tree: &StateTree,
    node: StateId,
    data: &SuppliedData,
    below: Option<TokenStream>,
    regions: &[(StateId, TokenStream)],
    through_initial: bool,
) -> Result<TokenStream, MissingData> {
    let state = tree.node(node);
    let owner = state.owner_enum.as_ref().expect("owner enum");
    let variant = &state.name;
    let struct_name = state.composite_struct.as_ref().expect("composite struct");
    let shared = tree.shared_fields(node);
    let names: Vec<&Ident> = shared
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .collect();
    let (values, base): (&[syn::FieldValue], Option<&syn::Expr>) = match data {
        SuppliedData::Fields { values, base } => (values, base.as_deref()),
        _ => (&[], None),
    };
    let supplied = |name: &Ident| {
        values
            .iter()
            .find(|value| matches!(&value.member, syn::Member::Named(given) if given == name))
    };
    let missing: Vec<String> = names
        .iter()
        .filter(|name| supplied(name).is_none())
        .map(ToString::to_string)
        .collect();
    if base.is_none() && !missing.is_empty() {
        return Err(MissingData {
            node,
            fields: missing,
            through_initial,
        });
    }

    let base_local = format_ident!("__estado_base");
    let mut statements = TokenStream::new();
    if let Some(base) = base {
        statements.extend(quote! { let #base_local = #base; });
    }
    for name in &names {
        let local = field_local(name);
        let value = supplied(name).map_or_else(
            || quote! { #base_local.#name },
            |value| {
                let expression = &value.expr;
                quote! { #expression }
            },
        );
        statements.extend(quote! { #[allow(unused_mut)] let mut #local = #value; });
    }
    let locals: Vec<Ident> = names.iter().map(|name| field_local(name)).collect();
    let view = fields_type(tree, node);
    let fields_view = if names.is_empty() {
        quote! { #view { _lifetime: ::core::marker::PhantomData } }
    } else {
        quote! { #view { #(#names: &mut #locals),* } }
    };

    let mut pairs = Vec::new();
    if state.kind == StateKind::Parallel {
        for &region in &state.children {
            let field = tree.node(region).children_field.as_ref().expect("field");
            let value = match regions.iter().find(|(id, _)| *id == region) {
                Some((_, value)) => value.clone(),
                None => initial_value_in(tree, region, Some(fields_view.clone()))?,
            };
            let local = format_ident!(
                "__estado_region_{}",
                field.to_string().trim_start_matches("r#")
            );
            statements.extend(quote! { let #local = #value; });
            pairs.push(quote! { #field: #local });
        }
    } else {
        let field = state.children_field.as_ref().expect("substates field");
        let value = match below {
            Some(value) => value,
            None => initial_value_in(tree, node, Some(fields_view))?,
        };
        statements.extend(quote! { let __estado_substates = #value; });
        pairs.push(quote! { #field: __estado_substates });
    }
    Ok(quote! {
        {
            #statements
            #owner::#variant(#struct_name { #(#names: #locals,)* #(#pairs),* })
        }
    })
}
