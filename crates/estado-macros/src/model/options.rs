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
    /// Whether `defer_cap` was written: the queues are then generated even
    /// when no handler is seen using them (ADR 0013 item 8).
    pub defer_cap_given: bool,
    /// `defer_overflow = error | drop_oldest`.
    pub defer_overflow: DeferOverflow,
    /// `raise_cap = N`, default 8.
    pub raise_cap: usize,
    /// Whether `raise_cap` was written (see [`Self::defer_cap_given`]).
    pub raise_cap_given: bool,
    /// `tracks_active`: `cx.in_state` is available even when no code of the
    /// module calls it syntactically (ADR 0013 item 8).
    pub tracks_active: bool,
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
    /// The effect type of the machine, `()` when not declared.
    pub fn effect_type(&self) -> Type {
        self.effect.clone().unwrap_or_else(|| parse_quote!(()))
    }

    /// The context type of the machine, `()` when not declared.
    pub fn ctx_type(&self) -> Type {
        self.ctx.clone().unwrap_or_else(|| parse_quote!(()))
    }

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
            defer_cap_given: false,
            defer_overflow: DeferOverflow::Error,
            raise_cap: 8,
            raise_cap_given: false,
            tracks_active: false,
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
