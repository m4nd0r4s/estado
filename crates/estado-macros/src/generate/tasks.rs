//! The life of a state instance (spec 5.3 steps 2, 8, 9 and 10, spec 7):
//! timers and invokes are cancelled on exit and scheduled on entry, with one
//! epoch per state that has them; a state with `#[always]` gets its pending
//! flag set on entry and cleared on exit.
//!
//! Per children owner (root, compound state, region) with such a state below:
//!
//! | Function | Does |
//! | --- | --- |
//! | `start_<enum>` | epochs, `StartTimer`, `Invoke`, pending `#[always]` flags of the states that just entered; parents first, regions in declaration order |
//! | `cancel_<enum>` | `CancelTimer`, `Cancel`, clears the flags of the states that just exited; deepest first, regions in reverse order |
//!
//! `lookups` generates `timer_event` and `accepts_invoke`.
//!
//! Closures of `#[after]` and `#[invoke]` receive the data of the state by
//! shared reference: `&T` for a newtype variant (the `#[state]` struct for a
//! compound or parallel state), `&<Variant>Ref` for a struct-like or tuple
//! variant, `&()` for a unit variant.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Expr;

use super::context::Generation;
use super::naming::{
    children_function, hygienic_local, payload_binding, ref_view_type, state_constant,
};
use super::runtime::{always_field, epoch_field};
use super::walk::{Binding, struct_field_pairs, tuple_field_bindings, variant_pattern};
use crate::model::{HandlerRole, Payload, StateId, StateKind};

/// Whether the state itself has timers, invokes or an `#[always]` handler.
pub fn has_lifecycle(generation: &Generation<'_>, node: StateId) -> bool {
    let state = generation.tree.node(node);
    !state.timers.is_empty()
        || !state.invokes.is_empty()
        || generation.handler(HandlerRole::Always, node).is_some()
}

/// Whether some state strictly below `node` has a lifecycle.
pub fn has_lifecycle_below(generation: &Generation<'_>, node: StateId) -> bool {
    generation
        .tree
        .any_below(node, &|child| has_lifecycle(generation, child))
}

/// Whether the machine has any timer.
pub fn has_timers(generation: &Generation<'_>) -> bool {
    generation
        .tree
        .nodes
        .iter()
        .any(|node| !node.timers.is_empty())
}

/// Whether the machine has any invoke.
pub fn has_invokes(generation: &Generation<'_>) -> bool {
    generation
        .tree
        .nodes
        .iter()
        .any(|node| !node.invokes.is_empty())
}

