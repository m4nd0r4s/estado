//! Mermaid `stateDiagram-v2` export.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, HistoryKind, StateKind};

use crate::label::edge_label;
use crate::out::Lines;
use crate::tree::Tree;

/// Words the Mermaid state-diagram grammar reads as keywords. A state with
/// one of these names gets a different id and keeps its name as the label.
const RESERVED: [&str; 22] = [
    "accdescr",
    "acctitle",
    "as",
    "callback",
    "choice",
    "class",
    "classdef",
    "click",
    "cssclass",
    "default",
    "direction",
    "end",
    "fork",
    "hide",
    "join",
    "link",
    "note",
    "scale",
    "state",
    "statediagram",
    "style",
    "title",
];

/// Renders the graph as a Mermaid `stateDiagram-v2`.
///
/// - Compound states are `state Name { .. }` blocks, parallel states hold
///   their regions separated by `--`, and each block starts with a `[*]`
///   arrow to its initial state.
/// - A final state has an arrow to `[*]`, a `#[choice]` is `<<choice>>`, and a
///   compound state with history holds a pseudostate labelled `H` (shallow)
///   or `H*` (deep) that `history` edges point to.
/// - An edge is labelled with its event hint and, unless it is a plain
///   transition, its kind in brackets: `Event::Land [timer]`.
/// - A state id is the state name with every character outside `A-Za-z0-9_`
///   replaced by `_`. An id that clashes with another, or with a Mermaid
///   keyword, gets the state index appended, and the name stays as its label.
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
/// let diagram = estado_graph::to_mermaid(&GRAPH);
/// assert!(diagram.starts_with("stateDiagram-v2\n"));
/// assert!(diagram.contains("    Off --> On : Event::Toggle\n"));
/// ```
pub fn to_mermaid(graph: &Graph) -> String {
    let tree = Tree::new(graph);
    let names = Names::new(&tree);
    let mut renderer = Renderer {
        tree: &tree,
        names: &names,
        lines: Lines::new("    "),
    };
    renderer.lines.line("stateDiagram-v2");
    renderer.lines.open();
    renderer
        .lines
        .line_of(&["%% machine ", &single_line(graph.machine)]);
    renderer.scope(None);
    renderer.lines.close();
    renderer.lines.finish()
}

/// The ids of the diagram nodes.
struct Names {
    states: Vec<String>,
    histories: Vec<Option<String>>,
    dynamic: Option<String>,
}

impl Names {
    fn new(tree: &Tree<'_>) -> Self {
        let graph = tree.graph;
        let mut used = BTreeSet::new();
        let mut states = Vec::new();
        for (index, state) in graph.states.iter().enumerate() {
            let base = identifier(state.name, index);
            let mut id = base.clone();
            if is_reserved(&id) || used.contains(&id) {
                id = format!("{base}_{index}");
            }
            while used.contains(&id) {
                id.push('_');
            }
            used.insert(id.clone());
            states.push(id);
        }
        let mut histories = Vec::new();
        for (index, state) in graph.states.iter().enumerate() {
            histories.push((state.history != HistoryKind::None).then(|| {
                let mut id = format!("{}_history", states[index]);
                while used.contains(&id) {
                    id.push('_');
                }
                used.insert(id.clone());
                id
            }));
        }
        let dynamic = graph
            .edges
            .iter()
            .any(|edge| edge.to == EdgeTarget::Dynamic)
            .then(|| {
                let mut id = String::from("dynamic");
                while used.contains(&id) {
                    id.push('_');
                }
                id
            });
        Self {
            states,
            histories,
            dynamic,
        }
    }

    /// The node an edge points at.
    fn target(&self, edge: &EdgeInfo) -> &str {
        match edge.to {
            EdgeTarget::State(state) if edge.kind == EdgeKind::History => {
                match &self.histories[state.index()] {
                    Some(history) => history,
                    None => &self.states[state.index()],
                }
            }
            EdgeTarget::State(state) | EdgeTarget::Choice(state) => &self.states[state.index()],
            EdgeTarget::Dynamic => self
                .dynamic
                .as_deref()
                .expect("a dynamic edge allocates the dynamic node"),
        }
    }
}

