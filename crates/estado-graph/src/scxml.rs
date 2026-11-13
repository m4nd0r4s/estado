//! W3C SCXML export.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, HistoryKind, StateKind};

use crate::out::Lines;
use crate::tree::Tree;

/// Renders the graph as an SCXML document (W3C Recommendation, 1.0).
///
/// - The document element is `<scxml>` with the machine name and the
///   `initial` state; compound states are `<state initial="..">`, parallel
///   states `<parallel>` with their regions as `<state>` children, final
///   states `<final>`, and history is `<history type="shallow|deep">` with a
///   default `<transition>` to the initial substate.
/// - An edge is a `<transition>` inside the state that declares it. The
///   `event` is the event hint with `::` turned into `.` (an SCXML event
///   descriptor) and the alternatives of an or-pattern as separate tokens, so
///   `Event::Move | Event::Stop` is `Event.Move Event.Stop`. `#[always]`
///   edges have no event, and `#[on_done]` edges listen to
///   `done.state.<state id>`.
/// - `type` is `internal` when the target is a strict descendant of the
///   source (a local transition, spec 5.2) and `external` otherwise or when
///   the edge is `goto!(.., external)`.
/// - There is no executable content: timers, tasks and guards are not part of
///   the document. A `#[choice]` and a final state with outgoing edges (SCXML
///   final states have no transitions) are plain `<state>` elements, with a
///   comment that says so.
/// - A state id is the name with every character outside `A-Za-z0-9_-.`
///   replaced by `_`; clashes get the state index appended.
///
/// ```
/// use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind};
///
/// static STATES: [StateInfo; 2] = [
///     StateInfo::new(StateId::new(0), "Off", None, StateKind::Atomic, 0),
///     StateInfo::new(StateId::new(1), "On", None, StateKind::Atomic, 0),
/// ];
/// static EDGES: [EdgeInfo; 1] = [EdgeInfo {
///     from: StateId::new(0),
///     to: EdgeTarget::State(StateId::new(1)),
///     kind: EdgeKind::Normal,
///     event_hint: Some("Event::Toggle"),
/// }];
/// static GRAPH: Graph = Graph { machine: "Light", states: &STATES, edges: &EDGES };
///
/// let document = estado_graph::to_scxml(&GRAPH);
/// assert!(document.contains(
///     "<transition event=\"Event.Toggle\" target=\"On\" type=\"external\"/>"
/// ));
/// ```
pub fn to_scxml(graph: &Graph) -> String {
    let tree = Tree::new(graph);
    let ids = state_ids(graph);
    let mut renderer = Renderer {
        tree: &tree,
        ids: &ids,
        lines: Lines::new("  "),
    };
    renderer
        .lines
        .line("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let mut root = format!(
        "<scxml xmlns=\"http://www.w3.org/2005/07/scxml\" version=\"1.0\" name=\"{}\"",
        escape(graph.machine)
    );
    if let Some(initial) = tree.machine_initial() {
        root.push_str(&format!(" initial=\"{}\"", ids[initial]));
    }
    root.push('>');
    renderer.lines.line(&root);
    renderer.lines.open();
    for &state in tree.roots() {
        renderer.state(state);
    }
    renderer.lines.close();
    renderer.lines.line("</scxml>");
    renderer.lines.finish()
}

/// `text` as XML attribute content.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\n' => escaped.push_str("&#10;"),
            '\r' => escaped.push_str("&#13;"),
            '\t' => escaped.push_str("&#9;"),
            control if (control as u32) < 0x20 => escaped.push('\u{FFFD}'),
            other => escaped.push(other),
        }
    }
    escaped
}

/// One valid, unique XML id per state.
fn state_ids(graph: &Graph) -> Vec<String> {
    let mut used = BTreeSet::new();
    let mut ids = Vec::new();
    for (index, state) in graph.states.iter().enumerate() {
        let mut id: String = state
            .name
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        if !id.starts_with(|character: char| character.is_ascii_alphabetic() || character == '_') {
            id.insert(0, '_');
        }
        if used.contains(&id) {
            id = format!("{id}_{index}");
        }
        while used.contains(&id) {
            id.push('_');
        }
        used.insert(id.clone());
        ids.push(id);
    }
    ids
}

