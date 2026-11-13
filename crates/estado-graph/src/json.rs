//! JSON export, described by `docs/graph.schema.json`.

use alloc::format;
use alloc::string::String;

use estado_core::{EdgeKind, EdgeTarget, Graph, HistoryKind, StateKind};

/// The value of the `version` property of the JSON document.
pub const JSON_VERSION: u32 = 1;

/// Renders the graph as JSON, in the shape `docs/graph.schema.json` describes.
///
/// The document is pretty-printed with two spaces, ends with a line feed and
/// lists states and edges in the order of the graph, so the same graph
/// always gives the same text.
///
/// The top-level object has `version` ([`JSON_VERSION`]), `machine`, `states`
/// and `edges`. A state has `id`, `name`, `parent`, `kind` (`atomic`,
/// `compound`, `parallel`, `region`, `final`, `choice`), `initial`, `history`
/// (`none`, `shallow`, `deep`), `timers`, `invokes` and `depth`. An edge has
/// `from`, `to`, `kind` (`normal`, `external`, `history`, `error`, `timer`,
/// `always`, `done`) and `event`. `parent`, `initial` and `event` are `null`
/// when the graph has no value; `to` is `{"type": "state", "id": n}`,
/// `{"type": "choice", "id": n}` or `{"type": "dynamic"}`.
///
/// ```
/// use estado_core::{Graph, StateId, StateInfo, StateKind};
///
/// static STATES: [StateInfo; 1] =
///     [StateInfo::new(StateId::new(0), "Off", None, StateKind::Atomic, 0)];
/// static GRAPH: Graph = Graph { machine: "Light", states: &STATES, edges: &[] };
///
/// let json = estado_graph::to_json(&GRAPH);
/// assert!(json.starts_with("{\n  \"version\": 1,\n  \"machine\": \"Light\",\n"));
/// assert!(json.contains("\"kind\": \"atomic\""));
/// ```
pub fn to_json(graph: &Graph) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"version\": {JSON_VERSION},\n"));
    out.push_str(&format!("  \"machine\": {},\n", string(graph.machine)));
    if graph.states.is_empty() {
        out.push_str("  \"states\": [],\n");
    } else {
        out.push_str("  \"states\": [\n");
        for (position, state) in graph.states.iter().enumerate() {
            out.push_str("    {\n");
            out.push_str(&format!("      \"id\": {},\n", state.id.index()));
            out.push_str(&format!("      \"name\": {},\n", string(state.name)));
            out.push_str(&format!(
                "      \"parent\": {},\n",
                optional_id(state.parent.map(|parent| parent.index()))
            ));
            out.push_str(&format!(
                "      \"kind\": \"{}\",\n",
                state_kind(state.kind)
            ));
            out.push_str(&format!(
                "      \"initial\": {},\n",
                optional_id(state.initial.map(|initial| initial.index()))
            ));
            out.push_str(&format!(
                "      \"history\": \"{}\",\n",
                history_kind(state.history)
            ));
            out.push_str(&format!("      \"timers\": {},\n", state.timers));
            out.push_str(&format!("      \"invokes\": {},\n", state.invokes));
            out.push_str(&format!("      \"depth\": {}\n", state.depth));
            out.push_str(if position + 1 == graph.states.len() {
                "    }\n"
            } else {
                "    },\n"
            });
        }
        out.push_str("  ],\n");
    }
    if graph.edges.is_empty() {
        out.push_str("  \"edges\": []\n");
    } else {
        out.push_str("  \"edges\": [\n");
        for (position, edge) in graph.edges.iter().enumerate() {
            out.push_str("    {\n");
            out.push_str(&format!("      \"from\": {},\n", edge.from.index()));
            out.push_str(&format!("      \"to\": {},\n", target(edge.to)));
            out.push_str(&format!("      \"kind\": \"{}\",\n", edge_kind(edge.kind)));
            out.push_str(&format!(
                "      \"event\": {}\n",
                edge.event_hint.map_or_else(|| String::from("null"), string)
            ));
            out.push_str(if position + 1 == graph.edges.len() {
                "    }\n"
            } else {
                "    },\n"
            });
        }
        out.push_str("  ]\n");
    }
    out.push_str("}\n");
    out
}

fn optional_id(id: Option<usize>) -> String {
    id.map_or_else(|| String::from("null"), |id| format!("{id}"))
}

fn target(target: EdgeTarget) -> String {
    match target {
        EdgeTarget::State(state) => format!("{{ \"type\": \"state\", \"id\": {} }}", state.index()),
        EdgeTarget::Choice(state) => {
            format!("{{ \"type\": \"choice\", \"id\": {} }}", state.index())
        }
        EdgeTarget::Dynamic => String::from("{ \"type\": \"dynamic\" }"),
    }
}

fn state_kind(kind: StateKind) -> &'static str {
    match kind {
        StateKind::Atomic => "atomic",
        StateKind::Compound => "compound",
        StateKind::Parallel => "parallel",
        StateKind::Region => "region",
        StateKind::Final => "final",
        StateKind::Choice => "choice",
    }
}

fn history_kind(kind: HistoryKind) -> &'static str {
    match kind {
        HistoryKind::None => "none",
        HistoryKind::Shallow => "shallow",
        HistoryKind::DeepKeep => "deep",
    }
}

fn edge_kind(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Normal => "normal",
        EdgeKind::External => "external",
        EdgeKind::History => "history",
        EdgeKind::Error => "error",
        EdgeKind::Timer => "timer",
        EdgeKind::Always => "always",
        EdgeKind::Done => "done",
    }
}

/// `text` as a JSON string literal.
fn string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                quoted.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}
