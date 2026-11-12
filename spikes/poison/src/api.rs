//! Shared vocabulary: what the generated code would provide, hand-written.

/// What a `send` does, so tests can place a panic in any phase of spec 5.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    /// Transition at the root: exit, take, construct, write, enter.
    Plain,
    /// Transition whose LCA is below the root (constructor rebuilds the path).
    Nested,
    PanicInExit,
    PanicInConstructor,
    PanicInNestedConstructor,
    PanicInEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    Poisoned,
}

/// Stand-in for the macro-generated `Spec` implementation.
pub trait Spec: Sized {
    fn initial() -> Self;
    /// Constructor body: consumes the old value, returns the new one.
    fn next(self) -> Self;
    /// Constructor body when the LCA is below the root. `panic_inside` makes
    /// it panic after the old value was destructured.
    fn nested(self, panic_inside: bool) -> Self {
        assert!(!panic_inside, "panic inside nested constructor");
        self.next()
    }

    /// Exit hook, runs in place before the take (spec 5.3 step 1).
    fn exit(&mut self, script: Script) {
        assert!(script != Script::PanicInExit, "panic in exit");
    }
    /// Entry hook, runs in place after the write (spec 5.3 step 7).
    fn enter(&mut self, script: Script) {
        assert!(script != Script::PanicInEntry, "panic in entry");
    }
    /// Step 5: the constructor.
    fn construct(self, script: Script) -> Self {
        match script {
            Script::PanicInConstructor => panic!("panic in constructor"),
            Script::Nested => self.nested(false),
            Script::PanicInNestedConstructor => self.nested(true),
            _ => self.next(),
        }
    }
}

/// The part of the `Machine` API that every option can implement safely.
pub trait MachineApi<M: Spec, C>: Sized {
    fn create(initial_state: M, ctx: C) -> Self;
    fn send(&mut self, script: Script) -> Result<(), SendError>;
    fn is_poisoned(&self) -> bool;
}

/// `state()` and `try_state()`. The safe mirror enum cannot implement this.
pub trait StateAccess<M> {
    /// Panics when poisoned.
    fn state(&self) -> &M;
    fn try_state(&self) -> Option<&M>;
}

/// Conversions between the user enum `M` and an internal mirror enum.
pub trait Mirrored: Spec {
    type Repr;
    fn into_repr(self) -> Self::Repr;
    /// `None` when the representation is the poisoned variant.
    fn from_repr(repr: Self::Repr) -> Option<Self>;
    fn poisoned_repr() -> Self::Repr;
    fn is_poisoned_repr(repr: &Self::Repr) -> bool;
}
