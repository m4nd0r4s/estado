//! The event enum of the synthetic machine: 50 variants, unit, tuple and
//! struct-like, as a real protocol would have.

/// Shape of an event variant.
pub enum EventShape {
    /// `Name`.
    Unit,
    /// `Name(ty)`.
    Tuple(&'static str),
    /// `Name { field: ty, .. }`.
    Struct(&'static [(&'static str, &'static str)]),
}

/// One event variant.
pub struct EventSpec {
    pub name: &'static str,
    pub shape: EventShape,
}

use EventShape::{Struct, Tuple, Unit};

const fn unit(name: &'static str) -> EventSpec {
    EventSpec { name, shape: Unit }
}

const fn tuple(name: &'static str, ty: &'static str) -> EventSpec {
    EventSpec {
        name,
        shape: Tuple(ty),
    }
}

/// The 50 variants.
pub const EVENTS: [EventSpec; 50] = [
    unit("Start"),
    unit("Stop"),
    unit("Pause"),
    unit("Resume"),
    unit("Reset"),
    unit("Retry"),
    unit("Timeout"),
    unit("Deadline"),
    unit("Cancel"),
    unit("Confirm"),
    unit("Reject"),
    unit("Open"),
    unit("Close"),
    unit("Connect"),
    unit("Disconnect"),
    unit("Authenticate"),
    unit("Refresh"),
    unit("Flush"),
    unit("Commit"),
    unit("Rollback"),
    unit("Suspend"),
    unit("Wake"),
    unit("Sleep"),
    unit("Calibrate"),
    unit("Arm"),
    unit("Disarm"),
    unit("Fire"),
    unit("Hold"),
    unit("Release"),
    unit("Lock"),
    unit("Unlock"),
    unit("Enable"),
    unit("Disable"),
    unit("Escalate"),
    unit("Recover"),
    unit("Shutdown"),
    unit("Boot"),
    unit("Probe"),
    tuple("Load", "u32"),
    tuple("Unload", "u32"),
    tuple("Tick", "u32"),
    tuple("Fault", "u16"),
    tuple("Warn", "u16"),
    tuple("Select", "u8"),
    tuple("Deselect", "u8"),
    EventSpec {
        name: "Move",
        shape: Struct(&[("x", "i16"), ("y", "i16")]),
    },
    EventSpec {
        name: "Resize",
        shape: Struct(&[("width", "u16"), ("height", "u16")]),
    },
    EventSpec {
        name: "Sample",
        shape: Struct(&[("channel", "u8"), ("value", "i32")]),
    },
    EventSpec {
        name: "Link",
        shape: Struct(&[("peer", "u16")]),
    },
    EventSpec {
        name: "Mode",
        shape: Tuple("u8"),
    },
];

/// Index of the variant called `name`.
pub fn event_index(name: &str) -> usize {
    EVENTS
        .iter()
        .position(|event| event.name == name)
        .unwrap_or_else(|| panic!("no event called {name}"))
}

/// The `pub enum Event` declaration.
pub fn render_event_enum() -> String {
    let mut text = String::from("#[derive(Debug, Clone, PartialEq, Eq)]\npub enum Event {\n");
    for event in &EVENTS {
        match &event.shape {
            Unit => text.push_str(&format!("    {},\n", event.name)),
            Tuple(ty) => text.push_str(&format!("    {}({ty}),\n", event.name)),
            Struct(fields) => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|(name, ty)| format!("{name}: {ty}"))
                    .collect();
                text.push_str(&format!(
                    "    {} {{ {} }},\n",
                    event.name,
                    fields.join(", ")
                ));
            }
        }
    }
    text.push_str("}\n");
    text
}

/// The pattern of a `match` arm for an event; a tuple event sometimes gets a
/// guard, `variety` picks which.
pub fn render_pattern(event: usize, variety: usize) -> String {
    let spec = &EVENTS[event];
    match &spec.shape {
        Unit => format!("Event::{}", spec.name),
        Tuple(_) if variety % 2 == 0 => {
            format!("Event::{}(amount) if *amount > {}", spec.name, variety % 7)
        }
        Tuple(_) => format!("Event::{}(_)", spec.name),
        Struct(_) => format!("Event::{} {{ .. }}", spec.name),
    }
}
