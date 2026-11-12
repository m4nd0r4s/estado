//! `#[repr(u8)]` root enum and its mirror, needed by the unsafe cast variant.
//! `MarkedPlain` is the same enum with the default layout, for size comparison.

use crate::api::{Mirrored, Spec};

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum Marked {
    Idle,
    Counting(u32),
    Failed { reason: String },
}

/// Same variants, same order, same fields, plus the hidden one at the end.
#[repr(u8)]
pub enum MarkedRepr {
    Idle,
    Counting(u32),
    Failed { reason: String },
    Poisoned,
}

#[derive(Debug, PartialEq)]
pub enum MarkedPlain {
    Idle,
    Counting(u32),
    Failed { reason: String },
}

impl Spec for Marked {
    fn initial() -> Self {
        Marked::Idle
    }
    fn next(self) -> Self {
        match self {
            Marked::Idle => Marked::Counting(1),
            Marked::Counting(_) => Marked::Failed {
                reason: "boom".to_string(),
            },
            Marked::Failed { .. } => Marked::Idle,
        }
    }
}

impl Mirrored for Marked {
    type Repr = MarkedRepr;
    fn into_repr(self) -> MarkedRepr {
        match self {
            Marked::Idle => MarkedRepr::Idle,
            Marked::Counting(count) => MarkedRepr::Counting(count),
            Marked::Failed { reason } => MarkedRepr::Failed { reason },
        }
    }
    fn from_repr(repr: MarkedRepr) -> Option<Self> {
        Some(match repr {
            MarkedRepr::Idle => Marked::Idle,
            MarkedRepr::Counting(count) => Marked::Counting(count),
            MarkedRepr::Failed { reason } => Marked::Failed { reason },
            MarkedRepr::Poisoned => return None,
        })
    }
    fn poisoned_repr() -> MarkedRepr {
        MarkedRepr::Poisoned
    }
    fn is_poisoned_repr(repr: &MarkedRepr) -> bool {
        matches!(repr, MarkedRepr::Poisoned)
    }
}
