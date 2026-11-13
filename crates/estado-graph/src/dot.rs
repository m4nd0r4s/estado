//! Graphviz DOT export.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, HistoryKind, StateKind};

use crate::label::edge_label;
use crate::out::Lines;
use crate::tree::Tree;

/// Renders the graph as a Graphviz `digraph`.
///
/// - Compound states, parallel states and regions are `subgraph cluster_*`
///   blocks; parallel states are dashed and regions dotted. An edge that
///   starts or ends at a cluster uses an invisible anchor node inside it with
///   `ltail`/`lhead`, so it is drawn to the cluster border.
/// - A filled circle points at the initial state of the machine, of each
///   compound state and of each region. A final state has a double border, a
///   `#[choice]` is a diamond, and a compound state with history holds a
///   circle labelled `H` (shallow) or `H*` (deep) that `history` edges end at.
/// - An edge to a dynamic target (`goto_dyn!`) is dashed and ends at a node
///   labelled `*`.
/// - Node ids are `s<index>`, so names never need to be valid identifiers;
///   every name and label is written as a quoted string with `\`, `"` and
///   line breaks escaped.
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
/// let dot = estado_graph::to_dot(&GRAPH);
/// assert!(dot.starts_with("digraph \"Light\" {\n"));
/// assert!(dot.contains("    s0 -> s1 [label=\"Event::Toggle\"];\n"));
/// ```
pub fn to_dot(graph: &Graph) -> String {
    let tree = Tree::new(graph);
    let anchored = anchored_clusters(&tree);
    let mut renderer = Renderer {
        tree: &tree,
        anchored,
        lines: Lines::new("    "),
    };
    renderer
        .lines
        .line_of(&["digraph ", &quote(graph.machine), " {"]);
    renderer.lines.open();
    renderer.lines.line("compound=true;");
    renderer
        .lines
        .line_of(&["label=", &quote(graph.machine), ";"]);
    renderer.lines.line("labelloc=t;");
    renderer.lines.line("node [shape=box, style=rounded];");
    renderer.contents(None);
    for edge in graph.edges {
        renderer.edge(edge);
    }
    renderer.lines.close();
    renderer.lines.line("}");
    renderer.lines.finish()
}

/// `text` as a DOT quoted string.
fn quote(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => {}
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

fn is_cluster(kind: StateKind) -> bool {
    matches!(
        kind,
        StateKind::Compound | StateKind::Parallel | StateKind::Region
    )
}

/// The state an edge ends at, unless it ends at a history pseudostate or a
/// dynamic target (which are nodes, not clusters).
fn cluster_target(tree: &Tree<'_>, edge: &EdgeInfo) -> Option<usize> {
    match edge.to {
        EdgeTarget::State(target) | EdgeTarget::Choice(target) => {
            let target = target.index();
            let at_history = edge.kind == EdgeKind::History
                && tree.graph.states[target].history != HistoryKind::None;
            (is_cluster(tree.kind(target)) && !at_history).then_some(target)
        }
        EdgeTarget::Dynamic => None,
    }
}

/// Which states need an anchor node: the clusters an edge starts or ends at.
fn anchored_clusters(tree: &Tree<'_>) -> Vec<bool> {
    let mut anchored = alloc::vec![false; tree.graph.states.len()];
    for edge in tree.graph.edges {
        let from = edge.from.index();
        if is_cluster(tree.kind(from)) {
            anchored[from] = true;
        }
        if let Some(target) = cluster_target(tree, edge) {
            anchored[target] = true;
        }
    }
    anchored
}

struct Renderer<'graph> {
    tree: &'graph Tree<'graph>,
    anchored: Vec<bool>,
    lines: Lines,
}

