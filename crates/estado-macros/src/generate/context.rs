//! What every generator needs: the model, the types of the machine and a
//! lookup of the handlers by role and state.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
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
    pub fn needs_always(&self) -> bool {
        super::completion::has_completion(self)
    }

    /// Whether any handler calls `cx.in_state`.
    pub fn tracks_active(&self) -> bool {
        self.model
            .analysed_handlers
            .iter()
            .any(|handler| handler.uses.in_state)
    }
}
