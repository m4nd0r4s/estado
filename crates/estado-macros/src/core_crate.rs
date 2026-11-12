//! Where the generated code finds `estado-core` (ADR 0013, item 1).
//!
//! A crate that depends on the `estado` facade does not necessarily depend on
//! `estado-core`, so the generated code goes through the facade's hidden
//! `__private::core` re-export when the facade is there, and through
//! `estado_core` otherwise (the machine tests of this workspace).

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

/// Name of the alias every machine module gets for the core crate; the
/// generated code spells `__estado_core::Next`, never `::estado_core::Next`.
pub const CORE_ALIAS: &str = "__estado_core";

/// The `use` that defines [`CORE_ALIAS`] in the machine module.
pub fn core_alias_item() -> TokenStream {
    let alias = Ident::new(CORE_ALIAS, Span::call_site());
    let path = core_path();
    quote! {
        #[doc(hidden)]
        #[allow(unused_imports)]
        use #path as #alias;
    }
}

/// `::estado::__private::core` when the crate being compiled depends on the
/// facade (under whatever name), else `::estado_core`.
fn core_path() -> TokenStream {
    let facade = match crate_name("estado") {
        Ok(FoundCrate::Itself) => Some("estado".to_owned()),
        Ok(FoundCrate::Name(name)) => Some(name),
        Err(_) => None,
    };
    match facade.and_then(|name| syn::parse_str::<Ident>(&name).ok()) {
        Some(name) => quote! { ::#name::__private::core },
        None => quote! { ::estado_core },
    }
}
