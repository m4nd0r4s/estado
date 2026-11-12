//! Option (a), unsafe form: like the safe mirror, but `state() -> &M` is a
//! pointer cast from `&Repr`. Sound only if both enums have a defined layout
//! (`#[repr(u8)]` or `#[repr(C)]`), which forfeits the compiler's niche and
//! field-reordering optimisations for the user's enum. The generated `unsafe`
//! also lands in the user's crate (breaks `#![forbid(unsafe_code)]`).

use crate::api::{MachineApi, Mirrored, Script, SendError, StateAccess};

/// # Safety
/// `Self` and `Self::Repr` must both be `#[repr(u8)]` (or `#[repr(C)]`) enums
/// with identical variants, in identical order, with identical fields; `Repr`
/// may only add variants after the last shared one. Then a `Repr` that is not
/// one of the extra variants has exactly the layout and validity of a `Self`.
#[allow(unsafe_code)]
pub unsafe trait LayoutMirror: Mirrored {}

#[allow(unsafe_code)]
unsafe impl LayoutMirror for crate::fixtures_marked::Marked {}

pub struct MirrorCastMachine<M: LayoutMirror, C> {
    repr: M::Repr,
    ctx: C,
}

struct PoisonOnUnwind<'slot, M: Mirrored>(&'slot mut M::Repr);

impl<M: Mirrored> Drop for PoisonOnUnwind<'_, M> {
    fn drop(&mut self) {
        *self.0 = M::poisoned_repr();
    }
}

/// # Safety
/// `repr` must not be a poisoned representation (see [`LayoutMirror`]).
#[allow(unsafe_code)]
unsafe fn view<M: LayoutMirror>(repr: &M::Repr) -> &M {
    // SAFETY: caller guarantees a non-poisoned repr; `LayoutMirror` guarantees layout.
    unsafe { &*core::ptr::from_ref(repr).cast::<M>() }
}

/// # Safety
/// Same as [`view`].
#[allow(unsafe_code)]
unsafe fn view_mut<M: LayoutMirror>(repr: &mut M::Repr) -> &mut M {
    // SAFETY: caller guarantees a non-poisoned repr; `LayoutMirror` guarantees layout.
    unsafe { &mut *core::ptr::from_mut(repr).cast::<M>() }
}

#[allow(unsafe_code)]
impl<M: LayoutMirror, C> MachineApi<M, C> for MirrorCastMachine<M, C> {
    fn create(initial_state: M, ctx: C) -> Self {
        Self {
            repr: initial_state.into_repr(),
            ctx,
        }
    }

    fn send(&mut self, script: Script) -> Result<(), SendError> {
        if M::is_poisoned_repr(&self.repr) {
            return Err(SendError::Poisoned);
        }
        let guard = PoisonOnUnwind::<M>(&mut self.repr);
        // SAFETY: checked non-poisoned above.
        unsafe { view_mut::<M>(guard.0) }.exit(script);
        let taken = core::mem::replace(guard.0, M::poisoned_repr());
        let old = M::from_repr(taken).ok_or(SendError::Poisoned)?;
        *guard.0 = old.construct(script).into_repr();
        // SAFETY: just written from an `M`, so non-poisoned.
        unsafe { view_mut::<M>(guard.0) }.enter(script);
        core::mem::forget(guard);
        Ok(())
    }

    fn is_poisoned(&self) -> bool {
        M::is_poisoned_repr(&self.repr)
    }
}

#[allow(unsafe_code)]
impl<M: LayoutMirror, C> StateAccess<M> for MirrorCastMachine<M, C> {
    fn state(&self) -> &M {
        self.try_state()
            .expect("estado: machine is poisoned by an earlier panic")
    }
    fn try_state(&self) -> Option<&M> {
        // SAFETY: checked non-poisoned in the same expression.
        (!M::is_poisoned_repr(&self.repr)).then(|| unsafe { view::<M>(&self.repr) })
    }
}

impl<M: LayoutMirror, C> MirrorCastMachine<M, C> {
    pub fn ctx(&self) -> &C {
        &self.ctx
    }
}
