//! Procedural macros for `estado`. Use them through the `estado` facade.
//!
//! `#[machine(..)]` is applied to an inline module holding the whole machine
//! (ADR 0010); the other attributes are markers it consumes.

mod analysis;
mod construct;
mod core_crate;
mod expand;
mod generate;
mod handlers;
mod model;
mod naming;
mod parse;
mod suggest;
mod warnings;

use proc_macro::TokenStream;

/// Declares a state machine: an inline module with one `#[root]` enum,
/// `#[state]` types and `#[handlers]` impl blocks.
///
/// # Feature detection
///
/// The macro finds `cx.raise(..)`, `defer!()` and `cx.in_state(..)` by
/// looking for them in the machine module, and generates the queues and the
/// active-configuration snapshot only when it sees them. A helper function
/// outside the module that calls `cx.raise` or `cx.in_state`, or a call
/// written `Cx::raise(cx, ..)`, is invisible to that scan: switch the feature
/// on by hand with `#[machine(tracks_active)]` (for `cx.in_state`) and with an
/// explicit `raise_cap = N` or `defer_cap = N` (for the queues).
#[proc_macro_attribute]
pub fn machine(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand::machine(attribute.into(), item.into()).into()
}

/// Marks a state type. Only meaningful inside a `#[machine]` module.
#[proc_macro_attribute]
pub fn state(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand::outside_machine("state", item.into()).into()
}

/// Marks the root enum. Only meaningful inside a `#[machine]` module.
#[proc_macro_attribute]
pub fn root(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand::outside_machine("root", item.into()).into()
}

/// Marks an impl block of handlers. Only meaningful inside a `#[machine]` module.
#[proc_macro_attribute]
pub fn handlers(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand::outside_machine("handlers", item.into()).into()
}