impl Renderer<'_> {
    /// The nodes and clusters at one level, then its initial arrow.
    fn contents(&mut self, scope: Option<usize>) {
        let graph = self.tree.graph;
        let (children, initial, initial_node) = match scope {
            None => (
                self.tree.roots().to_vec(),
                self.tree.machine_initial(),
                String::from("i_root"),
            ),
            Some(state) => (
                self.tree.children(state).to_vec(),
                graph.states[state].initial.map(|initial| initial.index()),
                format!("i{state}"),
            ),
        };
        if let Some(state) = scope {
            if graph.states[state].history != HistoryKind::None {
                let label = if graph.states[state].history == HistoryKind::DeepKeep {
                    "H*"
                } else {
                    "H"
                };
                self.lines.line_of(&[
                    "h",
                    &format!("{state}"),
                    " [shape=circle, label=",
                    &quote(label),
                    "];",
                ]);
            }
            if self.anchored[state] {
                self.lines.line_of(&[
                    "a",
                    &format!("{state}"),
                    " [shape=point, style=invis, width=0, label=\"\"];",
                ]);
            }
        } else if graph
            .edges
            .iter()
            .any(|edge| edge.to == EdgeTarget::Dynamic)
        {
            self.lines.line("dynamic [shape=plaintext, label=\"*\"];");
        }
        if let Some(initial) = initial {
            self.lines.line_of(&[
                &initial_node,
                " [shape=circle, style=filled, fillcolor=black, label=\"\", width=0.2, fixedsize=true];",
            ]);
            self.lines
                .line_of(&[&initial_node, " -> s", &format!("{initial}"), ";"]);
        }
        for &child in &children {
            self.declare(child);
        }
    }

    fn declare(&mut self, state: usize) {
        let info = &self.tree.graph.states[state];
        let name = quote(info.name);
        match info.kind {
            StateKind::Atomic => {
                self.lines
                    .line_of(&["s", &format!("{state}"), " [label=", &name, "];"]);
            }
            StateKind::Final => {
                self.lines.line_of(&[
                    "s",
                    &format!("{state}"),
                    " [label=",
                    &name,
                    ", peripheries=2];",
                ]);
            }
            StateKind::Choice => {
                self.lines.line_of(&[
                    "s",
                    &format!("{state}"),
                    " [label=",
                    &name,
                    ", shape=diamond];",
                ]);
            }
            StateKind::Compound | StateKind::Parallel | StateKind::Region => {
                self.lines
                    .line_of(&["subgraph cluster_s", &format!("{state}"), " {"]);
                self.lines.open();
                self.lines.line_of(&["label=", &name, ";"]);
                match info.kind {
                    StateKind::Parallel => self.lines.line("style=dashed;"),
                    StateKind::Region => self.lines.line("style=dotted;"),
                    _ => {}
                }
                self.contents(Some(state));
                self.lines.close();
                self.lines.line("}");
            }
        }
    }

    fn edge(&mut self, edge: &EdgeInfo) {
        let from = edge.from.index();
        let mut attributes: Vec<String> = Vec::new();
        let label = edge_label(edge);
        if !label.is_empty() {
            attributes.push(format!("label={}", quote(&label)));
        }
        let cluster_head = cluster_target(self.tree, edge);
        let head = match edge.to {
            EdgeTarget::State(target) | EdgeTarget::Choice(target) => {
                let target = target.index();
                if cluster_head.is_none() && is_cluster(self.tree.kind(target)) {
                    format!("h{target}")
                } else {
                    self.endpoint(target)
                }
            }
            EdgeTarget::Dynamic => {
                attributes.push(String::from("style=dashed"));
                String::from("dynamic")
            }
        };
        // Graphviz rejects a cluster edge whose other end is inside that
        // cluster, and a loop on a cluster has nothing to point at.
        let inside_tail = match edge.to {
            EdgeTarget::State(target) | EdgeTarget::Choice(target) => {
                target.index() == from || self.tree.is_strict_ancestor(from, target.index())
            }
            EdgeTarget::Dynamic => false,
        };
        if is_cluster(self.tree.kind(from)) && !inside_tail {
            attributes.push(format!("ltail=cluster_s{from}"));
        }
        if let Some(cluster) = cluster_head {
            if cluster != from && !self.tree.is_strict_ancestor(cluster, from) {
                attributes.push(format!("lhead=cluster_s{cluster}"));
            }
        }
        let tail = self.endpoint(from);
        if attributes.is_empty() {
            self.lines.line_of(&[&tail, " -> ", &head, ";"]);
        } else {
            let attributes = attributes.join(", ");
            self.lines
                .line_of(&[&tail, " -> ", &head, " [", &attributes, "];"]);
        }
    }

    /// The node an edge attaches to: the state, or the anchor of a cluster.
    fn endpoint(&self, state: usize) -> String {
        if is_cluster(self.tree.kind(state)) {
            format!("a{state}")
        } else {
            format!("s{state}")
        }
    }
}
