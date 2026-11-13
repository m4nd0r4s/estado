//! The `impl Spec for Root` (spec 4.1, 6.5).

use proc_macro2::TokenStream;
use quote::quote;

use super::context::Generation;
use super::graph::{path_body, state_id_body};
use super::naming::enter_function;
use crate::construct::initial_value;
use crate::construct_context::{initial_value_in, needs_context};
use crate::model::{DeferOverflow, UnhandledPolicy};

/// The `Spec` implementation. `initial()` is built from the `#[initial]`
/// chain of the root; missing data was reported as E0020 by the analysis.
pub fn spec_impl(generation: &Generation<'_>) -> TokenStream {
    let root = generation.root;
    let options = &generation.model.options;
    let event = &options.event;
    let effect = &generation.effect;
    let ctx = options.ctx_type();
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
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    // An initializer that takes the `Cx` cannot run in `initial()`: the
    // machine builds its root through `initial_in` (errata 15).
    let (initial_function, initial_in_function) = if needs_context(generation.tree, 0) {
        let initial = initial_value_in(generation.tree, 0, None)
            .unwrap_or_else(|_| quote! { ::core::unreachable!() });
        (
            quote! {
                fn initial() -> Self {
                    ::core::panic!(
                        "estado: the initial state of `{}` is built with a `Cx`; start the machine with `Machine::start` or `send`",
                        <Self as __estado_core::Spec>::NAME
                    )
                }
            },
            quote! {
                fn initial_in<#sink_parameter>(cx: &mut #cx_type) -> Self {
                    #initial
                }
            },
        )
    } else {
        let initial =
            initial_value(generation.tree, 0).unwrap_or_else(|_| quote! { ::core::unreachable!() });
        (
            quote! {
                fn initial() -> Self {
                    #initial
                }
            },
            TokenStream::new(),
        )
    };
    let enter_root = enter_function(generation.tree, 0);
    let start_root = super::tasks::start_call(
        generation,
        0,
        &proc_macro2::Ident::new("self", proc_macro2::Span::call_site()),
    );
    let path = path_body(generation);
    let state_id = state_id_body(generation);
    let runtime = super::runtime::runtime_type(generation);
    let output = super::completion::output_type(generation);
    let done_methods = super::completion::done_methods(generation);
    let task_methods = super::lookups::spec_methods(generation);
    let run_always = super::always::has_run_always(generation).then(|| {
        quote! {
            fn run_always<#sink_parameter>(
                slot: &mut __estado_core::Slot<Self>,
                cx: &mut #cx_type,
            ) -> __estado_core::Outcome {
                run_always(slot, cx)
            }
        }
    });

    quote! {
        impl __estado_core::Spec for #root {
            type Event = #event;
            type Effect = #effect;
            type Ctx = #ctx;
            type Output = #output;
            type Runtime = #runtime;
            type Effects = __estado_core::InlineEffects<#effect, #fx_inline>;

            const NAME: &'static str = #root_name;
            const GRAPH: &'static __estado_core::Graph = &GRAPH;
            const UNHANDLED: __estado_core::UnhandledPolicy = __estado_core::UnhandledPolicy::#unhandled;
            const DEFER_OVERFLOW: __estado_core::DeferOverflowPolicy =
                __estado_core::DeferOverflowPolicy::#defer_overflow;
            const MAX_MICROSTEPS: u16 = #max_microsteps;
            const TRACKS_ACTIVE: bool = #tracks_active;

            #initial_function
            #initial_in_function

            fn state_id(&self) -> __estado_core::StateId {
                #state_id
            }

            fn path(&self) -> __estado_core::StatePath {
                #path
            }

            fn enter_initial<#sink_parameter>(&mut self, cx: &mut #cx_type) {
                #enter_root(self, cx);
                #start_root
            }

            #[inline(always)]
            fn microstep<#sink_parameter>(
                slot: &mut __estado_core::Slot<Self>,
                cx: &mut #cx_type,
                event: &#event,
            ) -> __estado_core::Outcome {
                microstep(slot, cx, event)
            }

            #run_always
            #done_methods
            #task_methods
        }
    }
}
