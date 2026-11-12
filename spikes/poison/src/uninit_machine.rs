//! `MaybeUninit<M>` + `bool` with a drop guard. Internal `unsafe`, no
//! placeholder needed. Same size as the flag variant, more risk.
//!
//! Invariant: `poisoned == false` iff `state` holds an initialised `M`.

use crate::api::{MachineApi, Script, SendError, Spec, StateAccess};
use core::mem::MaybeUninit;

pub struct UninitMachine<M, C> {
    state: MaybeUninit<M>,
    ctx: C,
    poisoned: bool,
}

/// While armed it owns the duty to restore the invariant on unwind.
struct Guard<'machine, M> {
    state: &'machine mut MaybeUninit<M>,
    poisoned: &'machine mut bool,
    holds_value: bool,
}

impl<M> Drop for Guard<'_, M> {
    fn drop(&mut self) {
        if self.holds_value {
            // SAFETY: `holds_value` is true only while `state` is initialised.
            unsafe { self.state.assume_init_drop() };
        }
        *self.poisoned = true;
    }
}

impl<M: Spec, C> MachineApi<M, C> for UninitMachine<M, C> {
    fn create(initial_state: M, ctx: C) -> Self {
        Self {
            state: MaybeUninit::new(initial_state),
            ctx,
            poisoned: false,
        }
    }

    fn send(&mut self, script: Script) -> Result<(), SendError> {
        if self.poisoned {
            return Err(SendError::Poisoned);
        }
        let mut guard = Guard {
            state: &mut self.state,
            poisoned: &mut self.poisoned,
            holds_value: true,
        };
        // SAFETY: not poisoned, so `state` is initialised; `holds_value` is true.
        unsafe { guard.state.assume_init_mut() }.exit(script);
        // SAFETY: initialised (above). The value is moved out and `holds_value`
        // is cleared right away, so it is never dropped twice.
        let old = unsafe { guard.state.assume_init_read() };
        guard.holds_value = false;
        let new = old.construct(script);
        guard.state.write(new);
        guard.holds_value = true;
        // SAFETY: just written.
        unsafe { guard.state.assume_init_mut() }.enter(script);
        core::mem::forget(guard);
        Ok(())
    }

    fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

impl<M, C> StateAccess<M> for UninitMachine<M, C> {
    fn state(&self) -> &M {
        self.try_state()
            .expect("estado: machine is poisoned by an earlier panic")
    }
    fn try_state(&self) -> Option<&M> {
        // SAFETY: `poisoned == false` means `state` is initialised.
        (!self.poisoned).then(|| unsafe { self.state.assume_init_ref() })
    }
}

impl<M, C> Drop for UninitMachine<M, C> {
    fn drop(&mut self) {
        if !self.poisoned {
            // SAFETY: not poisoned, so `state` is initialised and dropped once.
            unsafe { self.state.assume_init_drop() };
        }
    }
}

impl<M, C> UninitMachine<M, C> {
    pub fn ctx(&self) -> &C {
        &self.ctx
    }
}
