//! `timer_event_<enum>` and `accepts_invoke_<enum>` (spec 7.1, 7.2): the
//! lookups the core calls when a timer fires or an invoke completes. They
//! walk the active configuration from `&self`, compare the key with the epoch
//! of the runtime and build the event from the current data of the state.

use proc_macro2::TokenStream;
use quote::quote;

use super::context::Generation;
use super::naming::{children_function, hygienic_local, payload_binding, state_constant};
use super::runtime::{epoch_field, runtime_name};
use super::tasks::{data_reference, evaluate, has_invokes, has_timers};
use super::walk::{Binding, variant_pattern};
use crate::model::{StateId, StateKind};

/// `timer_event_<enum>` and `accepts_invoke_<enum>` for every children owner
/// with a timer or an invoke below it.
pub fn lookup_functions(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let mut functions = TokenStream::new();
    for node in &tree.nodes {
        if !tree.owns_children(node.id) {
            continue;
        }
        if has_timers_below(generation, node.id) {
            functions.extend(lookup(generation, node.id, Lookup::Timer));
        }
        if has_invokes_below(generation, node.id) {
            functions.extend(lookup(generation, node.id, Lookup::Invoke));
        }
    }
    functions
}

fn has_timers_below(generation: &Generation<'_>, node: StateId) -> bool {
    let tree = generation.tree;
    tree.any_below(node, &|child| !tree.node(child).timers.is_empty())
}

fn has_invokes_below(generation: &Generation<'_>, node: StateId) -> bool {
    let tree = generation.tree;
    tree.any_below(node, &|child| !tree.node(child).invokes.is_empty())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lookup {
    Timer,
    Invoke,
}

fn lookup(generation: &Generation<'_>, owner: StateId, kind: Lookup) -> TokenStream {
    let tree = generation.tree;
    let event = &generation.model.options.event;
    let runtime = runtime_name(generation);
    let enum_name = tree.node(owner).children_enum.as_ref().expect("owner");
    let (prefix, key_type, key_function, documentation) = match kind {
        Lookup::Timer => (
            "timer_event",
            quote! { __estado_core::TimerKey },
            quote! { timer_key },
            "The event of the timer `key`, when it belongs to the active instance.",
        ),
        Lookup::Invoke => (
            "accepts_invoke",
            quote! { __estado_core::InvokeKey },
            quote! { invoke_key },
            "Whether the invoke `key` belongs to the active instance.",
        ),
    };
    let name = children_function(prefix, generation.tree, owner);
    // The data of the state and the user's expressions share this function.
    let children = hygienic_local("children");
    let runtime_parameter = hygienic_local("runtime");
    let key_parameter = hygienic_local("key");
    let (return_type, nothing) = match kind {
        Lookup::Timer => (
            quote! { ::core::option::Option<#event> },
            quote! { ::core::option::Option::None },
        ),
        Lookup::Invoke => (quote! { bool }, quote! { false }),
    };
    let arms = tree.node(owner).children.iter().filter_map(|&child| {
        let state = tree.node(child);
        let composite = matches!(state.kind, StateKind::Compound | StateKind::Parallel);
        let own_count = match kind {
            Lookup::Timer => state.timers.len(),
            Lookup::Invoke => state.invokes.len(),
        };
        let below = composite
            && match kind {
                Lookup::Timer => has_timers_below(generation, child),
                Lookup::Invoke => has_invokes_below(generation, child),
            };
        if own_count == 0 && !below {
            return None;
        }
        let pattern = variant_pattern(generation, child, Binding::Everything);
        let id = state_constant(generation, child);
        let field = epoch_field(generation, child);
        let data = data_reference(generation, child);
        let checks = (0..own_count).map(|index| {
            let number = u8::try_from(index).unwrap_or(u8::MAX);
            let key = quote! { #key_function(#id, #runtime_parameter.#field, #number) };
            match kind {
                Lookup::Timer => {
                    let expression = evaluate(&state.timers[index].event, &data);
                    quote! { if #key_parameter == #key { return ::core::option::Option::Some(#expression); } }
                }
                Lookup::Invoke => quote! { if #key_parameter == #key { return true; } },
            }
        });
        let payload = payload_binding(tree, child);
        let nested = below.then(|| {
            let owners = tree.children_owners(child);
            let calls = owners.iter().filter_map(|&inner| {
                let present = match kind {
                    Lookup::Timer => has_timers_below(generation, inner),
                    Lookup::Invoke => has_invokes_below(generation, inner),
                };
                present.then(|| {
                    let function = children_function(prefix, generation.tree, inner);
                    let field = tree.node(inner).children_field.as_ref().expect("field");
                    quote! { #function(&#payload.#field, #runtime_parameter, #key_parameter) }
                })
            });
            match kind {
                Lookup::Timer => {
                    let calls: Vec<_> = calls.collect();
                    let (first, rest) = calls.split_first().expect("a timer below");
                    quote! { #first #(.or_else(|| #rest))* }
                }
                Lookup::Invoke => {
                    let calls: Vec<_> = calls.collect();
                    quote! { #(#calls)||* }
                }
            }
        });
        let tail = nested.unwrap_or_else(|| nothing.clone());
        Some(quote! { #pattern => { #(#checks)* #tail } })
    });
    quote! {
        #[doc = #documentation]
        #[allow(unreachable_patterns)]
        pub fn #name(#children: &#enum_name, #runtime_parameter: &#runtime, #key_parameter: #key_type) -> #return_type {
            let _ = (#runtime_parameter, #key_parameter);
            match #children {
                #(#arms)*
                _ => #nothing,
            }
        }
    }
}

/// The `timer_event` and `accepts_invoke` methods of the `Spec` impl.
pub fn spec_methods(generation: &Generation<'_>) -> TokenStream {
    let event = &generation.model.options.event;
    let timer = has_timers(generation).then(|| {
        let function = children_function("timer_event", generation.tree, 0);
        quote! {
            fn timer_event(
                &self,
                runtime: &Self::Runtime,
                key: __estado_core::TimerKey,
            ) -> ::core::option::Option<#event> {
                #function(self, runtime, key)
            }
        }
    });
    let invoke = has_invokes(generation).then(|| {
        let function = children_function("accepts_invoke", generation.tree, 0);
        quote! {
            fn accepts_invoke(
                &self,
                runtime: &Self::Runtime,
                key: __estado_core::InvokeKey,
            ) -> bool {
                #function(self, runtime, key)
            }
        }
    });
    quote! { #timer #invoke }
}
