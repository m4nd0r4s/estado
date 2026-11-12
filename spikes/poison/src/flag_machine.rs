//! Flag + placeholder: `M` and a `bool`; `mem::replace` against a cheap
//! placeholder value while the old value is taken. No unsafe.
//!
//! Needs a placeholder `M`. The spike uses `Spec::initial()`; in the real
//! library `initial()` may need `Cx` or allocate, so this is not generally
//! available. The flag is extra state: it costs bytes unless padding absorbs it.

use crate::api::{MachineApi, Script, SendError, Spec, StateAccess};

pub struct FlagMachine<M, C> {
    state: M,
    ctx: C,
    poisoned: bool,
}

struct SetFlagOnUnwind<'flag>(&'flag mut bool);

impl Drop for SetFlagOnUnwind<'_> {
    fn drop(&mut self) {
        *self.0 = true;
    }
}

impl<M: Spec, C> MachineApi<M, C> for FlagMachine<M, C> {
    fn create(initial_state: M, ctx: C) -> Self {
        Self {
            state: initial_state,
            ctx,
            poisoned: false,
        }
    }

    fn send(&mut self, script: Script) -> Result<(), SendError> {
        if self.poisoned {
            return Err(SendError::Poisoned);
        }
        let guard = SetFlagOnUnwind(&mut self.poisoned);
        self.state.exit(script);
        let old = core::mem::replace(&mut self.state, M::initial()); // steps 4: placeholder
        self.state = old.construct(script); // steps 5-6
        self.state.enter(script);
        core::mem::forget(guard);
        Ok(())
    }

    fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

impl<M, C> StateAccess<M> for FlagMachine<M, C> {
    fn state(&self) -> &M {
        assert!(
            !self.poisoned,
            "estado: machine is poisoned by an earlier panic"
        );
        &self.state
    }
    fn try_state(&self) -> Option<&M> {
        (!self.poisoned).then_some(&self.state)
    }
}

impl<M, C> FlagMachine<M, C> {
    pub fn ctx(&self) -> &C {
        &self.ctx
    }
}
