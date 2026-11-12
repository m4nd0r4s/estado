//! Event queues of the run-to-completion algorithm (spec 5.4).

/// Fixed-capacity FIFO without allocation.
///
/// ```
/// use estado_core::InlineQueue;
/// let mut queue = InlineQueue::<u8, 2>::new();
/// assert_eq!(queue.push(1), Ok(()));
/// assert_eq!(queue.push(2), Ok(()));
/// assert_eq!(queue.push(3), Err(3));
/// assert_eq!(queue.pop(), Some(1));
/// assert_eq!(queue.len(), 1);
/// ```
#[derive(Debug)]
pub struct InlineQueue<Event, const CAPACITY: usize> {
    slots: [Option<Event>; CAPACITY],
    head: usize,
    length: usize,
}

impl<Event, const CAPACITY: usize> InlineQueue<Event, CAPACITY> {
    /// An empty queue.
    pub const fn new() -> Self {
        Self {
            slots: [const { None }; CAPACITY],
            head: 0,
            length: 0,
        }
    }

    /// Appends at the back.
    ///
    /// # Errors
    ///
    /// Gives the event back when the queue is full.
    pub fn push(&mut self, event: Event) -> Result<(), Event> {
        if self.length == CAPACITY {
            return Err(event);
        }
        self.slots[(self.head + self.length) % CAPACITY] = Some(event);
        self.length += 1;
        Ok(())
    }

    /// Removes from the front.
    pub fn pop(&mut self) -> Option<Event> {
        if self.length == 0 {
            return None;
        }
        let event = self.slots[self.head].take();
        self.head = (self.head + 1) % CAPACITY;
        self.length -= 1;
        event
    }

    /// Number of queued events.
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Whether the queue is empty.
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Iterates front to back without removing.
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        (0..self.length)
            .filter_map(move |offset| self.slots[(self.head + offset) % CAPACITY].as_ref())
    }
}

impl<Event, const CAPACITY: usize> Default for InlineQueue<Event, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

/// The three queues of spec 5.4 plus the pending-`#[always]` flag, ready to
/// embed in a generated [`Runtime`](crate::Runtime).
///
/// `RAISE` is `raise_cap`, `DEFER` is `defer_cap`; the requeue has the same
/// capacity as the defer queue.
#[derive(Debug)]
pub struct Queues<Event, const RAISE: usize, const DEFER: usize> {
    /// Internal events (`cx.raise`).
    pub raised: InlineQueue<Event, RAISE>,
    /// Deferred events, kept across `send`s.
    pub deferred: InlineQueue<Event, DEFER>,
    /// Deferred events being redelivered after a configuration change.
    pub requeued: InlineQueue<Event, DEFER>,
}

impl<Event, const RAISE: usize, const DEFER: usize> Queues<Event, RAISE, DEFER> {
    /// Empty queues.
    pub const fn new() -> Self {
        Self {
            raised: InlineQueue::new(),
            deferred: InlineQueue::new(),
            requeued: InlineQueue::new(),
        }
    }

    /// Moves every deferred event to the requeue (configuration changed).
    pub fn requeue_deferred(&mut self) {
        while let Some(event) = self.deferred.pop() {
            if let Err(event) = self.requeued.push(event) {
                // Requeue and defer share a capacity, and the requeue is
                // drained before it can fill: put it back rather than lose it.
                let _ = self.deferred.push(event);
                break;
            }
        }
    }
}

impl<Event, const RAISE: usize, const DEFER: usize> Default for Queues<Event, RAISE, DEFER> {
    fn default() -> Self {
        Self::new()
    }
}
