//! Option (a), safe form: the machine stores `M::Repr`, an internal mirror enum
//! with the extra `Poisoned` variant. No unsafe, but `state() -> &M` cannot be
//! implemented: there is no `M` in memory to borrow. Every send converts
//! `Repr -> M -> Repr` by value (the repr stays poisoned meanwhile, so no guard).

use crate::api::{MachineApi, Mirrored, Script, SendError};

pub struct MirrorMachine<M: Mirrored, C> {
    repr: M::Repr,
    ctx: C,
}

impl<M: Mirrored, C> MachineApi<M, C> for MirrorMachine<M, C> {
    fn create(initial_state: M, ctx: C) -> Self {
        Self {
            repr: initial_state.into_repr(),
            ctx,
        }
    }

    fn send(&mut self, script: Script) -> Result<(), SendError> {
        let taken = core::mem::replace(&mut self.repr, M::poisoned_repr());
        let Some(mut current) = M::from_repr(taken) else {
            return Err(SendError::Poisoned);
        };
        current.exit(script);
        let mut new = current.construct(script);
        new.enter(script);
        self.repr = new.into_repr();
        Ok(())
    }

    fn is_poisoned(&self) -> bool {
        M::is_poisoned_repr(&self.repr)
    }
}

impl<M: Mirrored, C> MirrorMachine<M, C> {
    pub fn ctx(&self) -> &C {
        &self.ctx
    }
}
