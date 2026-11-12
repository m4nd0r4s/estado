//! Minimal stand-in for `estado-core`: just enough to type-check the
//! typed-edge technique of spec 4.2.

/// Result of a handler. `E` is the edge set of that handler.
pub enum Next<E> {
    Stay,
    Bubble,
    Unhandled,
    Defer,
    Reenter,
    Goto(E),
}

/// Records what the transition code did, so tests can check the order.
#[derive(Default, Debug)]
pub struct Context {
    pub log: Vec<String>,
}

impl Context {
    pub fn note(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
    }
}

/// Implemented by every generated edge enum. `apply` runs spec 5.3 for the
/// chosen edge: exits, take, constructor, write, entries.
pub trait Edge<Machine> {
    const TARGETS: &'static [&'static str];
    fn apply(self, machine: &mut Machine, context: &mut Context);
}

/// Edge set of a handler without any `goto!`.
pub enum NoEdge {}

impl<Machine> Edge<Machine> for NoEdge {
    const TARGETS: &'static [&'static str] = &[];
    fn apply(self, _: &mut Machine, _: &mut Context) {
        match self {}
    }
}
