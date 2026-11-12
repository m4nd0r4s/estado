//! The warning codes of spec 6.7.

/// A warning of the macros. `W0004` is the only one that needs just the
/// state tree; the others need the edges or the handlers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WarningCode {
    /// `W0001`: state unreachable from the initial state.
    UnreachableState,
    /// `W0002`: state with no outgoing edges that is not `#[terminal]`.
    DeadEnd,
    /// `W0003`: `defer!` in a state with no outgoing edges.
    DeferInDeadEnd,
    /// `W0004`: variant larger than `size_warn` bytes.
    OversizedVariant,
    /// `W0005`: `#[after]` event that no handler handles.
    UnhandledTimerEvent,
    /// `W0010`: typestate `match` arm not associated with an event variant.
    UnassociatedArm,
}

impl WarningCode {
    /// Every code, in numeric order.
    pub const ALL: [Self; 6] = [
        Self::UnreachableState,
        Self::DeadEnd,
        Self::DeferInDeadEnd,
        Self::OversizedVariant,
        Self::UnhandledTimerEvent,
        Self::UnassociatedArm,
    ];

    /// `W0001`, `W0004`, ...
    pub const fn label(self) -> &'static str {
        match self {
            Self::UnreachableState => "W0001",
            Self::DeadEnd => "W0002",
            Self::DeferInDeadEnd => "W0003",
            Self::OversizedVariant => "W0004",
            Self::UnhandledTimerEvent => "W0005",
            Self::UnassociatedArm => "W0010",
        }
    }

    /// The code written as `label`, if it is one.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.label() == label)
    }

    /// All labels, for "known codes" help texts.
    pub fn labels() -> impl Iterator<Item = &'static str> {
        Self::ALL.into_iter().map(Self::label)
    }
}

#[cfg(test)]
mod tests {
    use super::WarningCode;

    #[test]
    fn labels_round_trip() {
        for code in WarningCode::ALL {
            assert_eq!(WarningCode::from_label(code.label()), Some(code));
        }
        assert_eq!(WarningCode::from_label("W0099"), None);
        assert_eq!(WarningCode::from_label("E0001"), None);
    }
}
