//! Expansion of the toy `#[machine]` attribute.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Item, ItemEnum};

use crate::arguments::{self, MachineArguments};
use crate::warning::{self, Warning, WarningCode};

pub fn expand(arguments: TokenStream, item: TokenStream) -> TokenStream {
    let parsed_arguments = match arguments::parse(arguments) {
        Ok(parsed) => parsed,
        // Best effort (spec 6.6): keep the item, add the error next to it.
        Err(error) => {
            let compile_error = error.to_compile_error();
            return quote! { #item #compile_error };
        }
    };
    match syn::parse2::<Item>(item.clone()) {
        Ok(Item::Enum(item_enum)) => expand_enum(item_enum, &parsed_arguments),
        _ => {
            let message = "E0001: `#[machine]` outside an enum";
            quote! { #item ::core::compile_error!(#message); }
        }
    }
}

fn expand_enum(mut item_enum: ItemEnum, arguments: &MachineArguments) -> TokenStream {
    let module = format_ident!("__estado_{}", item_enum.ident);
    let mut definitions = Vec::new();
    let mut usages = Vec::new();

    for variant in &mut item_enum.variants {
        let marker_count = variant.attrs.len();
        variant
            .attrs
            .retain(|attribute| !attribute.path().is_ident("unreachable_demo"));
        let is_unreachable = variant.attrs.len() != marker_count;
        if !is_unreachable || arguments.allows(WarningCode::UnreachableState) {
            continue;
        }
        let state = variant.ident.to_string();
        let warning = Warning {
            code: WarningCode::UnreachableState,
            message: format!(
                "state `{state}` is unreachable from the initial state \
                 (silence with `#[machine(allow(W0001))]`)"
            ),
            subject: state,
            span: variant.ident.span(),
        };
        let tokens = warning::emit(&warning, arguments.strategy, arguments.span_source, &module);
        definitions.push(tokens.definition);
        usages.push(tokens.usage);
    }

    if definitions.is_empty() {
        return quote! { #item_enum };
    }
    quote! {
        #item_enum

        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #module {
            #(#definitions)*
        }

        const _: () = { #(#usages)* };
    }
}
