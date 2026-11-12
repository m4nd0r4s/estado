//! The machine model: everything the code generators need, parsed from a
//! `#[machine]` module and validated (ADR 0010).
//!
//! Parsing fills [`MachineModel`]; analysis then checks it and derives the
//! [`StateTree`]. Generators only read the model, never the raw syntax.

#![allow(dead_code)] // the IR is read in full only once the generators of 4.2/4.3 land

mod handlers;
mod options;
mod tree;

#[allow(unused_imports)] // consumed by the generators of beads 4.2 and 4.3
pub use handlers::{
    GotoForm, GotoOccurrence, HandlerModel, HandlerParameter, HandlerRole, HandlerUses,
};
pub use options::{DeferOverflow, MachineOptions, UnhandledPolicy};
pub use tree::{
    AfterSpec, HistoryKind, InvokeSpec, Payload, StateId, StateKind, StateNode, StateTree, TypeDecl,
};

use syn::{Attribute, Ident, Item, ItemImpl, Visibility};

/// A whole machine, as written in the `#[machine]` module.
pub struct MachineModel {
    /// Options of the `#[machine(..)]` attribute.
    pub options: MachineOptions,
    /// Attributes of the module other than `#[machine]` (docs, `allow`, ..),
    /// re-emitted on the generated module. Added in 4.1.
    pub module_attributes: Vec<Attribute>,
    /// Visibility of the module.
    pub module_visibility: Visibility,
    /// Name of the module.
    pub module_name: Ident,
    /// Every state, root first, in declaration order (pre-order).
    pub tree: StateTree,
    /// The `#[handlers]` impl blocks, untouched (parsing output).
    pub handlers: Vec<ItemImpl>,
    /// The analysed handlers (bead 4.2), consumed by the generators (4.3).
    pub analysed_handlers: Vec<HandlerModel>,
    /// Items of the module that are not part of the machine; emitted as is.
    pub passthrough: Vec<Item>,
}

/// One diagnostic with a stable code (spec 6.7).
pub struct Diagnostic {
    /// `E0001`, `W0004`, ...
    pub code: &'static str,
    /// Where it points.
    pub span: proc_macro2::Span,
    /// Short message.
    pub message: String,
    /// Optional `help:` line, e.g. a "did you mean" suggestion.
    pub help: Option<String>,
    /// Secondary locations, e.g. "first declared here". Stable proc-macros
    /// cannot attach labels, so each one becomes its own `compile_error!`
    /// whose message starts with `note:`. Added in 4.1.
    pub notes: Vec<(proc_macro2::Span, String)>,
}

impl Diagnostic {
    /// An error without help.
    pub fn new(code: &'static str, span: proc_macro2::Span, message: impl Into<String>) -> Self {
        Self {
            code,
            span,
            message: message.into(),
            help: None,
            notes: Vec::new(),
        }
    }

    /// Adds a `help:` line.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Adds a secondary location, rendered as an extra `note:` error.
    pub fn with_note(mut self, span: proc_macro2::Span, message: impl Into<String>) -> Self {
        self.notes.push((span, message.into()));
        self
    }

    /// Renders as a `compile_error!` next to the code (spec 6.6: never
    /// abort the whole expansion), followed by one per note.
    pub fn to_compile_error(&self) -> proc_macro2::TokenStream {
        let mut text = format!("estado::{}: {}", self.code, self.message);
        if let Some(help) = &self.help {
            text.push_str("\n  = help: ");
            text.push_str(help);
        }
        let mut tokens = syn::Error::new(self.span, text).to_compile_error();
        for (span, note) in &self.notes {
            let note_text = format!("estado::{}: note: {note}", self.code);
            tokens.extend(syn::Error::new(*span, note_text).to_compile_error());
        }
        tokens
    }
}