/// The SCXML event descriptor of an event hint.
fn event_descriptor(hint: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();
    for word in hint.split_whitespace() {
        if word != "|" {
            tokens.push(word.replace("::", "."));
        }
    }
    tokens.join(" ")
}

struct Renderer<'graph> {
    tree: &'graph Tree<'graph>,
    ids: &'graph [String],
    lines: Lines,
}

impl Renderer<'_> {
    fn state(&mut self, state: usize) {
        let graph = self.tree.graph;
        let info = &graph.states[state];
        let id = &self.ids[state];
        let outgoing: Vec<&EdgeInfo> = graph
            .edges
            .iter()
            .filter(|edge| edge.from.index() == state)
            .collect();
        let children = self.tree.children(state);
        let history = (info.history != HistoryKind::None).then_some(info.history);

        let element = match info.kind {
            StateKind::Parallel => "parallel",
            StateKind::Final if outgoing.is_empty() => "final",
            _ => "state",
        };
        match info.kind {
            StateKind::Choice => self.lines.line("<!-- choice pseudostate -->"),
            StateKind::Final if !outgoing.is_empty() => {
                self.lines
                    .line("<!-- final state; it has transitions, so it is a state here -->");
            }
            _ => {}
        }
        let mut open = format!("<{element} id=\"{id}\"");
        if let Some(initial) = info.initial {
            open.push_str(&format!(" initial=\"{}\"", self.ids[initial.index()]));
        }
        let is_empty = children.is_empty() && outgoing.is_empty() && history.is_none();
        if is_empty {
            open.push_str("/>");
            self.lines.line(&open);
            return;
        }
        open.push('>');
        self.lines.line(&open);
        self.lines.open();
        if let (Some(kind), Some(initial)) = (history, info.initial) {
            let kind = if kind == HistoryKind::DeepKeep {
                "deep"
            } else {
                "shallow"
            };
            self.lines.line_of(&[
                "<history id=\"",
                &self.history_id(state),
                "\" type=\"",
                kind,
                "\">",
            ]);
            self.lines.open();
            self.lines
                .line_of(&["<transition target=\"", &self.ids[initial.index()], "\"/>"]);
            self.lines.close();
            self.lines.line("</history>");
        }
        for &child in children {
            self.state(child);
        }
        for edge in outgoing {
            self.transition(edge, state);
        }
        self.lines.close();
        self.lines.line_of(&["</", element, ">"]);
    }

    /// The id of the history pseudostate of a compound state.
    fn history_id(&self, state: usize) -> String {
        let mut id = format!("{}.history", self.ids[state]);
        // `.history` cannot clash with a name that is not already taken, but
        // two states can differ only by this suffix.
        while self.ids.contains(&id) {
            id.push('_');
        }
        id
    }

    fn transition(&mut self, edge: &EdgeInfo, from: usize) {
        let graph = self.tree.graph;
        let mut element = String::from("<transition");
        match (edge.kind, edge.event_hint) {
            (EdgeKind::Always, _) => {}
            (EdgeKind::Done, _) => {
                element.push_str(&format!(" event=\"done.state.{}\"", self.ids[from]));
            }
            (_, Some(hint)) => {
                let descriptor = event_descriptor(hint);
                if !descriptor.is_empty() {
                    element.push_str(&format!(" event=\"{}\"", escape(&descriptor)));
                }
            }
            (_, None) => {}
        }
        let target = match edge.to {
            EdgeTarget::State(target) | EdgeTarget::Choice(target) => {
                let target = target.index();
                let id = if edge.kind == EdgeKind::History
                    && graph.states[target].history != HistoryKind::None
                {
                    self.history_id(target)
                } else {
                    self.ids[target].clone()
                };
                element.push_str(&format!(" target=\"{id}\""));
                Some(target)
            }
            EdgeTarget::Dynamic => {
                self.lines
                    .line("<!-- dynamic target: decided at runtime -->");
                None
            }
        };
        let local = edge.kind != EdgeKind::External
            && target.is_some_and(|target| self.tree.is_strict_ancestor(from, target));
        element.push_str(if local {
            " type=\"internal\"/>"
        } else {
            " type=\"external\"/>"
        });
        self.lines.line(&element);
    }
}
