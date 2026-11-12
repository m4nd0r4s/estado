//! The `Runtime` of the machine: only what the machine uses (ADR 0011).
//!
//! | Field | When |
//! | --- | --- |
//! | `queues` | a handler calls `cx.raise` or `defer!()` |
//! | `<state>_history` | the state has `#[history]` (`Option<u8>`) or `#[history(deep, keep)]` (`Option<Substates>`) |
//! | `<state>_epoch` | the state has `#[after]` or `#[invoke]` |
//! | `completion_pending` | the machine has a final state below the root or an `#[on_done]` |
//! | `<state>_always_pending` | the state has an `#[always]` handler |
//!
//! With none of them the runtime is `NoRuntime<Event>` and `Machine` keeps
//! its AC-15 size.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use super::context::Generation;
use crate::model::{HandlerRole, HistoryKind, StateId};
use crate::naming::snake_case_name;

/// What the runtime holds, derived from the model.
pub struct RuntimePlan {
    /// Raise and defer queues.
    pub queues: bool,
    /// States with history, in tree order.
    pub histories: Vec<StateId>,
    /// States with timers or invokes, in tree order.
    pub epochs: Vec<StateId>,
    /// Completion flag.
    pub completion: bool,
    /// States with an `#[always]` handler, in tree order.
    pub always_states: Vec<StateId>,
}

impl RuntimePlan {
    /// Whether the machine needs a generated runtime at all.
    pub fn is_empty(&self) -> bool {
        !self.queues
            && self.histories.is_empty()
            && self.epochs.is_empty()
            && !self.completion
            && self.always_states.is_empty()
    }
}

/// Derives the plan.
pub fn runtime_plan(generation: &Generation<'_>) -> RuntimePlan {
    let tree = generation.tree;
    let queues = generation.uses_queues();
    let histories = tree
        .nodes
        .iter()
        .filter(|node| node.history != HistoryKind::None)
        .map(|node| node.id)
        .collect();
    let epochs = tree
        .nodes
        .iter()
        .filter(|node| !node.timers.is_empty() || !node.invokes.is_empty())
        .map(|node| node.id)
        .collect();
    RuntimePlan {
        queues,
        histories,
        epochs,
        completion: generation.needs_completion(),
        always_states: generation
            .model
            .analysed_handlers
            .iter()
            .filter(|handler| handler.role == HandlerRole::Always)
            .map(|handler| handler.state)
            .collect(),
    }
}

/// `PlayerRuntime`.
pub fn runtime_name(generation: &Generation<'_>) -> Ident {
    format_ident!("{}Runtime", generation.root)
}

/// `active_history`: the field of the history of a compound state.
pub fn history_field(generation: &Generation<'_>, node: StateId) -> Ident {
    format_ident!(
        "{}_history",
        snake_case_name(&generation.tree.node(node).name)
    )
}

/// `connecting_epoch`.
pub fn epoch_field(generation: &Generation<'_>, node: StateId) -> Ident {
    format_ident!(
        "{}_epoch",
        snake_case_name(&generation.tree.node(node).name)
    )
}

/// `awaiting_always_pending`: set while the `#[always]` of the state is due.
pub fn always_field(generation: &Generation<'_>, node: StateId) -> Ident {
    format_ident!(
        "{}_always_pending",
        snake_case_name(&generation.tree.node(node).name)
    )
}

/// The `Runtime` type: `NoRuntime<Event>` when nothing needs auxiliary state.
pub fn runtime_type(generation: &Generation<'_>) -> TokenStream {
    if runtime_plan(generation).is_empty() {
        let event = &generation.model.options.event;
        return quote! { ::estado_core::NoRuntime<#event> };
    }
    let name = runtime_name(generation);
    quote! { #name }
}

/// The runtime struct and its `Runtime` impl; nothing when unused.
pub fn runtime_items(generation: &Generation<'_>) -> TokenStream {
    let plan = runtime_plan(generation);
    if plan.is_empty() {
        return TokenStream::new();
    }
    let tree = generation.tree;
    let options = &generation.model.options;
    let event = &options.event;
    let name = runtime_name(generation);
    let raise_cap = options.raise_cap;
    let defer_cap = options.defer_cap;

    let queues_type = if plan.queues {
        quote! { ::estado_core::Queues<#event, #raise_cap, #defer_cap> }
    } else {
        quote! { ::estado_core::NoRuntime<#event> }
    };
    let queues_new = if plan.queues {
        quote! { ::estado_core::Queues::new() }
    } else {
        quote! { <::estado_core::NoRuntime<#event> as ::estado_core::Runtime>::NEW }
    };

    let mut fields = Vec::new();
    let mut initial = Vec::new();
    for &node in &plan.histories {
        let field = history_field(generation, node);
        let state = tree.node(node);
        let (field_type, documentation) = match state.history {
            HistoryKind::DeepKeep => {
                let children = state.children_enum.as_ref().expect("a compound");
                (
                    quote! { ::core::option::Option<#children> },
                    format!(
                        "Deep history of `{}`: the substate value when it last exited.",
                        state.name
                    ),
                )
            }
            _ => (
                quote! { ::core::option::Option<u8> },
                format!(
                    "Shallow history of `{}`: the discriminant of its last substate.",
                    state.name
                ),
            ),
        };
        fields.push(quote! { #[doc = #documentation] pub #field: #field_type });
        initial.push(quote! { #field: ::core::option::Option::None });
    }
    for &node in &plan.epochs {
        let field = epoch_field(generation, node);
        let documentation = format!(
            "Epoch of `{}`: incremented on every entry.",
            tree.node(node).name
        );
        fields.push(quote! { #[doc = #documentation] pub #field: u32 });
        initial.push(quote! { #field: 0 });
    }
    if plan.completion {
        fields.push(quote! { #[doc = "A completion check (`#[on_done]`) is pending."] pub completion_pending: bool });
        initial.push(quote! { completion_pending: false });
    }
    let mut pending = Vec::new();
    if plan.completion {
        pending.push(quote! { self.completion_pending });
    }
    for &node in &plan.always_states {
        let field = always_field(generation, node);
        let documentation = format!("The `#[always]` of `{}` is due.", tree.node(node).name);
        fields.push(quote! { #[doc = #documentation] pub #field: bool });
        initial.push(quote! { #field: false });
        pending.push(quote! { self.#field });
    }
    // The run clears the flag it serves; this only reports whether one is set.
    let take_always = (!pending.is_empty()).then(|| {
        quote! {
            fn take_always_pending(&mut self) -> bool {
                #(#pending)||*
            }
        }
    });
    let documentation = format!(
        "Auxiliary state of `{}`: only what the machine uses.",
        generation.root
    );
    quote! {
        #[doc = #documentation]
        pub struct #name {
            #[doc = "Raise, defer and requeue queues."]
            pub queues: #queues_type,
            #(#fields,)*
        }

        impl ::estado_core::Runtime for #name {
            type Event = #event;
            type Queues = #queues_type;
            const NEW: Self = Self { queues: #queues_new, #(#initial,)* };

            fn queues(&mut self) -> &mut Self::Queues {
                &mut self.queues
            }

            #take_always
        }
    }
}
