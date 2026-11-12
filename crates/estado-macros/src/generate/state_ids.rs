//! `impl Root { pub const PLAYING: StateId = ..; }` (spec 6.5).

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::model::{MachineModel, TypeDecl};
use crate::naming::state_constant_name;

/// One associated constant per state and region, valued with the runtime id
/// (index in the tree minus the root). A name that clashes with an earlier
/// one is skipped: E0012 already reports it.
pub fn state_id_constants(model: &MachineModel) -> TokenStream {
    let tree = &model.tree;
    let root_name = &tree.root().name;
    let generics = tree.types.iter().find_map(|declaration| match declaration {
        TypeDecl::Enum(item) if item.ident == *root_name => Some(&item.generics),
        _ => None,
    });
    let Some(generics) = generics else {
        return TokenStream::new();
    };
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let mut emitted = HashSet::new();
    let constants = tree.nodes.iter().skip(1).filter_map(|node| {
        let constant_name = state_constant_name(&node.name);
        if !emitted.insert(constant_name.clone()) {
            return None;
        }
        let constant = format_ident!("{}", constant_name, span = node.name.span());
        let documentation = format!("Id of the state `{}`.", node.name);
        let runtime_id = tree.runtime_id(node.id);
        Some(quote! {
            #[doc = #documentation]
            pub const #constant: __estado_core::StateId = __estado_core::StateId::new(#runtime_id);
        })
    });
    quote! {
        impl #impl_generics #root_name #type_generics #where_clause {
            #(#constants)*
        }
    }
}
