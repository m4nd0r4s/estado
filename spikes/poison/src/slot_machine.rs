//! Recommended shape: option (b) generalised to a three-way slot owned by
//! `estado-core`, so the macro generates nothing for poisoning.
//!
//! `Unstarted` is needed anyway by spec 4.5 (`new` runs no entries, `send`
//! starts implicitly); `Poisoned` replaces `None`. The compiler packs both
//! into the niches of `M` exactly like `Option<M>`, when `M` has two spare
//! niche values.

use crate::api::{MachineApi, Script, SendError, Spec, StateAccess};

pub enum Slot<M> {
    Unstarted,
    Live(M),
    Poisoned,
}

pub struct SlotMachine<M, C> {
    slot: Slot<M>,
    ctx: C,
}

struct PoisonOnUnwind<'slot, M>(&'slot mut Slot<M>);

impl<M> Drop for PoisonOnUnwind<'_, M> {
    fn drop(&mut self) {
        *self.0 = Slot::Poisoned;
    }
}

impl<M: Spec, C> SlotMachine<M, C> {
    /// Like `Machine::new`: no value exists until the first `send`/`start`.
    pub fn unstarted(ctx: C) -> Self {
        Self {
            slot: Slot::Unstarted,
            ctx,
        }
    }

    pub fn is_started(&self) -> bool {
        !matches!(self.slot, Slot::Unstarted)
    }

    pub fn ctx(&self) -> &C {
        &self.ctx
    }
}

impl<M: Spec, C> MachineApi<M, C> for SlotMachine<M, C> {
    fn create(initial_state: M, ctx: C) -> Self {
        Self {
            slot: Slot::Live(initial_state),
            ctx,
        }
    }

    fn send(&mut self, script: Script) -> Result<(), SendError> {
        let guard = PoisonOnUnwind(&mut self.slot);
        if matches!(guard.0, Slot::Unstarted) {
            // Implicit start: initial value, then its entries, in place.
            let mut initial = M::initial();
            initial.enter(Script::Plain);
            *guard.0 = Slot::Live(initial);
        }
        match guard.0 {
            Slot::Live(current) => current.exit(script), // steps 1-3, in place
            _ => return Err(SendError::Poisoned),
        }
        let Slot::Live(old) = core::mem::replace(guard.0, Slot::Poisoned) else {
            return Err(SendError::Poisoned); // step 4: now poisoned
        };
        let new = old.construct(script); // step 5
        *guard.0 = Slot::Live(new); // step 6
        if let Slot::Live(written) = guard.0 {
            written.enter(script); // step 7, in place
        }
        core::mem::forget(guard);
        Ok(())
    }

    fn is_poisoned(&self) -> bool {
        matches!(self.slot, Slot::Poisoned)
    }
}

impl<M, C> StateAccess<M> for SlotMachine<M, C> {
    fn state(&self) -> &M {
        match &self.slot {
            Slot::Live(state) => state,
            Slot::Poisoned => panic!("estado: machine is poisoned by an earlier panic"),
            Slot::Unstarted => panic!("estado: machine has not been started"),
        }
    }
    fn try_state(&self) -> Option<&M> {
        match &self.slot {
            Slot::Live(state) => Some(state),
            _ => None,
        }
    }
}
