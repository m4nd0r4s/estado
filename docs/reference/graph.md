# Graph export

The `estado-graph` crate writes the static graph of a machine as Mermaid, Graphviz DOT, SCXML or JSON, and checks the output against committed files. For a walk-through, see the [guide page](../guide/24-graphs.md).

```toml
[dependencies]
estado-graph = { git = "https://github.com/m4nd0r4s/estado" }
```

## What it exports

| Item | Kind | Purpose |
| --- | --- | --- |
| [`to_mermaid(&Graph)`](#mermaid) | function | A `stateDiagram-v2`. |
| [`to_dot(&Graph)`](#dot) | function | A Graphviz `digraph`. |
| [`to_scxml(&Graph)`](#scxml) | function | A W3C SCXML document. |
| [`to_json(&Graph)`](#json) | function | JSON described by [`docs/graph.schema.json`](../graph.schema.json). |
| [`GraphExport`](#graphexport) | trait | The four functions as methods of `Graph`. |
| [`Format`](#format) | enum | Picks a format by value or by file extension. |
| [`assert_snapshot!`](#assert_snapshot) | macro | Compares an export with a committed file. |
| [`snapshot`](#snapshot-helpers) | module | The engine of `assert_snapshot!`, for other kinds of text. |

Every function takes a `&Graph` and returns a `String`. The output depends only on the graph: states and edges come out in the order of the graph, nothing is sorted by hash or time, and every line ends with `\n`. Calling a function twice gives the same text.

The graph of a machine is `<Root as Spec>::GRAPH` (`&'static Graph`).

## GraphExport

```rust,ignore
pub trait GraphExport {
    fn to_mermaid(&self) -> String;
    fn to_dot(&self) -> String;
    fn to_scxml(&self) -> String;
    fn to_json(&self) -> String;
}
impl GraphExport for Graph { .. }
```

```rust
use estado::prelude::*;
use estado_graph::GraphExport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play,
    Resume,
    Pause,
    Stop,
}

#[estado::machine(event = Event)]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
    }

    #[state]
    #[history]
    pub struct Active {
        #[substates]
        pub mode: Mode,
    }

    #[state]
    pub enum Mode {
        #[initial]
        Playing,
        Paused,
    }

    #[handlers]
    impl Player {
        #[on(Idle)]
        fn on_idle(event: &Event) -> Next<Self> {
            match event {
                Event::Play => goto!(Active),
                Event::Resume => goto!(Active, history),
                _ => unhandled!(),
            }
        }

        #[on(Playing)]
        fn on_playing(event: &Event) -> Next<Self> {
            match event {
                Event::Pause => goto!(Paused),
                _ => bubble!(),
            }
        }

        #[on(Paused)]
        fn on_paused(event: &Event) -> Next<Self> {
            match event {
                Event::Play => goto!(Playing),
                _ => bubble!(),
            }
        }

        #[on(Active)]
        fn on_active(_active: ActiveFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Stop => goto!(Idle),
                _ => unhandled!(),
            }
        }
    }
}

use player::Player;

fn main() {
    let graph = <Player as Spec>::GRAPH;

    assert_eq!(
        graph.to_mermaid(),
        "\
stateDiagram-v2
    %% machine Player
    Idle
    state Active {
        state \"H\" as Active_history
        Playing
        Paused
        [*] --> Playing
        Playing --> Paused : Event::Pause
        Paused --> Playing : Event::Play
    }
    [*] --> Idle
    Idle --> Active : Event::Play
    Idle --> Active_history : Event::Resume [history]
    Active --> Idle : Event::Stop
"
    );

    let dot = graph.to_dot();
    assert!(dot.starts_with("digraph \"Player\" {\n"));
    assert!(dot.contains("subgraph cluster_s1 {"));
    assert!(dot.contains("s0 -> h1 [label=\"Event::Resume [history]\"];"));

    let scxml = graph.to_scxml();
    assert!(scxml.contains("<history id=\"Active.history\" type=\"shallow\">"));
    assert!(scxml.contains(
        "<transition event=\"Event.Resume\" target=\"Active.history\" type=\"external\"/>"
    ));

    let json = graph.to_json();
    assert!(json.contains("\"history\": \"shallow\""));
}
```

## Format

```rust,ignore
pub enum Format { Mermaid, Dot, Scxml, Json }
impl Format {
    pub const ALL: [Format; 4];
    pub fn from_extension(extension: &str) -> Option<Format>;
    pub const fn extension(self) -> &'static str;
    pub fn render(self, graph: &Graph) -> String;
}
```

| Format | `extension()` | Also read by `from_extension` |
| --- | --- | --- |
| `Mermaid` | `mmd` | `mermaid` |
| `Dot` | `dot` | `gv` |
| `Scxml` | `scxml` | |
| `Json` | `json` | |

`from_extension` ignores case and takes the extension without the dot. `render` calls the matching function.

```rust
use estado_graph::Format;

assert_eq!(Format::from_extension("GV"), Some(Format::Dot));
assert_eq!(Format::Json.extension(), "json");
assert_eq!(Format::from_extension("svg"), None);
```

## Mermaid

`to_mermaid` writes a `stateDiagram-v2`, indented with four spaces. The first line after the header is a comment with the machine name.

| Graph | Mermaid |
| --- | --- |
| Initial state of the machine, of a compound state, of a region | `[*] --> State`, inside the block that owns it. |
| Compound state | `state Name { .. }` with its substates, then its internal edges. |
| Parallel state | `state Name { .. }` with the regions side by side, separated by a line with `--`. A `%% region name` comment starts each region. |
| Final state | The state, then `State --> [*]` in the same block. |
| Choice | `state Name <<choice>>`. |
| History | Inside the compound state: `state "H" as Name_history` (shallow) or `state "H*" as Name_history` (deep). Edges of kind `history` end at it. |
| Dynamic target | A node `state "*" as dynamic` at the top level; the edge points at it. |

An edge is written inside the innermost compound state or region that contains both of its ends, strictly above them, and at the top level when there is none. An edge between a state and its own substate is written outside that state.

An edge line is `From --> To : label`, with no label when the label is empty. The label is the event hint, then the kind in brackets unless the kind is `normal`:

| Edge kind | Label suffix |
| --- | --- |
| `normal` | none |
| `external` | `[external]` |
| `history` | `[history]` |
| `error` | `[error]` |
| `timer` | `[timer]` |
| `always` | `[always]` |
| `done` | `[done]` |

`Event::Land [timer]`, `[always]` (no hint), `Event::Resume [history]`. White space in a hint, including line breaks, becomes one space, and `;` becomes `,` because Mermaid ends a statement at it.

State ids are the state names with every character outside `A-Za-z0-9_` replaced by `_`, a leading `s_` for a name that starts with a digit, and `state_<index>` for an empty name. An id that another state already has, or that is a word of the Mermaid grammar (`end`, `state`, `note`, `class`, `default`, `style`, ..., compared ignoring case), gets `_<index>` appended. When the id differs from the name, a `state "Name" as id` line keeps the name as the label, with `"` written as `#quot;`.

## DOT

`to_dot` writes a `digraph` with `compound=true`, the machine name as title, and nodes with rounded boxes.

- Node ids are `s<index>`, where the index is the state id. Names never need to be valid DOT identifiers, because they only appear in quoted labels, escaped for `\`, `"` and line breaks.
- A compound state, a parallel state and a region are each a `subgraph cluster_s<index>` with the state name as its label. A parallel state is dashed and a region dotted.
- An edge that starts or ends at a cluster uses an invisible anchor node `a<index>` inside it, with `ltail`/`lhead` set so Graphviz draws it to the border. `lhead` or `ltail` is left out when the other end is inside the cluster.
- The initial state is pointed at by a small filled circle (`i_root`, or `i<index>` inside a cluster).
- A final state has `peripheries=2`; a choice has `shape=diamond`.
- History is a circle `h<index>` labelled `H` or `H*` inside the cluster; edges of kind `history` end at it.
- An edge to a dynamic target ends at a node labelled `*` and is `style=dashed`.
- Edge labels are the same as in Mermaid.

## SCXML

`to_scxml` writes a document for the [W3C SCXML 1.0](https://www.w3.org/TR/scxml/) schema, indented with two spaces, with no executable content.

| Graph | SCXML |
| --- | --- |
| The machine | `<scxml xmlns=".." version="1.0" name=".." initial="..">` |
| Atomic state | `<state id="..">` |
| Compound state, region | `<state id=".." initial="..">` with its substates |
| Parallel state | `<parallel id="..">` with its regions as `<state>` children |
| Final state | `<final id="..">` |
| History | `<history id="Name.history" type="shallow">` or `type="deep"`, with a default `<transition target="..">` to the initial substate |
| Edge | `<transition event=".." target=".." type="..">` inside the state that declares it |

- **Event.** The event hint with `::` turned into `.`, and the alternatives of an or-pattern as separate tokens: `Event::Move | Event::Stop` is `Event.Move Event.Stop`. An `always` edge has no `event`. A `done` edge has `done.state.<id of its state>`. An edge without a hint has no `event`.
- **Type.** `internal` when the target is a strict descendant of the source (a local transition, [spec 5.2](../spec.md)) and the edge is not `external`; otherwise `external`.
- **Target.** The id of the target state; for a `history` edge, the id of its history. A dynamic target has no `target` attribute, with a comment before the transition.
- **Ids.** The name with every character outside `A-Za-z0-9_-.` replaced by `_`, `_` in front of a name that does not start with a letter or `_`, and `_<index>` appended on a clash.
- **Escaping.** `&`, `<`, `>`, `"`, `'` and line breaks in attribute values are written as XML entities; control characters other than white space become U+FFFD.
- **Plain states.** SCXML has no choice pseudostate, and a `<final>` cannot have transitions. A `#[choice]`, and a final state that has outgoing edges, are written as `<state>` with a comment that says what they are.

Timers, tasks and guards are not part of the document.

## JSON

`to_json` writes an object described by [`docs/graph.schema.json`](../graph.schema.json) (JSON Schema 2020-12), pretty-printed with two spaces, ending with a line feed.

```json
{
  "version": 1,
  "machine": "Player",
  "states": [
    { "id": 0, "name": "Idle", "parent": null, "kind": "atomic", "initial": null, "history": "none", "timers": 0, "invokes": 0, "depth": 0 }
  ],
  "edges": [
    { "from": 0, "to": { "type": "state", "id": 1 }, "kind": "normal", "event": "Event::Play" }
  ]
}
```

(The real output has one property per line.)

| Property | Values |
| --- | --- |
| `version` | `1`. Also `estado_graph::JSON_VERSION`. |
| `states[]` | Same order as `Graph::states`, so `states[n].id == n`. |
| `states[].kind` | `atomic`, `compound`, `parallel`, `region`, `final`, `choice` |
| `states[].history` | `none`, `shallow`, `deep` |
| `states[].parent`, `.initial` | A state id, or `null` |
| `edges[].kind` | `normal`, `external`, `history`, `error`, `timer`, `always`, `done` |
| `edges[].to` | `{"type": "state", "id": n}`, `{"type": "choice", "id": n}` or `{"type": "dynamic"}` |
| `edges[].event` | The event pattern as text, or `null` |

The JSON is a faithful copy of the `Graph`: it holds no data the graph does not have, in particular no initial state of the machine (see [limits](#limits)). The tests of the crate validate the output of every example against the schema, so the schema and the exporter cannot drift apart.

## assert_snapshot

```rust,ignore
estado_graph::assert_snapshot!(graph, "docs/player.mmd");
estado_graph::assert_snapshot!(graph, "docs/player.txt", estado_graph::Format::Dot);
```

| Argument | Meaning |
| --- | --- |
| `graph` | A `&Graph` or a `Graph`: `<Root as Spec>::GRAPH`, or a `static`. |
| `path` | The snapshot file, relative to the manifest directory of the crate that calls the macro. |
| `format` | Optional. Without it, the format comes from the extension of `path` (`mmd`, `mermaid`, `dot`, `gv`, `scxml`, `json`); another extension panics. |

The macro renders the graph and compares it with the file. Line endings do not count (`\r\n` equals `\n`).

| Situation | Result |
| --- | --- |
| The file holds the text | passes |
| The file is missing | panics: `snapshot .. does not exist; run the test with ESTADO_BLESS=1 to create it` |
| The file differs | panics with the first line that differs, from the file and from the export |
| `ESTADO_BLESS=1` and the file is missing or differs | writes the file, creating its directories, and passes |

`ESTADO_BLESS` counts as set when it is not empty, `0` or `false`. Blessing rewrites every snapshot a test run reaches, so review the diff of the files before you commit them.

A doctest cannot use a committed file of its own, so this page shows the macro without running it. The tests of the repository do: `examples/tests/graph_snapshots.rs` checks the four reference scenarios against `docs/graphs/`, one file per format.

## Snapshot helpers

The module `estado_graph::snapshot` holds what the macro is built on, so you can snapshot other text the same way.

```rust,ignore
pub fn assert_text_snapshot(path: impl AsRef<Path>, actual: &str);
pub fn check_snapshot(path: &Path, actual: &str, bless: bool)
    -> Result<SnapshotOutcome, SnapshotError>;
pub fn bless_requested() -> bool;
pub const BLESS_VARIABLE: &str; // "ESTADO_BLESS"
```

`assert_text_snapshot` panics like the macro and reads `ESTADO_BLESS`. `check_snapshot` does the work without the environment and without panicking: it returns `SnapshotOutcome::Matches` or `Written`, or a `SnapshotError` (`Missing`, `Differs { path, line, expected, actual }`, `Io`).

```rust
use estado_graph::snapshot::{SnapshotError, SnapshotOutcome, check_snapshot};

let path = std::env::temp_dir().join("estado-graph-reference-example.txt");
let _ = std::fs::remove_file(&path);

// A missing file fails, unless blessing.
assert!(matches!(check_snapshot(&path, "one\n", false), Err(SnapshotError::Missing(_))));
assert_eq!(check_snapshot(&path, "one\n", true).unwrap(), SnapshotOutcome::Written);
assert_eq!(check_snapshot(&path, "one\n", false).unwrap(), SnapshotOutcome::Matches);

// A different text names the first line that differs.
match check_snapshot(&path, "two\n", false) {
    Err(SnapshotError::Differs { line, expected, actual, .. }) => {
        assert_eq!((line, expected.as_str(), actual.as_str()), (1, "one", "two"));
    }
    other => panic!("unexpected {other:?}"),
}
std::fs::remove_file(&path).unwrap();
```

## Limits

- **The initial state of the machine is a guess.** `Graph` records the `#[initial]` of every compound state and region, but not the one of the root enum (spec 8.1 has no field for it). The exports use the first top-level state in declaration order, which is the initial state whenever `#[initial]` is on the first variant, as in every example. A root enum whose `#[initial]` is not first gets a wrong `[*]` arrow in Mermaid and DOT and a wrong `initial` in SCXML until the graph records it ([errata 19](../spec-errata.md)).
- **Events are text.** The label comes from the pattern of the `match` arm (`event_hint`). A handler without a `match`, or an arm with a wildcard or a binding, has no hint, and its edge has no label.
- **Mermaid has no history pseudostate and no names for regions.** History is drawn as a labelled state and regions as comments, as described above. Mermaid versions differ in what they render for edges that cross compound states; the DOT output gives exact control over clusters.
- **Choice and dynamic targets** (`#[choice]`, `goto_dyn!`) are reserved: the macro does not generate them yet ([E0090](../errors/E0090.md)). The exporters handle them for graphs built by hand.
- **Features.** The crate is `no_std` with `alloc`. The default `std` feature adds `assert_snapshot!` and the [`snapshot`](#snapshot-helpers) module.
