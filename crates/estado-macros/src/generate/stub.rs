//! A placeholder `Spec` impl for machines with errors (spec 6.6): the user's
//! handlers mention `Cx<Self>`, which needs `Self: Spec`; without an impl
//! every diagnostic of the macro would be buried under trait-bound errors.

use proc_macro2::TokenStream;
use quote::quote;

use crate::model::MachineModel;

/// The stub, when the machine has handler functions that would otherwise
/// fail to type-check.
pub fn stub_spec(model: &MachineModel) -> TokenStream {
    if model.handlers.iter().all(|block| block.items.is_empty()) {
        return TokenStream::new();
    }
    let root = &model.tree.root().name;
    let options = &model.options;
    let event = &options.event;
    let effect = options
        .effect
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(()));
    let ctx = options.ctx.clone().unwrap_or_else(|| syn::parse_quote!(()));
    let name = root.to_string();
    quote! {
        #[doc(hidden)]
        #[allow(missing_docs, unreachable_code, unused_variables)]
        const _: () = {
            impl ::estado_core::Spec for #root {
                type Event = #event;
                type Effect = #effect;
                type Ctx = #ctx;
                type Output = ();
                type Runtime = ::estado_core::NoRuntime<#event>;
                type Effects = ::estado_core::InlineEffects<#effect, 1>;

                const NAME: &'static str = #name;
                const GRAPH: &'static ::estado_core::Graph = &::estado_core::Graph {
                    machine: #name,
                    states: &[],
                    edges: &[],
                };

                fn initial() -> Self { ::core::unreachable!() }
                fn state_id(&self) -> ::estado_core::StateId { ::core::unreachable!() }
                fn path(&self) -> ::estado_core::StatePath { ::core::unreachable!() }
                fn enter_initial<EffectSink: ::estado_core::Sink<#effect>>(
                    &mut self,
                    cx: &mut ::estado_core::Cx<'_, Self, EffectSink>,
                ) {
                    ::core::unreachable!()
                }
                fn microstep<EffectSink: ::estado_core::Sink<#effect>>(
                    slot: &mut ::estado_core::Slot<Self>,
                    cx: &mut ::estado_core::Cx<'_, Self, EffectSink>,
                    event: &#event,
                ) -> ::estado_core::Outcome {
                    ::core::unreachable!()
                }
            }
        };
    }
}
