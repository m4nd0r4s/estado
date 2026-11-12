//! Handlers after analysis: the contract between the handler rewrite
//! (bead 4.2) and the transition and dispatch generators (bead 4.3).
//! See ADR 0011.

use syn::{Expr, ExprClosure, Ident, ItemFn};

use super::tree::StateId;

/// What a function in a `#[handlers]` impl is for (spec 6.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    /// `&mut Cx<Self>`.
    Context,
    /// `&Event`.
    Event,
}

/// How a `goto!` builds its target (spec 6.3).
pub enum GotoForm {
    /// `goto!(T)`, `goto!(T { .. })`, `goto!(T(x))`, `goto!(A { .. } / B { .. })`:
    /// one expression per level from the LCA child down to `target`
    /// (missing intermediate levels are E0020, unit levels need none).
    Simple { levels: Vec<(StateId, Option<Expr>)> },
    /// `goto!(T, |old: Exiting![X]| expr)`.
    Constructor { closure: ExprClosure },
    /// `goto!(T, history)`, with optional data for `T` itself.
    History { data: Option<Expr> },
    /// `goto!(T, external)`, with optional data for `T`.
    External { data: Option<Expr> },
}

/// One `goto!` occurrence; becomes one variant of the handler's edge enum.
pub struct GotoOccurrence {
    /// Variant name in the edge enum, e.g. `ToPaused`, `ToFailed1`.
    pub variant: Ident,
    /// Destination state (a choice node for `goto!(Choice)`).
    pub target: StateId,
    /// The LCA of the transition, resolved per spec table 5.2 from the
    /// handler's state and `target` (0 = root).
    pub lca: StateId,
    /// How the target is built.
    pub form: GotoForm,
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
    /// Whether the body calls `cx.raise`, `defer!()` or `cx.in_state`.
    pub uses: HandlerUses,
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
}
