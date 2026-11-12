//! Flat root enums: unit-only, niche-bearing data, and enums without niche.

use crate::api::Spec;
use core::num::NonZeroU32;

#[derive(Debug, PartialEq)]
pub enum Unit {
    Red,
    Green,
    Blue,
}

impl Spec for Unit {
    fn initial() -> Self {
        Unit::Red
    }
    fn next(self) -> Self {
        match self {
            Unit::Red => Unit::Green,
            Unit::Green => Unit::Blue,
            Unit::Blue => Unit::Red,
        }
    }
}

/// `Idle` already consumes the only niche of `Box` (null).
#[derive(Debug, PartialEq)]
pub enum Boxed {
    Idle,
    Data(Box<u32>),
}

impl Spec for Boxed {
    fn initial() -> Self {
        Boxed::Idle
    }
    fn next(self) -> Self {
        match self {
            Boxed::Idle => Boxed::Data(Box::new(7)),
            Boxed::Data(_) => Boxed::Idle,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Strs {
    Idle,
    Name(&'static str),
    Count(NonZeroU32),
}

impl Spec for Strs {
    fn initial() -> Self {
        Strs::Idle
    }
    fn next(self) -> Self {
        match self {
            Strs::Idle => Strs::Name("name"),
            Strs::Name(_) => Strs::Count(NonZeroU32::MIN),
            Strs::Count(_) => Strs::Idle,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Owned {
    Idle,
    Failed { reason: String },
}

impl Spec for Owned {
    fn initial() -> Self {
        Owned::Idle
    }
    fn next(self) -> Self {
        match self {
            Owned::Idle => Owned::Failed {
                reason: "boom".to_string(),
            },
            Owned::Failed { .. } => Owned::Idle,
        }
    }
}

/// Tag has 254 spare values: a niche exists in the tag.
#[derive(Debug, PartialEq)]
pub enum Pair {
    First(u8),
    Second(u8),
}

impl Spec for Pair {
    fn initial() -> Self {
        Pair::First(0)
    }
    fn next(self) -> Self {
        match self {
            Pair::First(value) => Pair::Second(value.wrapping_add(1)),
            Pair::Second(value) => Pair::First(value.wrapping_add(1)),
        }
    }
}

/// One variant, every bit pattern valid: no niche at all.
#[derive(Debug, PartialEq)]
pub enum Only {
    Value(u32),
}

impl Spec for Only {
    fn initial() -> Self {
        Only::Value(0)
    }
    fn next(self) -> Self {
        let Only::Value(value) = self;
        Only::Value(value.wrapping_add(1))
    }
}
