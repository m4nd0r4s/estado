//! A root enum whose payload counts creations and drops, to catch leaks and
//! double drops in every panic scenario.

use crate::api::Spec;
use std::cell::Cell;

thread_local! {
    static CREATED: Cell<usize> = const { Cell::new(0) };
    static DROPPED: Cell<usize> = const { Cell::new(0) };
}

#[derive(Debug, PartialEq)]
pub struct Probe(());

impl Probe {
    pub fn new() -> Self {
        CREATED.with(|created| created.set(created.get() + 1));
        Probe(())
    }
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        DROPPED.with(|dropped| dropped.set(dropped.get() + 1));
    }
}

/// Probes created and not yet dropped on this thread.
pub fn live_probes() -> usize {
    CREATED.with(Cell::get) - DROPPED.with(Cell::get)
}

#[derive(Debug, PartialEq)]
pub enum Tracked {
    Idle,
    Holding(Probe),
}

impl Spec for Tracked {
    fn initial() -> Self {
        Tracked::Holding(Probe::new())
    }
    fn next(self) -> Self {
        match self {
            Tracked::Idle => Tracked::Holding(Probe::new()),
            Tracked::Holding(_) => Tracked::Idle,
        }
    }
}
