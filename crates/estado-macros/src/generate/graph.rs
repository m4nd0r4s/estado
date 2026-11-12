//! The static graph (spec 8.1) and the functions that read the active
//! configuration: `path()` and `state_id()`.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;

use super::context::Generation;
use super::naming::{payload_binding, state_constant};
use super::walk::{Binding, variant_pattern};
use crate::model::{HandlerRole, HistoryKind, StateId, StateKind, TransitionKind};

/// `STATES`, `EDGES` and `GRAPH`.
pub fn graph_items(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let root_name = generation.root.to_string();
    let count = tree.nodes.len() - 1;
    let states = tree.nodes.iter().skip(1).map(|node| {
        let id = state_constant(generation, node.id);
        let name = crate::naming::plain_name(&node.name);
        let parent = match node.parent {
            Some(parent) if parent != 0 => {
                let parent = state_constant(generation, parent);
                quote! { ::core::option::Option::Some(#parent) }
            }
            _ => quote! { ::core::option::Option::None },
        };
        let kind = match node.kind {
            _ if node.is_final => quote! { Final },
            StateKind::Root | StateKind::Atomic => quote! { Atomic },
            StateKind::Compound => quote! { Compound },
            StateKind::Parallel => quote! { Parallel },
            StateKind::Region => quote! { Region },
        };
        let depth = node.depth;
        let initial = node.initial.map(|initial| {
            let initial = state_constant(generation, initial);
            quote! { .with_initial(#initial) }
        });
        let history = match node.history {
            HistoryKind::None => None,
            HistoryKind::Shallow => Some(quote! { .with_history(__estado_core::HistoryKind::Shallow) }),
            HistoryKind::DeepKeep => Some(quote! { .with_history(__estado_core::HistoryKind::DeepKeep) }),
        };
        let tasks = (!node.timers.is_empty() || !node.invokes.is_empty()).then(|| {
            let timers = u8::try_from(node.timers.len()).expect("guarded by E0090: at most 255 timers per state");
            let invokes = u8::try_from(node.invokes.len()).expect("guarded by E0090: at most 255 invokes per state");
            quote! { .with_tasks(#timers, #invokes) }
        });
        quote! {
            __estado_core::StateInfo::new(#id, #name, #parent, __estado_core::StateKind::#kind, #depth)
                #initial #history #tasks
        }
    });

    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    for handler in &generation.model.analysed_handlers {
        for goto in &handler.gotos {
            let from = state_constant(generation, handler.state);
            let to = state_constant(generation, goto.target);
            let is_timer = handler.role == HandlerRole::On
                && goto.event_hint.as_ref().is_some_and(|hint| {
                    generation
                        .tree
                        .node(handler.state)
                        .timers
                        .iter()
                        .any(|timer| timer_event_hint(&timer.event).as_ref() == Some(hint))
                });
            let kind = match (handler.role, goto.kind) {
                (HandlerRole::Always, _) => quote! { Always },
                (HandlerRole::OnDone, _) => quote! { Done },
                (_, TransitionKind::Normal) if is_timer => quote! { Timer },
                (_, TransitionKind::External) => quote! { External },
                (_, TransitionKind::History) => quote! { History },
                (_, TransitionKind::Normal) => quote! { Normal },
            };
            let hint = goto.event_hint.as_ref().map_or_else(
                || quote! { ::core::option::Option::None },
                |hint| quote! { ::core::option::Option::Some(#hint) },
            );
            // Two edges are the same only when everything the graph says about
            // them is: `goto!(T)` and `goto!(T, history)` in one arm are two.
            if seen.insert((
                handler.state,
                goto.target,
                goto.event_hint.clone(),
                handler.role,
                goto.kind,
            )) {
                edges.push(quote! {
                    __estado_core::EdgeInfo {
                        from: #from,
                        to: __estado_core::EdgeTarget::State(#to),
                        kind: __estado_core::EdgeKind::#kind,
                        event_hint: #hint,
                    }
                });
            }
        }
    }
    quote! {
        static STATES: [__estado_core::StateInfo; #count] = [ #(#states),* ];

        /// The static graph: the exact union of the edges of the handlers (spec 6.3).
        pub static GRAPH: __estado_core::Graph = __estado_core::Graph {
            machine: #root_name,
            states: &STATES,
            edges: &[ #(#edges),* ],
        };
    }
}

/// The event a timer fires, as the text of its path (`Event::Retry`), when
/// the expression is a path, a call of one, or a closure returning either:
/// the same text the handler's `match` arm has as `event_hint` (spec 8.1).
fn timer_event_hint(expression: &syn::Expr) -> Option<String> {
    let path = match expression {
        syn::Expr::Path(inner) => &inner.path,
        syn::Expr::Call(call) => match &*call.func {
            syn::Expr::Path(inner) => &inner.path,
            _ => return None,
        },
        syn::Expr::Closure(closure) => return timer_event_hint(&closure.body),
        syn::Expr::Paren(inner) => return timer_event_hint(&inner.expr),
        _ => return None,
    };
    Some(
        path.segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
    )
}

/// The body of `Spec::path`: pushes the active states, top down.
pub fn path_body(generation: &Generation<'_>) -> TokenStream {
    let arms = path_arms(generation, 0);
    quote! {
        let mut path = __estado_core::StatePath::new(&GRAPH);
        match self { #arms }
        path
    }
}

fn path_arms(generation: &Generation<'_>, owner: StateId) -> TokenStream {
    let tree = generation.tree;
    let arms = tree.node(owner).children.iter().map(|&child| {
        let id = state_constant(generation, child);
        match tree.node(child).kind {
            StateKind::Compound => {
                let pattern = variant_pattern(generation, child, Binding::Everything);
                let payload = payload_binding(tree, child);
                let field = tree.node(child).children_field.as_ref().expect("substates");
                let inner = path_arms_over(generation, child, quote! { &#payload.#field });
                quote! { #pattern => { path.push(#id); #inner } }
            }
            StateKind::Parallel => {
                let pattern = variant_pattern(generation, child, Binding::Everything);
                let payload = payload_binding(tree, child);
                let regions = tree.node(child).children.iter().map(|&region| {
                    let region_id = state_constant(generation, region);
                    let field = tree.node(region).children_field.as_ref().expect("region");
                    let inner = path_arms_over(generation, region, quote! { &#payload.#field });
                    quote! { path.push(#region_id); #inner }
                });
                quote! { #pattern => { path.push(#id); #(#regions)* } }
            }
            _ => {
                let pattern = variant_pattern(generation, child, Binding::Nothing);
                quote! { #pattern => path.push(#id), }
            }
        }
    });
    quote! { #(#arms)* }
}

fn path_arms_over(
    generation: &Generation<'_>,
    owner: StateId,
    scrutinee: TokenStream,
) -> TokenStream {
    let arms = path_arms(generation, owner);
    quote! { match #scrutinee { #arms } }
}

/// The body of `Spec::state_id`: the id of the active leaf.
pub fn state_id_body(generation: &Generation<'_>) -> TokenStream {
    let arms = leaf_arms(generation, 0);
    quote! { match self { #arms } }
}

pub fn leaf_arms(generation: &Generation<'_>, owner: StateId) -> TokenStream {
    let tree = generation.tree;
    let arms = tree.node(owner).children.iter().map(|&child| {
        if tree.node(child).kind == StateKind::Compound {
            let pattern = variant_pattern(generation, child, Binding::Everything);
            let payload = payload_binding(tree, child);
            let field = tree.node(child).children_field.as_ref().expect("substates");
            let inner = leaf_arms(generation, child);
            quote! { #pattern => match &#payload.#field { #inner }, }
        } else {
            let id = state_constant(generation, child);
            let pattern = variant_pattern(generation, child, Binding::Nothing);
            quote! { #pattern => #id, }
        }
    });
    quote! { #(#arms)* }
}