fn is_reserved(id: &str) -> bool {
    RESERVED.iter().any(|word| word.eq_ignore_ascii_case(id))
}

/// `name` with every character outside `A-Za-z0-9_` replaced by `_`.
fn identifier(name: &str, index: usize) -> String {
    let mut id: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if id.is_empty() {
        id = format!("state_{index}");
    } else if id.starts_with(|character: char| character.is_ascii_digit()) {
        id.insert_str(0, "s_");
    }
    id
}

/// A text for a comment: one line.
fn single_line(text: &str) -> String {
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    line
}

/// A text for a quoted label: one line, double quotes as the `#quot;` entity.
fn quoted_text(text: &str) -> String {
    single_line(text).replace('"', "#quot;")
}

struct Renderer<'graph> {
    tree: &'graph Tree<'graph>,
    names: &'graph Names,
    lines: Lines,
}

impl Renderer<'_> {
    /// The body of the diagram, of a compound state or of a region.
    fn scope(&mut self, scope: Option<usize>) {
        let graph = self.tree.graph;
        let (children, initial): (Vec<usize>, Option<usize>) = match scope {
            None => (self.tree.roots().to_vec(), self.tree.machine_initial()),
            Some(state) => (
                self.tree.children(state).to_vec(),
                graph.states[state].initial.map(|initial| initial.index()),
            ),
        };
        if let Some(state) = scope {
            if let Some(history) = &self.names.histories[state] {
                let label = match graph.states[state].history {
                    HistoryKind::DeepKeep => "H*",
                    _ => "H",
                };
                self.lines.line_of(&["state \"", label, "\" as ", history]);
            }
        } else if let Some(dynamic) = &self.names.dynamic {
            self.lines.line_of(&["state \"*\" as ", dynamic]);
        }
        for &child in &children {
            self.declare(child);
        }
        if let Some(initial) = initial {
            self.lines
                .line_of(&["[*] --> ", &self.names.states[initial]]);
        }
        for &child in &children {
            if self.tree.kind(child) == StateKind::Final {
                self.lines.line_of(&[&self.names.states[child], " --> [*]"]);
            }
        }
        for edge in graph.edges {
            if self.tree.edge_scope(edge) == scope {
                self.edge(edge);
            }
        }
    }

    fn declare(&mut self, state: usize) {
        let info = &self.tree.graph.states[state];
        let id = &self.names.states[state];
        let renamed = id != info.name;
        match info.kind {
            StateKind::Atomic | StateKind::Final => {
                if renamed {
                    self.alias(state);
                } else {
                    self.lines.line(id);
                }
            }
            StateKind::Choice => {
                if renamed {
                    self.alias(state);
                }
                self.lines.line_of(&["state ", id, " <<choice>>"]);
            }
            StateKind::Compound | StateKind::Region => {
                if renamed {
                    self.alias(state);
                }
                self.lines.line_of(&["state ", id, " {"]);
                self.lines.open();
                self.scope(Some(state));
                self.lines.close();
                self.lines.line("}");
            }
            StateKind::Parallel => {
                if renamed {
                    self.alias(state);
                }
                self.lines.line_of(&["state ", id, " {"]);
                self.lines.open();
                for (position, &region) in self.tree.children(state).iter().enumerate() {
                    if position > 0 {
                        self.lines.line("--");
                    }
                    let region_name = single_line(self.tree.graph.states[region].name);
                    self.lines.line_of(&["%% region ", &region_name]);
                    self.scope(Some(region));
                }
                self.lines.close();
                self.lines.line("}");
            }
        }
    }

    fn alias(&mut self, state: usize) {
        let name = quoted_text(self.tree.graph.states[state].name);
        self.lines
            .line_of(&["state \"", &name, "\" as ", &self.names.states[state]]);
    }

    fn edge(&mut self, edge: &EdgeInfo) {
        let from = &self.names.states[edge.from.index()];
        let target = self.names.target(edge);
        let label = edge_label(edge).replace(';', ",");
        if label.is_empty() {
            self.lines.line_of(&[from, " --> ", target]);
        } else {
            self.lines.line_of(&[from, " --> ", target, " : ", &label]);
        }
    }
}
