//! Toy `#[machine]` attribute used to prove that a stable proc-macro can emit
//! rustc warnings through `#[deprecated]` items.

mod arguments;
mod expansion;
mod warning;

use proc_macro::TokenStream;

/// Toy attribute for an enum. Variants marked `#[unreachable_demo]` produce a
/// W0001 warning unless `#[machine(allow(W0001))]` is given.
///
/// Spike-only argument: `strategy = "const" | "fn" | "struct"` selects the kind
/// of deprecated item (default: the chosen best variant).
#[proc_macro_attribute]
pub fn machine(arguments: TokenStream, item: TokenStream) -> TokenStream {
    expansion::expand(arguments.into(), item.into()).into()
}
