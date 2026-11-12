//! Where the generated code finds `estado-core` (ADR 0013, item 1).
//!
//! A crate that depends on the `estado` facade does not necessarily depend on
//! `estado-core`, so the generated code goes through the facade's hidden
//! `__private::core` re-export when the facade is there, and through
//! `estado-core` (under the name the crate gave it) otherwise (the machine
//! tests of this workspace).

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

/// Where the core crate is reachable from the crate being compiled:
///
/// - the crate depends on the `estado` facade (under whatever name):
///   `::<name>::__private::core`;
/// - it is the `estado` package itself (its own library, which cannot name
///   itself with `::estado`): `::estado_core`, a dependency the package always
///   has;
/// - it depends on `estado-core` directly, possibly renamed
///   (`ec = { package = "estado-core" }`): `::<name>`;
/// - none of them is found: `::estado_core`.
///
/// A crate that has `estado-core` and `estado-macros` as normal dependencies
/// and `estado` only as a dev-dependency still resolves to the facade, which
/// its library build does not have: `proc-macro-crate` cannot tell the kinds
/// of dependency apart (documented on `#[machine]` and in ADR 0013).
fn core_path() -> TokenStream {
    match crate_name("estado") {
        Ok(FoundCrate::Name(name)) => return through_facade(&name),
        Ok(FoundCrate::Itself) => return quote! { ::estado_core },
        Err(_) => {}
    }
    match crate_name("estado-core") {
        Ok(FoundCrate::Name(name)) => direct(&name),
        Ok(FoundCrate::Itself) | Err(_) => quote! { ::estado_core },
    }
}

fn through_facade(name: &str) -> TokenStream {
    match syn::parse_str::<Ident>(name) {
        Ok(name) => quote! { ::#name::__private::core },
        Err(_) => quote! { ::estado_core },
    }
}

fn direct(name: &str) -> TokenStream {
    match syn::parse_str::<Ident>(name) {
        Ok(name) => quote! { ::#name },
        Err(_) => quote! { ::estado_core },
    }
}
