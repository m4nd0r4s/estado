//! Auxiliary per-machine state used by the step algorithm.

use core::marker::PhantomData;

use crate::queue::Queues;

/// Queue operations the step algorithm needs (spec 5.4).
pub trait EventQueues {
    /// Event type.
    type Event;

    /// Queues an internal event; gives it back when there is no room.
    fn raise(&mut self, event: Self::Event) -> Result<(), Self::Event>;
    /// Next internal event.
    fn pop_raised(&mut self) -> Option<Self::Event>;
    /// Defers an event; gives it back when there is no room.
    fn defer(&mut self, event: Self::Event) -> Result<(), Self::Event>;
    /// Drops the oldest deferred event.
    fn drop_oldest_deferred(&mut self) -> Option<Self::Event>;
    /// Moves the deferred events to the requeue (configuration changed).
    fn requeue_deferred(&mut self);
    /// Next requeued event.
    fn pop_requeued(&mut self) -> Option<Self::Event>;
}

/// The `Runtime` associated type of a [`Spec`](crate::Spec).
pub trait Runtime {
    /// Event type.
    type Event;
    /// Queues used by `cx.raise` and `defer!()`.
    type Queues: EventQueues<Event = Self::Event>;

    /// Initial value; `const` so that `Machine::new` is a `const fn`.
    const NEW: Self;

    /// The queues.
    fn queues(&mut self) -> &mut Self::Queues;

    /// Whether an `#[always]` evaluation is pending; clears the flag.
    fn take_always_pending(&mut self) -> bool {
        false
    }
}

/// Runtime of a machine without raise, defer, always, timers or history:
/// zero-sized, every queue has capacity zero.
///
/// ```
/// assert_eq!(core::mem::size_of::<estado_core::NoRuntime<u64>>(), 0);
/// ```
pub struct NoRuntime<Event>(PhantomData<fn() -> Event>);

impl<Event> EventQueues for NoRuntime<Event> {
    type Event = Event;

    #[inline]
    fn raise(&mut self, event: Event) -> Result<(), Event> {
        Err(event)
    }
    #[inline]
    fn pop_raised(&mut self) -> Option<Event> {
        None
    }
    #[inline]
    fn defer(&mut self, event: Event) -> Result<(), Event> {
        Err(event)
    }
    #[inline]
    fn drop_oldest_deferred(&mut self) -> Option<Event> {
        None
    }
    #[inline]
    fn requeue_deferred(&mut self) {}
    #[inline]
    fn pop_requeued(&mut self) -> Option<Event> {
        None
    }
}

impl<Event> Runtime for NoRuntime<Event> {
    type Event = Event;
    type Queues = Self;
    const NEW: Self = Self(PhantomData);

    #[inline]
    fn queues(&mut self) -> &mut Self {
        self
    }
}

impl<Event, const RAISE: usize, const DEFER: usize> EventQueues for Queues<Event, RAISE, DEFER> {
    type Event = Event;

    fn raise(&mut self, event: Event) -> Result<(), Event> {
        self.raised.push(event)
    }
    fn pop_raised(&mut self) -> Option<Event> {
        self.raised.pop()
    }
    fn defer(&mut self, event: Event) -> Result<(), Event> {
        self.deferred.push(event)
    }
    fn drop_oldest_deferred(&mut self) -> Option<Event> {
        self.deferred.pop()
    }
    fn requeue_deferred(&mut self) {
        Queues::requeue_deferred(self);
    }
    fn pop_requeued(&mut self) -> Option<Event> {
        self.requeued.pop()
    }
}
