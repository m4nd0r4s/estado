//! Edge labels, shared by the Mermaid and DOT exporters.

use alloc::string::String;

use estado_core::{EdgeInfo, EdgeKind};

/// The word a diagram shows for an edge kind, `None` for a plain edge.
pub(crate) fn kind_word(kind: EdgeKind) -> Option<&'static str> {
    match kind {
        EdgeKind::Normal => None,
        EdgeKind::External => Some("external"),
        EdgeKind::History => Some("history"),
        EdgeKind::Error => Some("error"),
        EdgeKind::Timer => Some("timer"),
        EdgeKind::Always => Some("always"),
        EdgeKind::Done => Some("done"),
    }
}

/// The text on an edge: the event hint, then the kind in brackets.
///
/// `Event::Pause`, `Event::Land [timer]`, `[always]`, or empty for a plain
/// edge without a hint. Line breaks and runs of white space become one space.
pub(crate) fn edge_label(edge: &EdgeInfo) -> String {
    let mut label = String::new();
    if let Some(hint) = edge.event_hint {
        for word in hint.split_whitespace() {
            if !label.is_empty() {
                label.push(' ');
            }
            label.push_str(word);
        }
    }
    if let Some(word) = kind_word(edge.kind) {
        if !label.is_empty() {
            label.push(' ');
        }
        label.push('[');
        label.push_str(word);
        label.push(']');
    }
    label
}