/// The call that schedules what entered below `owner`; empty when nothing
/// there has a lifecycle. `children` is the binding of the children enum.
pub fn start_call(
    generation: &Generation<'_>,
    owner: StateId,
    children: &proc_macro2::Ident,
) -> TokenStream {
    if !has_lifecycle_below(generation, owner) {
        return TokenStream::new();
    }
    let function = children_function("start", generation.tree, owner);
    quote! { #function(&*#children, cx); }
}

/// The call that cancels what exited below `owner`; empty when nothing there
/// has a lifecycle.
pub fn cancel_call(
    generation: &Generation<'_>,
    owner: StateId,
    children: &proc_macro2::Ident,
) -> TokenStream {
    if !has_lifecycle_below(generation, owner) {
        return TokenStream::new();
    }
    let function = children_function("cancel", generation.tree, owner);
    quote! { #function(&*#children, cx); }
}

/// The expression of the data a closure of `node` receives, a reference.
pub fn data_reference(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(node);
    match &state.payload {
        Payload::Newtype(_) => {
            let payload = payload_binding(tree, node);
            quote! { #payload }
        }
        Payload::Struct(_) => {
            let view = ref_view_type(tree, node);
            let pairs = struct_field_pairs(generation, node);
            quote! { &#view { #(#pairs),* } }
        }
        Payload::Tuple(_) => {
            let view = ref_view_type(tree, node);
            let names = tuple_field_bindings(generation, node);
            quote! { &#view(#(#names),*) }
        }
        Payload::Unit | Payload::None => quote! { &() },
    }
}

/// `expression` evaluated on `data`: a closure is applied, anything else
/// is the value itself.
pub fn evaluate(expression: &Expr, data: &TokenStream) -> TokenStream {
    if matches!(expression, Expr::Closure(_)) {
        quote! { with_data(#data, #expression) }
    } else {
        quote! { #expression }
    }
}

/// The statements of step 8 and 9 (and 10) for one state that entered.
fn start_statements(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let state = generation.tree.node(node);
    let epoch = hygienic_local("epoch");
    let mut statements = TokenStream::new();
    if !state.timers.is_empty() || !state.invokes.is_empty() {
        let field = epoch_field(generation, node);
        let id = state_constant(generation, node);
        let data = data_reference(generation, node);
        statements.extend(quote! {
            let #epoch = {
                let counter = &mut cx.runtime_mut().#field;
                *counter = counter.wrapping_add(1);
                *counter
            };
        });
        for (index, timer) in state.timers.iter().enumerate() {
            let index = u8::try_from(index).unwrap_or(u8::MAX);
            let after = evaluate(&timer.duration, &data);
            statements.extend(quote! {
                cx.effect(__estado_core::Effect::StartTimer {
                    key: timer_key(#id, #epoch, #index),
                    after: #after,
                });
            });
        }
        for (index, invoke) in state.invokes.iter().enumerate() {
            let index = u8::try_from(index).unwrap_or(u8::MAX);
            let task = evaluate(&invoke.task, &data);
            statements.extend(quote! {
                cx.effect(__estado_core::Effect::Invoke {
                    key: invoke_key(#id, #epoch, #index),
                    task: #task,
                });
            });
        }
    }
    if generation.handler(HandlerRole::Always, node).is_some() {
        let field = always_field(generation, node);
        statements.extend(quote! { cx.runtime_mut().#field = true; });
    }
    statements
}

/// The statements of step 2 for one state that exited.
fn cancel_statements(generation: &Generation<'_>, node: StateId) -> TokenStream {
    let state = generation.tree.node(node);
    let epoch = hygienic_local("epoch");
    let mut statements = TokenStream::new();
    if !state.timers.is_empty() || !state.invokes.is_empty() {
        let field = epoch_field(generation, node);
        let id = state_constant(generation, node);
        statements.extend(quote! { let #epoch = cx.runtime_mut().#field; });
        for index in 0..state.timers.len() {
            let index = u8::try_from(index).unwrap_or(u8::MAX);
            statements.extend(quote! {
                cx.effect(__estado_core::Effect::CancelTimer(timer_key(#id, #epoch, #index)));
            });
        }
        for index in 0..state.invokes.len() {
            let index = u8::try_from(index).unwrap_or(u8::MAX);
            statements.extend(quote! {
                cx.effect(__estado_core::Effect::Cancel(invoke_key(#id, #epoch, #index)));
            });
        }
    }
    if generation.handler(HandlerRole::Always, node).is_some() {
        let field = always_field(generation, node);
        statements.extend(quote! { cx.runtime_mut().#field = false; });
    }
    statements
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Start,
    Cancel,
}

/// `start_<enum>` and `cancel_<enum>` of every children owner that needs them.
pub fn lifecycle_functions(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let mut functions = TokenStream::new();
    for node in &tree.nodes {
        if !tree.owns_children(node.id) || !has_lifecycle_below(generation, node.id) {
            continue;
        }
        functions.extend(sequence(generation, node.id, Phase::Start));
        functions.extend(sequence(generation, node.id, Phase::Cancel));
    }
    functions
}

fn sequence(generation: &Generation<'_>, owner: StateId, phase: Phase) -> TokenStream {
    let tree = generation.tree;
    let (prefix, verb) = match phase {
        Phase::Start => ("start", "Schedules the timers and invokes of"),
        Phase::Cancel => ("cancel", "Cancels the timers and invokes of"),
    };
    let name = children_function(prefix, generation.tree, owner);
    let enum_name = tree.node(owner).children_enum.as_ref().expect("owner");
    let documentation =
        format!("{verb} the active child of `{enum_name}` and everything below it.");
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let children = hygienic_local("children");
    let arms = tree.node(owner).children.iter().filter_map(|&child| {
        let composite = matches!(
            tree.node(child).kind,
            StateKind::Compound | StateKind::Parallel
        );
        let own = has_lifecycle(generation, child);
        let below = composite && has_lifecycle_below(generation, child);
        if !own && !below {
            return None;
        }
        let pattern = variant_pattern(generation, child, Binding::Everything);
        let own_statements = match phase {
            Phase::Start => start_statements(generation, child),
            Phase::Cancel => cancel_statements(generation, child),
        };
        let nested = below.then(|| nested_calls(generation, child, phase));
        let body = match phase {
            Phase::Start => quote! { #own_statements #nested },
            Phase::Cancel => quote! { #nested #own_statements },
        };
        Some(quote! { #pattern => { #body } })
    });
    quote! {
        #[doc = #documentation]
        #[allow(unreachable_patterns)]
        pub fn #name<#sink_parameter>(#children: &#enum_name, cx: &mut #cx_type) {
            match #children {
                #(#arms)*
                _ => {}
            }
        }
    }
}

/// The calls into the substates of a compound state or the regions of a
/// parallel one: regions in declaration order to start, reversed to cancel.
fn nested_calls(generation: &Generation<'_>, node: StateId, phase: Phase) -> TokenStream {
    let tree = generation.tree;
    let payload = payload_binding(tree, node);
    let mut owners = tree.children_owners(node);
    if phase == Phase::Cancel {
        owners.reverse();
    }
    let prefix = if phase == Phase::Start {
        "start"
    } else {
        "cancel"
    };
    let calls = owners.iter().filter_map(|&owner| {
        if !has_lifecycle_below(generation, owner) {
            return None;
        }
        let function = children_function(prefix, generation.tree, owner);
        let field = tree
            .node(owner)
            .children_field
            .as_ref()
            .expect("children field");
        Some(quote! { #function(&#payload.#field, cx); })
    });
    quote! { #(#calls)* }
}

/// `timer_key`, `invoke_key` and `with_data`.
pub fn helper_items(generation: &Generation<'_>) -> TokenStream {
    let timers = has_timers(generation).then(|| {
        quote! {
            /// The key of the timer `index` of the state instance with `epoch`.
            pub const fn timer_key(state: __estado_core::StateId, epoch: u32, index: u8) -> __estado_core::TimerKey {
                __estado_core::TimerKey { state, epoch, index, child: __estado_core::ChildPath::ROOT }
            }
        }
    });
    let invokes = has_invokes(generation).then(|| {
        quote! {
            /// The key of the invoke `index` of the state instance with `epoch`.
            pub const fn invoke_key(state: __estado_core::StateId, epoch: u32, index: u8) -> __estado_core::InvokeKey {
                __estado_core::InvokeKey { state, epoch, index, child: __estado_core::ChildPath::ROOT }
            }
        }
    });
    if timers.is_none() && invokes.is_none() {
        return TokenStream::new();
    }
    // `timer_key` and `invoke_key` are both used by start and cancel.
    let timer_helper = timers.unwrap_or_default();
    let invoke_helper = invokes.unwrap_or_default();
    quote! {
        #timer_helper
        #invoke_helper

        /// Applies a closure of `#[after]` or `#[invoke]` to the data of its
        /// state: the data type is known before the closure is checked.
        pub fn with_data<Data: ?Sized, Output>(data: &Data, function: impl FnOnce(&Data) -> Output) -> Output {
            function(data)
        }
    }
}
