//! Where effects go (spec 4.4).

use crate::effect::Effect;

/// The effect buffer is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overflow;

/// Receives the effects of a `send`.
///
/// ```
/// use estado_core::{Discard, Effect, Sink};
/// let mut sink = Discard;
/// assert!(Sink::<u8>::push(&mut sink, Effect::User(1)).is_ok());
/// ```
pub trait Sink<Command> {
    /// Stores one effect.
    ///
    /// # Errors
    ///
    /// [`Overflow`] when the sink has no room left.
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow>;
}

/// Discards every effect; for machines with `effect = ()` or tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct Discard;

impl<Command> Sink<Command> for Discard {
    #[inline]
    fn push(&mut self, _: Effect<Command>) -> Result<(), Overflow> {
        Ok(())
    }
}

/// Adapts a closure into a sink that never overflows.
///
/// ```
/// use estado_core::{Effect, FnSink, Sink};
/// let mut seen = 0;
/// let mut sink = FnSink(|_: Effect<u8>| seen += 1);
/// sink.push(Effect::User(1)).unwrap();
/// assert_eq!(seen, 1);
/// ```
pub struct FnSink<Function>(pub Function);

impl<Command, Function: FnMut(Effect<Command>)> Sink<Command> for FnSink<Function> {
    #[inline]
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow> {
        (self.0)(effect);
        Ok(())
    }
}

impl<Command, Inner: Sink<Command> + ?Sized> Sink<Command> for &mut Inner {
    #[inline]
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow> {
        (**self).push(effect)
    }
}

#[cfg(feature = "alloc")]
impl<Command> Sink<Command> for alloc::vec::Vec<Effect<Command>> {
    #[inline]
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow> {
        alloc::vec::Vec::push(self, effect);
        Ok(())
    }
}

/// Effect buffer of a [`Step`](crate::Step): `CAPACITY` effects inline, then
/// spill to the heap with `alloc`, or [`Overflow`] without it.
///
/// ```
/// use estado_core::{Effect, InlineEffects, Sink};
/// let mut effects = InlineEffects::<u8, 2>::new();
/// effects.push(Effect::User(1)).unwrap();
/// effects.push(Effect::User(2)).unwrap();
/// assert_eq!(effects.len(), 2);
/// assert_eq!(effects.iter().count(), 2);
/// ```
pub struct InlineEffects<Command, const CAPACITY: usize> {
    inline: [Option<Effect<Command>>; CAPACITY],
    length: usize,
    #[cfg(feature = "alloc")]
    spilled: alloc::vec::Vec<Effect<Command>>,
}

impl<Command, const CAPACITY: usize> InlineEffects<Command, CAPACITY> {
    /// An empty buffer.
    pub const fn new() -> Self {
        Self {
            inline: [const { None }; CAPACITY],
            length: 0,
            #[cfg(feature = "alloc")]
            spilled: alloc::vec::Vec::new(),
        }
    }

    /// Number of effects.
    pub fn len(&self) -> usize {
        self.length
    }

    /// Whether there is no effect.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// The effects, in emission order.
    pub fn iter(&self) -> impl Iterator<Item = &Effect<Command>> {
        let inline = self.inline.iter().filter_map(Option::as_ref);
        #[cfg(feature = "alloc")]
        let inline = inline.chain(self.spilled.iter());
        inline
    }

    /// The user effects only, in emission order.
    pub fn commands(&self) -> impl Iterator<Item = &Command> {
        self.iter().filter_map(|effect| match effect {
            Effect::User(command) => Some(command),
            _ => None,
        })
    }
}

impl<Command, const CAPACITY: usize> Default for InlineEffects<Command, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Command: core::fmt::Debug, const CAPACITY: usize> core::fmt::Debug
    for InlineEffects<Command, CAPACITY>
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl<Command, const CAPACITY: usize> Sink<Command> for InlineEffects<Command, CAPACITY> {
    #[inline]
    fn push(&mut self, effect: Effect<Command>) -> Result<(), Overflow> {
        if let Some(slot) = self.inline.get_mut(self.length) {
            *slot = Some(effect);
        } else {
            #[cfg(feature = "alloc")]
            self.spilled.push(effect);
            #[cfg(not(feature = "alloc"))]
            {
                let _ = effect;
                return Err(Overflow);
            }
        }
        self.length += 1;
        Ok(())
    }
}

impl<Command, const CAPACITY: usize> IntoIterator for InlineEffects<Command, CAPACITY> {
    type Item = Effect<Command>;
    type IntoIter = IntoEffects<Command, CAPACITY>;

    fn into_iter(self) -> Self::IntoIter {
        IntoEffects {
            inline: self.inline.into_iter(),
            #[cfg(feature = "alloc")]
            spilled: self.spilled.into_iter(),
        }
    }
}

/// Owning iterator over [`InlineEffects`].
pub struct IntoEffects<Command, const CAPACITY: usize> {
    inline: core::array::IntoIter<Option<Effect<Command>>, CAPACITY>,
    #[cfg(feature = "alloc")]
    spilled: alloc::vec::IntoIter<Effect<Command>>,
}

impl<Command, const CAPACITY: usize> Iterator for IntoEffects<Command, CAPACITY> {
    type Item = Effect<Command>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(effect) = self.inline.by_ref().flatten().next() {
            return Some(effect);
        }
        #[cfg(feature = "alloc")]
        return self.spilled.next();
        #[cfg(not(feature = "alloc"))]
        None
    }
}
