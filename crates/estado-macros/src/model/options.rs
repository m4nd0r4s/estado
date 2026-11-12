//! Options of the `#[machine(..)]` attribute (spec 6.1).

use proc_macro2::Span;
use syn::{Ident, Type, parse_quote};

/// Machine-wide options.
pub struct MachineOptions {
    /// Where the attribute is, for diagnostics about it. Added in 4.1.
    pub span: Span,
    /// Whether `event = T` was given. When `false`, [`Self::event`] is the
    /// placeholder `()` and analysis reports E0002. Added in 4.1.
    pub event_declared: bool,
    /// `event = T`; required (E0002).
    pub event: Type,
    /// `effect = T`, default `()`.
    pub effect: Option<Type>,
    /// `ctx = T`, default `()`.
    pub ctx: Option<Type>,
    /// `unhandled = ignore | error | panic`.
    pub unhandled: UnhandledPolicy,
    /// `defer_cap = N`, default 8.
    pub defer_cap: usize,
    /// `defer_overflow = error | drop_oldest`.
    pub defer_overflow: DeferOverflow,
    /// `raise_cap = N`, default 8.
    pub raise_cap: usize,
    /// `fx_inline = N`, default 4.
    pub fx_inline: usize,
    /// `max_microsteps = N`, default 64.
    pub max_microsteps: u16,
    /// `size_warn = N` bytes, default 128 (W0004).
    pub size_warn: usize,
    /// `typestate`.
    pub typestate: bool,
    /// `infallible`.
    pub infallible: bool,
    /// `version = N`, default 1.
    pub version: u32,
    /// `allow(W0001, ..)`.
    pub allowed_warnings: Vec<Ident>,
}

/// Unhandled-event policy (spec 5.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnhandledPolicy {
    /// `ignore` (default).
    Ignore,
    /// `error`.
    Error,
    /// `panic`.
    Panic,
}

/// Defer overflow policy (spec 5.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeferOverflow {
    /// `error` (default).
    Error,
    /// `drop_oldest`.
    DropOldest,
}

impl MachineOptions {
    /// Options with every default of spec 6.1 and no `event` yet.
    pub fn new(span: Span) -> Self {
        Self {
            span,
            event_declared: false,
            event: parse_quote!(()),
            effect: None,
            ctx: None,
            unhandled: UnhandledPolicy::Ignore,
            defer_cap: 8,
            defer_overflow: DeferOverflow::Error,
            raise_cap: 8,
            fx_inline: 4,
            max_microsteps: 64,
            size_warn: 128,
            typestate: false,
            infallible: false,
            version: 1,
            allowed_warnings: Vec::new(),
        }
    }
}
