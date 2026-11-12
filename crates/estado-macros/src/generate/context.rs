//! What every generator needs: the model, the types of the machine and a
//! lookup of the handlers by role and state.

use std::collections::HashMap;

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{Ident, Type};

use crate::model::{HandlerModel, HandlerRole, MachineModel, StateId, StateTree};
use crate::naming::SINK_PARAMETER;

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
            effect: model.options.effect_type(),
            handlers,
        }
    }

    /// The visibility of the root enum, which the generated types of the
    /// machine module copy (ADR 0013 item 9).
    pub fn root_visibility(&self) -> syn::Visibility {
        self.tree
            .enum_declaration(self.root)
            .map_or(syn::Visibility::Inherited, |declaration| {
                declaration.vis.clone()
            })
    }

    /// The handler of `role` for `state`, if the user wrote one.
    pub fn handler(&self, role: HandlerRole, state: StateId) -> Option<&'model HandlerModel> {
        self.handlers.get(&(role, state)).copied()
    }

    /// The generic sink parameter name.
    pub fn sink(&self) -> Ident {
        Ident::new(SINK_PARAMETER, proc_macro2::Span::call_site())
    }

    /// `__estado_core::Cx<'_, Root, EffectSink>`.
    pub fn cx_type(&self) -> TokenStream {
        let root = self.root;
        let sink = self.sink();
        quote! { __estado_core::Cx<'_, #root, #sink> }
    }

    /// `EffectSink: __estado_core::Sink<Effect>`, for generic parameter lists.
    pub fn sink_parameter(&self) -> TokenStream {
        let sink = self.sink();
        let effect = &self.effect;
        quote! { #sink: __estado_core::Sink<#effect> }
    }

    /// Whether some state can complete (has a pending flag).
    pub fn needs_completion(&self) -> bool {
        !super::completion::completing_states(self).is_empty()
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

    /// Whether `cx.in_state` is available: forced with
    /// `#[machine(tracks_active)]`, else some code of the module calls it
    /// (a handler, or a helper function inside the module).
    pub fn tracks_active(&self) -> bool {
        self.model.options.tracks_active
            || self
                .model
                .analysed_handlers
                .iter()
                .any(|handler| handler.uses.in_state)
            || self.module_calls("in_state")
    }

    /// Whether the machine needs the raise and defer queues: `defer!()` in a
    /// handler, `cx.raise` anywhere in the module, or an explicit
    /// `raise_cap` / `defer_cap` (for helpers outside the module).
    pub fn uses_queues(&self) -> bool {
        let options = &self.model.options;
        options.raise_cap_given
            || options.defer_cap_given
            || self
                .model
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

/// Whether `tokens` call `name` as a method (`.name(..)`, `.name::<..>(..)`)
/// or through a path (`Cx::name(cx, ..)`, `Cx::<..>::name(..)`,
/// `::estado::Cx::name(..)`): the identifier follows a `.` or a `::`.
fn calls_method(tokens: TokenStream, name: &str) -> bool {
    let mut previous = Previous::Other;
    for tree in tokens {
        match tree {
            TokenTree::Punct(punct) => {
                previous = match (punct.as_char(), previous) {
                    ('.', _) => Previous::Dot,
                    (':', Previous::Colon) => Previous::PathSeparator,
                    (':', _) => Previous::Colon,
                    _ => Previous::Other,
                };
            }
            TokenTree::Ident(ident) => {
                if ident == name && matches!(previous, Previous::Dot | Previous::PathSeparator) {
                    return true;
                }
                previous = Previous::Other;
            }
            TokenTree::Group(group) => {
                if calls_method(group.stream(), name) {
                    return true;
                }
                previous = Previous::Other;
            }
            TokenTree::Literal(_) => previous = Previous::Other,
        }
    }
    false
}

/// What the scan saw just before the current token.
#[derive(Clone, Copy)]
enum Previous {
    Dot,
    /// One `:`, the start of a `::`.
    Colon,
    /// A complete `::`.
    PathSeparator,
    Other,
}
