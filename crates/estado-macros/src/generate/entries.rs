//! Entry and exit code as linear functions per children enum (spec 5.3
//! steps 1 and 7, ADR 0011): `exit_<enum>` exits the active child of the
//! enum with everything below it, deepest first; `enter_<enum>` enters it,
//! shallowest first. Each calls the user's `#[exit]` / `#[entry]` functions.

use proc_macro2::TokenStream;
use quote::quote;

use super::completion::{completed_by, completing_states};
use super::context::Generation;
use super::naming::{children_binding, enter_function, exit_function, fields_binding, fields_type};
use super::runtime::{clear_done, set_done};
use super::walk::{Binding, FieldsAccess, handler_arguments, split_payload, variant_pattern};
use crate::model::{HandlerRole, StateId, StateKind};

/// Every `exit_*` and `enter_*` function of the machine.
pub fn entry_exit_functions(generation: &Generation<'_>) -> TokenStream {
    let tree = generation.tree;
    let mut functions = TokenStream::new();
    for node in &tree.nodes {
        if !tree.owns_children(node.id) {
            continue;
        }
        functions.extend(sequence_function(generation, node.id, Direction::Exit));
        functions.extend(sequence_function(generation, node.id, Direction::Enter));
    }
    functions
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Exit,
    Enter,
}

/// One function over the children enum of `owner`.
fn sequence_function(
    generation: &Generation<'_>,
    owner: StateId,
    direction: Direction,
) -> TokenStream {
    let tree = generation.tree;
    let (name, role, verb) = match direction {
        Direction::Exit => (exit_function(tree, owner), HandlerRole::Exit, "Exits"),
        Direction::Enter => (enter_function(tree, owner), HandlerRole::Entry, "Enters"),
    };
    let enum_name = tree
        .node(owner)
        .children_enum
        .as_ref()
        .expect("children enum");
    let documentation =
        format!("{verb} the active child of `{enum_name}` and everything below it.");
    let ancestors = tree.fields_chain(owner);
    let ancestor_parameters = ancestors.iter().map(|&ancestor| {
        let binding = fields_binding(tree, ancestor);
        let view = fields_type(tree, ancestor);
        quote! { #binding: &mut #view<'_> }
    });
    let sink_parameter = generation.sink_parameter();
    let cx_type = generation.cx_type();
    let arms: Vec<TokenStream> = tree
        .node(owner)
        .children
        .iter()
        .map(|&child| arm(generation, child, role, direction, &ancestors))
        .collect();
    quote! {
        #[doc = #documentation]
        pub fn #name<#sink_parameter>(
            children: &mut #enum_name,
            #(#ancestor_parameters,)*
            cx: &mut #cx_type,
        ) {
            match children {
                #(#arms)*
            }
        }
    }
}

/// The match arm of one child variant.
fn arm(
    generation: &Generation<'_>,
    child: StateId,
    role: HandlerRole,
    direction: Direction,
    ancestors: &[StateId],
) -> TokenStream {
    let tree = generation.tree;
    let state = tree.node(child);
    let handler = generation.handler(role, child);
    let is_composite = matches!(state.kind, StateKind::Compound | StateKind::Parallel);

    let own_call = handler.map(|handler| {
        let root = generation.root;
        let function = &handler.function.sig.ident;
        let sink = generation.sink();
        // Every `*Fields` argument is a reborrow of a binding in scope.
        let arguments = handler_arguments(generation, handler, FieldsAccess::Reborrow, None);
        quote! { #root::#function::<#sink>(#(#arguments),*); }
    });

    // Entering a final state below the root schedules the completion
    // microstep of the state it completes (ADR 0005, ADR 0013 item 4).
    let completion = (direction == Direction::Enter && state.is_final)
        .then(|| completed_by(tree, child))
        .flatten()
        .map(|completed| set_done(generation, completed));
    if !is_composite {
        let binding = if own_call.is_some() {
            Binding::Everything
        } else {
            Binding::Nothing
        };
        let pattern = variant_pattern(generation, child, binding);
        return quote! { #pattern => { #own_call #completion } };
    }

    let pattern = variant_pattern(generation, child, Binding::Everything);
    let own_fields = fields_binding(tree, child);
    let split = split_payload(generation, child);
    // A completion that was not served yet dies with its state.
    let clear_completion = (direction == Direction::Exit
        && completing_states(generation).contains(&child))
    .then(|| clear_done(generation, child));
    let inherited: Vec<_> = ancestors
        .iter()
        .map(|&ancestor| fields_binding(tree, ancestor))
        .collect();
    let recursion = |target: StateId| {
        let function = match direction {
            Direction::Exit => exit_function(tree, target),
            Direction::Enter => enter_function(tree, target),
        };
        let binding = children_binding(tree, target);
        quote! { #function(#binding, #(#inherited,)* &mut #own_fields, cx); }
    };
    if state.kind == StateKind::Parallel {
        let mut regions = state.children.clone();
        if direction == Direction::Exit {
            regions.reverse();
        }
        let recurse: Vec<TokenStream> = regions.iter().map(|&region| recursion(region)).collect();
        let body = match direction {
            Direction::Exit => quote! { #(#recurse)* #own_call #clear_completion },
            Direction::Enter => quote! { #own_call #(#recurse)* },
        };
        return quote! {
            #pattern => {
                #split
                #body
            }
        };
    }
    let recurse = recursion(child);
    let record = super::history::record_shallow(generation, child);
    let body = match direction {
        Direction::Exit => quote! { #recurse #own_call #record #clear_completion },
        Direction::Enter => quote! { #own_call #recurse },
    };
    quote! {
        #pattern => {
            #split
            #body
        }
    }
}
