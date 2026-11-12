//! What every generator needs: the model, the types of the machine and a
//! lookup of the handlers by role and state.

use std::collections::HashMap;

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{Ident, Type};

use crate::model::{HandlerModel, HandlerRole, MachineModel, StateId, StateTree};

/// Name of the generic sink parameter of generated functions.
pub const SINK: &str = "EffectSink";

/// Read-only context shared by the generators.
pub struct Generation<'model> {
    /// The model.
    pub model: &'model MachineModel,
    /// Its tree.
    pub tree: &'model StateTree,
    /// Name of the root enum, the type the machine is implemented for.
    pub root: &'model Ident,
    /// Effect type of the machine (`()` by default).
    pub effect: Type,
    handlers: HashMap<(HandlerRole, StateId), &'model HandlerModel>,
}

impl<'model> Generation<'model> {
    /// Builds the context of `model`.
    pub fn new(model: &'model MachineModel) -> Self {
        let handlers = model
            .analysed_handlers
            .iter()
            .map(|handler| ((handler.role, handler.state), handler))
            .collect();
        Self {
            model,
            tree: &model.tree,
            root: &model.tree.root().name,
            effect: model
                .options
                .effect
                .clone()
                .unwrap_or_else(|| syn::parse_quote!(())),
            handlers,
        }
    }

    /// The handler of `role` for `state`, if the user wrote one.
    pub fn handler(&self, role: HandlerRole, state: StateId) -> Option<&'model HandlerModel> {
        self.handlers.get(&(role, state)).copied()
    }

    /// The generic sink parameter name.
    pub fn sink(&self) -> Ident {
        Ident::new(SINK, proc_macro2::Span::call_site())
    }

    /// `::estado_core::Cx<'_, Root, EffectSink>`.
    pub fn cx_type(&self) -> TokenStream {
        let root = self.root;
        let sink = self.sink();
        quote! { ::estado_core::Cx<'_, #root, #sink> }
    }

    /// `EffectSink: ::estado_core::Sink<Effect>`, for generic parameter lists.
    pub fn sink_parameter(&self) -> TokenStream {
        let sink = self.sink();
        let effect = &self.effect;
        quote! { #sink: ::estado_core::Sink<#effect> }
    }

    /// Whether the runtime needs the completion flag: the machine has a
    /// final state below the root, or an `#[on_done]` handler.
    pub fn needs_completion(&self) -> bool {
        super::completion::has_completion(self)
    }

    /// Whether some state has an `#[always]` handler.
    pub fn has_always(&self) -> bool {
        self.model
            .analysed_handlers
            .iter()
            .any(|handler| handler.role == HandlerRole::Always)
    }

    /// The pattern of a `Resolution` that ends a microstep without a
    /// transition when nothing can bubble further (completion, `#[always]`).
    pub fn bubble_pattern(&self) -> TokenStream {
        if self.uses_reenter() {
            quote! { Resolution::Bubble | Resolution::Reenter }
        } else {
            quote! { Resolution::Bubble }
        }
    }

    /// Whether any handler returns `reenter!()`.
    pub fn uses_reenter(&self) -> bool {
        self.model
            .analysed_handlers
            .iter()
            .any(|handler| handler.role == HandlerRole::On && handler.uses.reenter)
    }

    /// Whether any code of the module calls `cx.in_state`: a handler, or a
    /// helper function a handler calls with its `cx`.
    pub fn tracks_active(&self) -> bool {
        self.model
            .analysed_handlers
            .iter()
            .any(|handler| handler.uses.in_state)
            || self.module_calls("in_state")
    }

    /// Whether the machine needs the raise and defer queues: `defer!()` in a
    /// handler, or `cx.raise` anywhere in the module.
    pub fn uses_queues(&self) -> bool {
        self.model
            .analysed_handlers
            .iter()
            .any(|handler| handler.uses.raise || handler.uses.defer)
            || self.module_calls("raise")
    }

    /// Whether some function of the module, handler or not, has a method
    /// call `.name(..)`. The scan is syntactic, like the detection in handler
    /// bodies, but it also sees the helpers handlers pass their `cx` to.
    fn module_calls(&self, name: &str) -> bool {
        let items = self
            .model
            .passthrough
            .iter()
            .map(ToTokens::to_token_stream)
            .chain(self.model.handlers.iter().map(ToTokens::to_token_stream));
        items.into_iter().any(|tokens| calls_method(tokens, name))
    }
}

/// Whether `tokens` contain `.name` followed by a parenthesis or a turbofish.
fn calls_method(tokens: TokenStream, name: &str) -> bool {
    let mut previous_is_dot = false;
    for tree in tokens {
        match tree {
            TokenTree::Punct(punct) => previous_is_dot = punct.as_char() == '.',
            TokenTree::Ident(ident) => {
                if previous_is_dot && ident == name {
                    return true;
                }
                previous_is_dot = false;
            }
            TokenTree::Group(group) => {
                if calls_method(group.stream(), name) {
                    return true;
                }
                previous_is_dot = false;
            }
            TokenTree::Literal(_) => previous_is_dot = false,
        }
    }
    false
}
