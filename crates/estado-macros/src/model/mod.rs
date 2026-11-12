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
    EventCoverage, GotoForm, GotoOccurrence, HandlerModel, HandlerParameter, HandlerRole,
    HandlerUses, TransitionKind,
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
    /// The `#[handlers]` impl blocks with every handler rewritten (bead 4.2);
    /// what is emitted when the machine has no errors. Until then
    /// [`Self::handlers`] holds the degraded versions.
    pub rewritten_handlers: Vec<ItemImpl>,
    /// The analysed handlers (bead 4.2), consumed by the generators (4.3).
    pub analysed_handlers: Vec<HandlerModel>,
    /// Items of the module that are not part of the machine; emitted as is.
    pub passthrough: Vec<Item>,
}

/// The stretch of source a diagnostic underlines: from the first to the last
/// token of a node, so rustc draws a caret under all of it and not under its
/// first token only (stable proc-macros cannot join spans, but an error
/// built from two spans covers the range between them).
#[derive(Clone, Copy)]
pub struct Extent {
    /// Where it starts.
    pub start: proc_macro2::Span,
    /// Where it ends.
    pub end: proc_macro2::Span,
}

impl Extent {
    /// From the first to the last token of `node`.
    pub fn of(node: &impl quote::ToTokens) -> Self {
        let mut tokens = node.to_token_stream().into_iter();
        let Some(first) = tokens.next() else {
            return proc_macro2::Span::call_site().into();
        };
        let last = tokens.last().unwrap_or_else(|| first.clone());
        Self {
            start: first.span(),
            end: last.span(),
        }
    }
}

impl From<proc_macro2::Span> for Extent {
    fn from(span: proc_macro2::Span) -> Self {
        Self {
            start: span,
            end: span,
        }
    }
}

/// One diagnostic with a stable code (spec 6.7).
pub struct Diagnostic {
    /// `E0001`, `W0004`, ...
    pub code: &'static str,
    /// Where it points.
    pub extent: Extent,
    /// Short message.
    pub message: String,
    /// Optional `help:` line, e.g. a "did you mean" suggestion.
    pub help: Option<String>,
    /// Secondary locations, e.g. "first declared here". Stable proc-macros
    /// cannot attach labels, so each one becomes its own `compile_error!`
    /// whose message starts with `note:`. Added in 4.1.
    pub notes: Vec<(Extent, String)>,
}

impl Diagnostic {
    /// An error without help.
    pub fn new(code: &'static str, span: impl Into<Extent>, message: impl Into<String>) -> Self {
        Self {
            code,
            extent: span.into(),
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
    pub fn with_note(mut self, span: impl Into<Extent>, message: impl Into<String>) -> Self {
        self.notes.push((span.into(), message.into()));
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
        let mut tokens = error_over(self.extent, text).to_compile_error();
        for (extent, note) in &self.notes {
            let note_text = format!("estado::{}: note: {note}", self.code);
            tokens.extend(error_over(*extent, note_text).to_compile_error());
        }
        tokens
    }
}

/// A `syn::Error` that underlines `extent`: it is spanned by two tokens, one
/// at each end.
fn error_over(extent: Extent, message: String) -> syn::Error {
    use proc_macro2::{Ident, TokenStream, TokenTree};
    let tokens: TokenStream = [
        TokenTree::Ident(Ident::new("estado", extent.start)),
        TokenTree::Ident(Ident::new("estado", extent.end)),
    ]
    .into_iter()
    .collect();
    syn::Error::new_spanned(tokens, message)
}
