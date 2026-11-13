//! The export formats and the extension trait that renders them.

use alloc::string::String;

use estado_core::Graph;

/// One of the export formats.
///
/// ```
/// use estado_graph::Format;
///
/// assert_eq!(Format::from_extension("mmd"), Some(Format::Mermaid));
/// assert_eq!(Format::Scxml.extension(), "scxml");
/// assert_eq!(Format::from_extension("txt"), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Mermaid `stateDiagram-v2` ([`to_mermaid`](crate::to_mermaid)).
    Mermaid,
    /// Graphviz DOT ([`to_dot`](crate::to_dot)).
    Dot,
    /// W3C SCXML ([`to_scxml`](crate::to_scxml)).
    Scxml,
    /// JSON ([`to_json`](crate::to_json)).
    Json,
}

impl Format {
    /// Every format, in the order of the spec's table.
    pub const ALL: [Format; 4] = [Format::Mermaid, Format::Dot, Format::Scxml, Format::Json];

    /// The format of a file extension, without the dot, ignoring case.
    ///
    /// `mmd` and `mermaid` are Mermaid, `dot` and `gv` are DOT, `scxml` is
    /// SCXML and `json` is JSON.
    pub fn from_extension(extension: &str) -> Option<Format> {
        let matches = |name: &str| extension.eq_ignore_ascii_case(name);
        if matches("mmd") || matches("mermaid") {
            Some(Format::Mermaid)
        } else if matches("dot") || matches("gv") {
            Some(Format::Dot)
        } else if matches("scxml") {
            Some(Format::Scxml)
        } else if matches("json") {
            Some(Format::Json)
        } else {
            None
        }
    }

    /// The conventional file extension: `mmd`, `dot`, `scxml` or `json`.
    pub const fn extension(self) -> &'static str {
        match self {
            Format::Mermaid => "mmd",
            Format::Dot => "dot",
            Format::Scxml => "scxml",
            Format::Json => "json",
        }
    }

    /// Renders `graph` in this format.
    pub fn render(self, graph: &Graph) -> String {
        match self {
            Format::Mermaid => crate::to_mermaid(graph),
            Format::Dot => crate::to_dot(graph),
            Format::Scxml => crate::to_scxml(graph),
            Format::Json => crate::to_json(graph),
        }
    }
}

/// The export functions as methods of [`Graph`], so a machine's graph reads
/// `Player::GRAPH.to_mermaid()`.
///
/// ```
/// use estado_core::{Graph, StateId, StateInfo, StateKind};
/// use estado_graph::GraphExport;
///
/// static STATES: [StateInfo; 1] =
///     [StateInfo::new(StateId::new(0), "Off", None, StateKind::Atomic, 0)];
/// static GRAPH: Graph = Graph { machine: "Light", states: &STATES, edges: &[] };
///
/// assert_eq!(GRAPH.to_mermaid(), estado_graph::to_mermaid(&GRAPH));
/// assert!(GRAPH.to_json().contains("\"machine\": \"Light\""));
/// ```
pub trait GraphExport {
    /// Mermaid `stateDiagram-v2`, see [`to_mermaid`](crate::to_mermaid).
    fn to_mermaid(&self) -> String;
    /// Graphviz DOT, see [`to_dot`](crate::to_dot).
    fn to_dot(&self) -> String;
    /// W3C SCXML, see [`to_scxml`](crate::to_scxml).
    fn to_scxml(&self) -> String;
    /// JSON, see [`to_json`](crate::to_json).
    fn to_json(&self) -> String;
}

impl GraphExport for Graph {
    fn to_mermaid(&self) -> String {
        crate::to_mermaid(self)
    }

    fn to_dot(&self) -> String {
        crate::to_dot(self)
    }

    fn to_scxml(&self) -> String {
        crate::to_scxml(self)
    }

    fn to_json(&self) -> String {
        crate::to_json(self)
    }
}
