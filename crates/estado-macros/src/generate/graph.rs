//! The static graph (spec 8.1) and the functions that read the active
//! configuration: `path()` and `state_id()`.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::context::Generation;
use super::naming::payload_binding;
use super::walk::{Binding, variant_pattern};
use crate::model::{HandlerRole, HistoryKind, StateId, StateKind, TransitionKind};
use crate::naming::state_constant_name;

/// `Root::PLAYING`.
fn constant(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let root = generation.root;
    let name = format_ident!("{}", state_constant_name(&generation.tree.node(node).name));
    quote! { #root::#name }
}

/// `STATES`, `EDGES` and `GRAPH`.
pub fn graph_items(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let root_name = generation.root.to_string();
    let count = tree.nodes.len() - 1;
    let states = tree.nodes.iter().skip(1).map(|node| {
        let id = constant(generation, node.id);
        let name = node.name.to_string();
        let parent = match node.parent {
            Some(parent) if parent != 0 => {
                let parent = constant(generation, parent);
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
            let initial = constant(generation, initial);
            quote! { .with_initial(#initial) }
        });
        let history = match node.history {
            HistoryKind::None => None,
            HistoryKind::Shallow => Some(quote! { .with_history(::estado_core::HistoryKind::Shallow) }),
            HistoryKind::DeepKeep => Some(quote! { .with_history(::estado_core::HistoryKind::DeepKeep) }),
        };
        let tasks = (!node.timers.is_empty() || !node.invokes.is_empty()).then(|| {
            let timers = u8::try_from(node.timers.len()).unwrap_or(u8::MAX);
            let invokes = u8::try_from(node.invokes.len()).unwrap_or(u8::MAX);
            quote! { .with_tasks(#timers, #invokes) }
        });
        quote! {
            ::estado_core::StateInfo::new(#id, #name, #parent, ::estado_core::StateKind::#kind, #depth)
                #initial #history #tasks
        }
    });

    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    for handler in &generation.model.analysed_handlers {
        for goto in &handler.gotos {
            let from = constant(generation, handler.state);
            let to = constant(generation, goto.target);
            let kind = match (handler.role, goto.kind) {
                (HandlerRole::Always, _) => quote! { Always },
                (HandlerRole::OnDone, _) => quote! { Done },
                (_, TransitionKind::External) => quote! { External },
                (_, TransitionKind::History) => quote! { History },
                (_, TransitionKind::Normal) => quote! { Normal },
            };
            let hint = goto.event_hint.as_ref().map_or_else(
                || quote! { ::core::option::Option::None },
                |hint| quote! { ::core::option::Option::Some(#hint) },
            );
            if seen.insert((
                handler.state,
                goto.target,
                goto.event_hint.clone(),
                goto.kind == TransitionKind::External,
            )) {
                edges.push(quote! {
                    ::estado_core::EdgeInfo {
                        from: #from,
                        to: ::estado_core::EdgeTarget::State(#to),
                        kind: ::estado_core::EdgeKind::#kind,
                        event_hint: #hint,
                    }
                });
            }
        }
    }
    quote! {
        static STATES: [::estado_core::StateInfo; #count] = [ #(#states),* ];

        /// The static graph: the exact union of the edges of the handlers (spec 6.3).
        pub static GRAPH: ::estado_core::Graph = ::estado_core::Graph {
            machine: #root_name,
            states: &STATES,
            edges: &[ #(#edges),* ],
        };
    }
}

/// The body of `Spec::path`: pushes the active states, top down.
pub fn path_body(generation: &Generation<'_>) -> TokenStream {
    let arms = path_arms(generation, 0);
    quote! {
        let mut path = ::estado_core::StatePath::new(&GRAPH);
        match self { #arms }
        path
    }
}

fn path_arms(generation: &Generation<'_>, owner: StateId) -> TokenStream {
    let tree = generation.tree;
    let arms = tree.node(owner).children.iter().map(|&child| {
        let id = constant(generation, child);
        if tree.node(child).kind == StateKind::Compound {
            let pattern = variant_pattern(generation, child, Binding::Everything);
            let payload = payload_binding(tree, child);
            let field = tree.node(child).children_field.as_ref().expect("substates");
            let inner = path_arms_over(generation, child, quote! { &#payload.#field });
            quote! { #pattern => { path.push(#id); #inner } }
        } else {
            let pattern = variant_pattern(generation, child, Binding::Nothing);
            quote! { #pattern => path.push(#id), }
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

fn leaf_arms(generation: &Generation<'_>, owner: StateId) -> TokenStream {
    let tree = generation.tree;
    let arms = tree.node(owner).children.iter().map(|&child| {
        if tree.node(child).kind == StateKind::Compound {
            let pattern = variant_pattern(generation, child, Binding::Everything);
            let payload = payload_binding(tree, child);
            let field = tree.node(child).children_field.as_ref().expect("substates");
            let inner = leaf_arms(generation, child);
            quote! { #pattern => match &#payload.#field { #inner }, }
        } else {
            let id = constant(generation, child);
            let pattern = variant_pattern(generation, child, Binding::Nothing);
            quote! { #pattern => #id, }
        }
    });
    quote! { #(#arms)* }
}
