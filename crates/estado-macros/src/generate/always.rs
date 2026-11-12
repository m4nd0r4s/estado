//! `#[always]` (spec 5.9) and the pending evaluations of the runtime:
//! `run_always` serves, one per call, the completion check of spec 5.10 and
//! the `#[always]` handlers whose state entered (their flag is set by the
//! entry and cleared by the exit, see `tasks`).

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::context::Generation;
use super::runtime::always_field;
use super::walk::{FieldsAccess, call_handler, state_function};
use crate::model::{HandlerRole, StateId};
use crate::naming::snake_case_name;

/// The states with an `#[always]` handler, deepest first: an inner state is
/// evaluated before the compound state that holds it.
fn always_states(generation: &Generation<'_>) -> Vec<StateId> {
    let tree = generation.tree;
    let mut states: Vec<StateId> = generation
        .model
        .analysed_handlers
        .iter()
        .filter(|handler| handler.role == HandlerRole::Always)
        .map(|handler| handler.state)
        .collect();
    states.sort_by_key(|&node| std::cmp::Reverse((tree.node(node).depth, node)));
    states
}

/// `always_playing`.
fn always_function(generation: &Generation<'_>, node: StateId) -> proc_macro2::Ident {
    format_ident!(
        "always_{}",
        snake_case_name(&generation.tree.node(node).name)
    )
}

/// Whether `run_always` exists: a completion check or an `#[always]`.
pub fn has_run_always(generation: &Generation<'_>) -> bool {
    generation.needs_completion() || generation.has_always()
}

/// `run_always` and one `always_<state>` function per `#[always]` handler.
pub fn always_items(generation: &Generation<'_>) -> TokenStream {
    if !has_run_always(generation) {
        return TokenStream::new();
    }
    let root = generation.root;
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let functions = always_states(generation)
        .into_iter()
        .map(|node| always_state_function(generation, node));
    let completions = super::completion::serve_completions(generation);
    let due = always_states(generation).into_iter().map(|node| {
        let field = always_field(generation, node);
        let function = always_function(generation, node);
        quote! {
            if ::core::mem::take(&mut cx.runtime_mut().#field) {
                return #function(slot, cx);
            }
        }
    });
    quote! {
        #(#functions)*

        /// One pending evaluation: the completion check first, then the
        /// `#[always]` of the innermost state that is due (spec 5.9, 5.10).
        pub fn run_always<#sink_parameter>(
            slot: &mut __estado_core::Slot<#root>,
            cx: &mut #cx_type,
        ) -> __estado_core::Outcome {
            #completions
            #(#due)*
            __estado_core::Outcome::Stayed
        }
    }
}

/// Runs the `#[always]` handler of one state and resolves its `Next`. Only
/// `stay!()` and `goto!` are allowed there (E0045), so nothing else bubbles.
fn always_state_function(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let tree = generation.tree;
    let handler = generation
        .handler(HandlerRole::Always, node)
        .expect("an always state has a handler");
    let call = call_handler(generation, handler, FieldsAccess::Binding, None);
    let bubble_pattern = generation.bubble_pattern();
    let body = quote! {
        let next = #call;
        match resolve(next, slot, cx) {
            Resolution::Done(outcome) => outcome,
            #bubble_pattern => __estado_core::Outcome::Stayed,
        }
    };
    let name = always_function(generation, node);
    let documentation = format!("The `#[always]` of `{}`.", tree.node(node).name);
    state_function(generation, node, &name, &documentation, body)
}
