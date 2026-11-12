//! The `impl Spec for Root` (spec 4.1, 6.5).

use proc_macro2::TokenStream;
use quote::quote;

use super::context::Generation;
use super::graph::{path_body, state_id_body};
use super::naming::enter_function;
use crate::construct::initial_value;
use crate::model::{DeferOverflow, UnhandledPolicy};

/// The `Spec` implementation. `initial()` is built from the `#[initial]`
/// chain of the root; missing data was reported as E0020 by the analysis.
pub fn spec_impl(generation: &Generation<'_>) -> TokenStream {
    let root = generation.root;
    let options = &generation.model.options;
    let event = &options.event;
    let effect = &generation.effect;
    let ctx = options.ctx.clone().unwrap_or_else(|| syn::parse_quote!(()));
    let root_name = root.to_string();
    let fx_inline = options.fx_inline;
    let max_microsteps = options.max_microsteps;
    let unhandled = match options.unhandled {
        UnhandledPolicy::Ignore => quote! { Ignore },
        UnhandledPolicy::Error => quote! { Error },
        UnhandledPolicy::Panic => quote! { Panic },
    };
    let defer_overflow = match options.defer_overflow {
        DeferOverflow::Error => quote! { Error },
        DeferOverflow::DropOldest => quote! { DropOldest },
    };
    let tracks_active = generation.tracks_active();
    let initial =
        initial_value(generation.tree, 0).unwrap_or_else(|_| quote! { ::core::unreachable!() });
    let enter_root = enter_function(generation.tree, 0);
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let path = path_body(generation);
    let state_id = state_id_body(generation);
    let runtime = super::runtime::runtime_type(generation);
    let output = super::completion::output_type(generation);
    let done_methods = super::completion::done_methods(generation);
    let run_always = super::completion::has_completion(generation).then(|| {
        quote! {
            fn run_always<#sink_parameter>(
                slot: &mut ::estado_core::Slot<Self>,
                cx: &mut #cx_type,
            ) -> ::estado_core::Outcome {
                run_always(slot, cx)
            }
        }
    });

    quote! {
        impl ::estado_core::Spec for #root {
            type Event = #event;
            type Effect = #effect;
            type Ctx = #ctx;
            type Output = #output;
            type Runtime = #runtime;
            type Effects = ::estado_core::InlineEffects<#effect, #fx_inline>;

            const NAME: &'static str = #root_name;
            const GRAPH: &'static ::estado_core::Graph = &GRAPH;
            const UNHANDLED: ::estado_core::UnhandledPolicy = ::estado_core::UnhandledPolicy::#unhandled;
            const DEFER_OVERFLOW: ::estado_core::DeferOverflowPolicy =
                ::estado_core::DeferOverflowPolicy::#defer_overflow;
            const MAX_MICROSTEPS: u16 = #max_microsteps;
            const TRACKS_ACTIVE: bool = #tracks_active;

            fn initial() -> Self {
                #initial
            }

            fn state_id(&self) -> ::estado_core::StateId {
                #state_id
            }

            fn path(&self) -> ::estado_core::StatePath {
                #path
            }

            fn enter_initial<#sink_parameter>(&mut self, cx: &mut #cx_type) {
                #enter_root(self, cx);
            }

            fn microstep<#sink_parameter>(
                slot: &mut ::estado_core::Slot<Self>,
                cx: &mut #cx_type,
                event: &#event,
            ) -> ::estado_core::Outcome {
                microstep(slot, cx, event)
            }

            #run_always
            #done_methods
        }
    }
}
