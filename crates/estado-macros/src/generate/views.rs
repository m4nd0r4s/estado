//! The views of ADR 0007, emitted next to the user's types in the machine
//! module: `<Composite>Fields<'a>` with `reborrow`, `split` on the struct,
//! `<Variant>Mut<'a>` and the owned `<Variant>Exiting` structs.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Field, Visibility};

use super::context::Generation;
use super::naming::{exiting_struct, fields_type, view_type};
use crate::model::{GotoForm, HistoryKind, Payload, StateId, StateKind};

/// Every view of the machine.
pub fn view_items(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let mut items = TokenStream::new();
    for node in tree.nodes.iter().skip(1) {
        if node.kind == StateKind::Compound {
            items.extend(fields_view(generation, node.id));
        }
        match &node.payload {
            Payload::Struct(fields) | Payload::Tuple(fields) => {
                items.extend(variant_view(generation, node.id, fields));
            }
            _ => {}
        }
    }
    items.extend(exiting_structs(generation));
    items
}

fn struct_visibility(generation: &Generation<'_>, node: StateId) -> Visibility {
    let name = generation
        .tree
        .node(node)
        .composite_struct
        .as_ref()
        .expect("composite");
    generation
        .tree
        .struct_declaration(name)
        .map_or(Visibility::Inherited, |declaration| declaration.vis.clone())
}

fn enum_visibility(generation: &Generation<'_>, node: StateId) -> Visibility {
    let name = generation
        .tree
        .node(node)
        .owner_enum
        .as_ref()
        .expect("owner enum");
    generation
        .tree
        .enum_declaration(name)
        .map_or(Visibility::Inherited, |declaration| declaration.vis.clone())
}

/// `ActiveFields<'a>`, its `reborrow` and `Active::split`.
fn fields_view(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(node);
    let struct_name = state.composite_struct.as_ref().expect("composite");
    let children_field = state.children_field.as_ref().expect("substates");
    let children_enum = state.children_enum.as_ref().expect("children enum");
    let view = fields_type(tree, node);
    let visibility = struct_visibility(generation, node);
    let shared = tree.shared_fields(node);
    let names: Vec<_> = shared
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .collect();
    let types: Vec<_> = shared.iter().map(|field| &field.ty).collect();
    let documentation = format!(
        "The fields of [`{struct_name}`] without its `#[substates]` field: the split borrow of ADR 0007."
    );
    let field_docs: Vec<_> = names
        .iter()
        .map(|name| format!("`{struct_name}::{name}`."))
        .collect();
    let marker = if names.is_empty() {
        quote! { #[doc(hidden)] pub _lifetime: ::core::marker::PhantomData<&'a mut ()>, }
    } else {
        TokenStream::new()
    };
    let marker_value = if names.is_empty() {
        quote! { _lifetime: ::core::marker::PhantomData }
    } else {
        TokenStream::new()
    };
    quote! {
        #[doc = #documentation]
        #visibility struct #view<'a> {
            #( #[doc = #field_docs] pub #names: &'a mut #types, )*
            #marker
        }

        impl #view<'_> {
            /// A shorter-lived copy, to hand the fields to a helper and keep using them.
            pub fn reborrow(&mut self) -> #view<'_> {
                #view { #( #names: &mut *self.#names, )* #marker_value }
            }
        }

        impl #struct_name {
            /// Splits the borrow: the shared fields and the substates.
            pub fn split(&mut self) -> (#view<'_>, &mut #children_enum) {
                let Self { #(#names,)* #children_field } = self;
                (#view { #(#names,)* #marker_value }, #children_field)
            }
        }
    }
}

/// `PlayingMut<'a>` for a struct-like or tuple variant.
fn variant_view(generation: &Generation<'_>, node: StateId, fields: &[Field]) -> TokenStream {
    let tree = generation.tree;
    let view = view_type(tree, node);
    let visibility = enum_visibility(generation, node);
    let variant = &tree.node(node).name;
    let documentation = format!("View of the variant `{variant}`: a `&mut` to each of its fields.");
    let types: Vec<_> = fields.iter().map(|field| &field.ty).collect();
    if matches!(tree.node(node).payload, Payload::Struct(_)) {
        let names: Vec<_> = fields
            .iter()
            .filter_map(|field| field.ident.as_ref())
            .collect();
        let docs: Vec<_> = names
            .iter()
            .map(|name| format!("`{name}` of the variant."))
            .collect();
        quote! {
            #[doc = #documentation]
            #visibility struct #view<'a> { #( #[doc = #docs] pub #names: &'a mut #types, )* }
        }
    } else {
        quote! {
            #[doc = #documentation]
            #visibility struct #view<'a>( #( pub &'a mut #types, )* );
        }
    }
}

/// `<Variant>Exiting`: owned values of struct-like and tuple variants that
/// a constructor receives (ADR 0007, item 3).
fn exiting_structs(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let mut done = HashSet::new();
    let mut items = TokenStream::new();
    for handler in &generation.model.analysed_handlers {
        for goto in &handler.gotos {
            let GotoForm::Constructor { exiting } = goto.form else {
                continue;
            };
            if !done.insert(exiting) {
                continue;
            }
            let name = exiting_struct(tree, exiting);
            let visibility = enum_visibility(generation, exiting);
            let variant = &tree.node(exiting).name;
            let documentation =
                format!("`Exiting![{variant}]`: the owned fields of the variant `{variant}`.");
            match &tree.node(exiting).payload {
                Payload::Newtype(_) if tree.node(exiting).history == HistoryKind::DeepKeep => {
                    let struct_name = tree
                        .node(exiting)
                        .composite_struct
                        .as_ref()
                        .expect("compound");
                    let name = quote::format_ident!("{}Exiting", struct_name);
                    let shared = tree.shared_fields(exiting);
                    let names: Vec<_> = shared
                        .iter()
                        .filter_map(|field| field.ident.as_ref())
                        .collect();
                    let types: Vec<_> = shared.iter().map(|field| &field.ty).collect();
                    let docs: Vec<_> = names
                        .iter()
                        .map(|name| format!("`{struct_name}::{name}`."))
                        .collect();
                    let documentation = format!(
                        "`Exiting![{variant}]`: `{struct_name}` without the substates, which moved into the deep history (errata 5)."
                    );
                    let visibility = struct_visibility(generation, exiting);
                    items.extend(quote! {
                        #[doc = #documentation]
                        #visibility struct #name { #( #[doc = #docs] pub #names: #types, )* }
                    });
                }
                Payload::Struct(fields) => {
                    let names: Vec<_> = fields
                        .iter()
                        .filter_map(|field| field.ident.as_ref())
                        .collect();
                    let types: Vec<_> = fields.iter().map(|field| &field.ty).collect();
                    let docs: Vec<_> = names
                        .iter()
                        .map(|name| format!("`{name}` of the variant."))
                        .collect();
                    items.extend(quote! {
                        #[doc = #documentation]
                        #visibility struct #name { #( #[doc = #docs] pub #names: #types, )* }
                    });
                }
                Payload::Tuple(fields) => {
                    let types: Vec<_> = fields.iter().map(|field| &field.ty).collect();
                    items.extend(quote! {
                        #[doc = #documentation]
                        #visibility struct #name( #( pub #types, )* );
                    });
                }
                _ => {}
            }
        }
    }
    items
}
