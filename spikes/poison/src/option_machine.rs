//! Option (b): `Option<M>` with `take()`. No unsafe, no placeholder.
//!
//! `None` is the poisoned state. The take (step 4) already leaves `None`, so a
//! panic in the constructor needs no guard. The guard only covers the in-place
//! phases (handler, exits, entries): on unwind it drops the half-changed value.

use crate::api::{MachineApi, Script, SendError, Spec, StateAccess};

pub struct OptionMachine<M, C> {
    state: Option<M>,
    ctx: C,
}

struct PoisonOnUnwind<'slot, M>(&'slot mut Option<M>);

impl<M> Drop for PoisonOnUnwind<'_, M> {
    fn drop(&mut self) {
        *self.0 = None;
    }
}

impl<M, C> OptionMachine<M, C> {
    pub fn ctx(&self) -> &C {
        &self.ctx
    }
}

impl<M: Spec, C> MachineApi<M, C> for OptionMachine<M, C> {
    fn create(initial_state: M, ctx: C) -> Self {
        Self {
            state: Some(initial_state),
            ctx,
        }
    }

    fn send(&mut self, script: Script) -> Result<(), SendError> {
        let guard = PoisonOnUnwind(&mut self.state);
        let Some(current) = guard.0.as_mut() else {
            return Err(SendError::Poisoned);
        };
        current.exit(script); // steps 1-3, in place
        let old = guard.0.take().ok_or(SendError::Poisoned)?; // step 4: now poisoned
        let new = old.construct(script); // step 5
        let written = guard.0.insert(new); // step 6: no longer poisoned
        written.enter(script); // step 7, in place
        core::mem::forget(guard); // disarm: compiles to nothing on the happy path
        Ok(())
    }

    fn is_poisoned(&self) -> bool {
        self.state.is_none()
    }
}

impl<M, C> StateAccess<M> for OptionMachine<M, C> {
    fn state(&self) -> &M {
        self.state
            .as_ref()
            .expect("estado: machine is poisoned by an earlier panic")
    }
    fn try_state(&self) -> Option<&M> {
        self.state.as_ref()
    }
}
