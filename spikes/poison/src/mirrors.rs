//! Mirror enums (default layout) for the safe mirror variant. This is the
//! boilerplate the macro would generate for every root enum.

use crate::api::Mirrored;
use crate::fixtures_flat::{Boxed, Only, Owned, Unit};
use crate::fixtures_player::{Active, Player};

pub enum UnitRepr {
    Red,
    Green,
    Blue,
    Poisoned,
}

impl Mirrored for Unit {
    type Repr = UnitRepr;
    fn into_repr(self) -> UnitRepr {
        match self {
            Unit::Red => UnitRepr::Red,
            Unit::Green => UnitRepr::Green,
            Unit::Blue => UnitRepr::Blue,
        }
    }
    fn from_repr(repr: UnitRepr) -> Option<Self> {
        Some(match repr {
            UnitRepr::Red => Unit::Red,
            UnitRepr::Green => Unit::Green,
            UnitRepr::Blue => Unit::Blue,
            UnitRepr::Poisoned => return None,
        })
    }
    fn poisoned_repr() -> UnitRepr {
        UnitRepr::Poisoned
    }
    fn is_poisoned_repr(repr: &UnitRepr) -> bool {
        matches!(repr, UnitRepr::Poisoned)
    }
}

pub enum BoxedRepr {
    Idle,
    Data(Box<u32>),
    Poisoned,
}

impl Mirrored for Boxed {
    type Repr = BoxedRepr;
    fn into_repr(self) -> BoxedRepr {
        match self {
            Boxed::Idle => BoxedRepr::Idle,
            Boxed::Data(data) => BoxedRepr::Data(data),
        }
    }
    fn from_repr(repr: BoxedRepr) -> Option<Self> {
        Some(match repr {
            BoxedRepr::Idle => Boxed::Idle,
            BoxedRepr::Data(data) => Boxed::Data(data),
            BoxedRepr::Poisoned => return None,
        })
    }
    fn poisoned_repr() -> BoxedRepr {
        BoxedRepr::Poisoned
    }
    fn is_poisoned_repr(repr: &BoxedRepr) -> bool {
        matches!(repr, BoxedRepr::Poisoned)
    }
}

pub enum OwnedRepr {
    Idle,
    Failed { reason: String },
    Poisoned,
}

impl Mirrored for Owned {
    type Repr = OwnedRepr;
    fn into_repr(self) -> OwnedRepr {
        match self {
            Owned::Idle => OwnedRepr::Idle,
            Owned::Failed { reason } => OwnedRepr::Failed { reason },
        }
    }
    fn from_repr(repr: OwnedRepr) -> Option<Self> {
        Some(match repr {
            OwnedRepr::Idle => Owned::Idle,
            OwnedRepr::Failed { reason } => Owned::Failed { reason },
            OwnedRepr::Poisoned => return None,
        })
    }
    fn poisoned_repr() -> OwnedRepr {
        OwnedRepr::Poisoned
    }
    fn is_poisoned_repr(repr: &OwnedRepr) -> bool {
        matches!(repr, OwnedRepr::Poisoned)
    }
}

pub enum OnlyRepr {
    Value(u32),
    Poisoned,
}

impl Mirrored for Only {
    type Repr = OnlyRepr;
    fn into_repr(self) -> OnlyRepr {
        let Only::Value(value) = self;
        OnlyRepr::Value(value)
    }
    fn from_repr(repr: OnlyRepr) -> Option<Self> {
        match repr {
            OnlyRepr::Value(value) => Some(Only::Value(value)),
            OnlyRepr::Poisoned => None,
        }
    }
    fn poisoned_repr() -> OnlyRepr {
        OnlyRepr::Poisoned
    }
    fn is_poisoned_repr(repr: &OnlyRepr) -> bool {
        matches!(repr, OnlyRepr::Poisoned)
    }
}

/// The mirror shares the payload structs with the user enum, so only the
/// root enum is duplicated.
pub enum PlayerRepr {
    Idle,
    Active(Active),
    Failed { reason: String },
    Poisoned,
}

impl Mirrored for Player {
    type Repr = PlayerRepr;
    fn into_repr(self) -> PlayerRepr {
        match self {
            Player::Idle => PlayerRepr::Idle,
            Player::Active(active) => PlayerRepr::Active(active),
            Player::Failed { reason } => PlayerRepr::Failed { reason },
        }
    }
    fn from_repr(repr: PlayerRepr) -> Option<Self> {
        Some(match repr {
            PlayerRepr::Idle => Player::Idle,
            PlayerRepr::Active(active) => Player::Active(active),
            PlayerRepr::Failed { reason } => Player::Failed { reason },
            PlayerRepr::Poisoned => return None,
        })
    }
    fn poisoned_repr() -> PlayerRepr {
        PlayerRepr::Poisoned
    }
    fn is_poisoned_repr(repr: &PlayerRepr) -> bool {
        matches!(repr, PlayerRepr::Poisoned)
    }
}
