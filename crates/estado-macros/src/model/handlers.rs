//! Handlers after analysis: the contract between the handler rewrite
//! (bead 4.2) and the transition and dispatch generators (bead 4.3).
//! See ADR 0011.

use syn::{Ident, ItemFn};

use super::tree::StateId;

/// What a function in a `#[handlers]` impl is for (spec 6.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HandlerRole {
    /// `#[on(S)]`: reacts to events.
    On,
    /// `#[entry(S)]`.
    Entry,
    /// `#[exit(S)]`.
    Exit,
    /// `#[always(S)]`.
    Always,
    /// `#[on_done(S)]`.
    OnDone,
    /// `#[choice]`: a pseudostate, `state` is the choice node.
    Choice,
}

/// One parameter of a handler, recognised by its type (spec 6.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HandlerParameter {
    /// `&mut T` / `&T` to the newtype payload of the own state.
    OwnData { mutable: bool },
    /// `<Variant>Mut<'_>` view of a struct-like or tuple own state.
    OwnView,
    /// `<Composite>Fields<'_>` of the ancestor `ancestor` (nearest first);
    /// for the own state of a compound or parallel state, its own fields.
    Fields { ancestor: StateId },
    /// `&Output` of `#[on_done]`: the payload of the final substate.
    Output,
    /// `&mut Cx<Self>`.
    Context,
    /// `&Event`.
    Event,
}

/// How a `goto!` is carried by its edge variant (spec 6.3, ADR 0011).
pub enum GotoForm {
    /// Simple form (`goto!(T)`, `goto!(T { .. })`, `goto!(T(x))`,
    /// `goto!(A { .. } / B { .. })`): the handler builds the value of the
    /// child of the LCA in the decision phase and the edge carries it.
    Value,
    /// `goto!(T, |old: Exiting![X]| expr)`: the edge carries the closure;
    /// `exiting` is `X`, the child of the LCA on the active path, whose value
    /// the constructor receives by value.
    Constructor { exiting: StateId },
    /// A simple form whose value contains an `#[initial(with = f)]` call that
    /// needs `cx` (errata 15): the handler only evaluates the data the user
    /// wrote, the edge carries it as `part_types`, and `Edge::apply` builds
    /// the value with `build` after the exits.
    Deferred {
        /// The types of the values the edge variant carries, in order.
        part_types: Vec<syn::Type>,
        /// The expression of the value of the child of the LCA, over the
        /// `part_ident(i)` bindings and `cx`.
        build: proc_macro2::TokenStream,
    },
}

/// The flavour of a transition, for the graph and for entry resolution.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TransitionKind {
    /// Plain `goto!(T)` (local or external by geometry, table 5.2).
    Normal,
    /// `goto!(T, external)`.
    External,
    /// `goto!(T, history)`.
    History,
}

/// One `goto!` occurrence; becomes one variant of the handler's edge enum.
pub struct GotoOccurrence {
    /// Variant name in the edge enum, e.g. `ToPaused`, `ToFailed2`.
    pub variant: Ident,
    /// Destination state.
    pub target: StateId,
    /// The LCA of the transition, resolved per spec table 5.2 from the
    /// handler's state and `target` (0 = root).
    pub lca: StateId,
    /// How the edge carries the new value.
    pub form: GotoForm,
    /// The flavour of the transition.
    pub kind: TransitionKind,
    /// Whether the target is the handler's own state, one of its ancestors or
    /// one of its descendants (which may already be active):
    /// `Transitioned { changed }` then compares the active configuration
    /// before and after (ADR 0006). A sibling or a cousin always changes it.
    pub compares_leaf: bool,
    /// Event pattern of the enclosing `match` arm, for the graph (spec 8.1).
    pub event_hint: Option<String>,
}

/// One analysed function of a `#[handlers]` impl.
pub struct HandlerModel {
    /// The role.
    pub role: HandlerRole,
    /// The state it belongs to.
    pub state: StateId,
    /// Its parameters, in order.
    pub parameters: Vec<HandlerParameter>,
    /// The function, rewritten by 4.2: generic sink added, `Cx<Self>`
    /// completed, `'event` named, return type rewritten, every `goto!`
    /// replaced by `Next::Goto(__estado::<edges>::<variant>(..))`.
    pub function: ItemFn,
    /// Name of the generated edge enum, e.g. `PlayingEdges`.
    pub edges_enum: Ident,
    /// The `goto!` occurrences, in source order.
    pub gotos: Vec<GotoOccurrence>,
    /// Whether it returns `Result<Next<Self>, E>` (spec 6.4).
    pub returns_result: bool,
    /// Whether the body calls `cx.raise`, `defer!()`, `cx.in_state` or
    /// `reenter!()`.
    pub uses: HandlerUses,
    /// Where each `defer!()` of the body is written (the `defer` identifier),
    /// in source order. Used by W0003.
    pub defer_calls: Vec<proc_macro2::Span>,
    /// Which events the handler reacts to. Used by W0005.
    pub event_coverage: EventCoverage,
}

/// The events an `#[on]` handler can react to, read from its `match` over the
/// event parameter. Conservative: whenever the shape of the body is not
/// understood the answer is [`EventCoverage::AnyEvent`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EventCoverage {
    /// May react to any event: no `match` over the event, a catch-all arm that
    /// does more than `bubble!()` / `unhandled!()`, an arm pattern that is
    /// not a variant, or any other use of the event.
    AnyEvent,
    /// Reacts only to the listed event variants (last path segment, e.g.
    /// `Timeout` for `Ev::Timeout`); every other event bubbles or is unhandled.
    Variants(Vec<String>),
}

/// Features a handler body uses, detected syntactically; they decide the
/// shape of the generated runtime (ADR 0011).
#[derive(Clone, Copy, Default, Debug)]
pub struct HandlerUses {
    /// `cx.raise(..)`.
    pub raise: bool,
    /// `defer!()`.
    pub defer: bool,
    /// `cx.in_state(..)`.
    pub in_state: bool,
    /// `reenter!()`.
    pub reenter: bool,
}
