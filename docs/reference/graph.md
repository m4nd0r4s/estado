# Graph export

The `estado-graph` crate writes the static graph of a machine as Mermaid, Graphviz DOT, SCXML or JSON, checks the output against committed files, and analyses the graph. For a walk-through, see the [guide page](../guide/24-graphs.md).

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
| [`GraphAnalysis`](#analyses) | trait | `reachable`, `dead_ends`, `edge_cover` and `shortest_path` as methods of `Graph`. |
| [`Path`](#path) | struct | A walk through the edges, from `shortest_path` and `edge_cover`. |
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
        Playing --> Paused : Event#58;#58;Pause
        Paused --> Playing : Event#58;#58;Play
    }
    [*] --> Idle
    Idle --> Active : Event#58;#58;Play
    Idle --> Active_history : Event#58;#58;Resume [history]
    Active --> Idle : Event#58;#58;Stop
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

`Event::Land [timer]`, `[always]` (no hint), `Event::Resume [history]`. White space in a hint, including line breaks, becomes one space.

Mermaid reads several characters as syntax, even in a label (a second `:` in a transition line is a parse error), so every label and every state name in a `state "Name" as id` line is written with Mermaid entities, which Mermaid draws as the character:

| Character | Written as |
| --- | --- |
| `:` | `#58;` |
| `;` | `#59;` |
| `#` | `#35;` |
| `"` | `#quot;` |
| `<` `>` `&` | `#lt;` `#gt;` `#amp;` |
| `\` | `#92;` |

So `Event::Pause` is written `Event#58;#58;Pause` in the source and reads `Event::Pause` in the picture.

State ids are the state names with every character outside `A-Za-z0-9_` replaced by `_`, a leading `s_` for a name that starts with a digit, and `state_<index>` for an empty name. An id that another state already has, or that is a word of the Mermaid grammar (`end`, `state`, `note`, `class`, `default`, `style`, ..., compared ignoring case), gets `_<index>` appended, so two states with the same name still get two nodes (Mermaid ids are global). When the id differs from the name, a `state "Name" as id` line keeps the name as the label.

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
  "version": 2,
  "machine": "Player",
  "initial": 0,
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
| `version` | `2`. Also `estado_graph::JSON_VERSION`. Version 2 added `initial`. |
| `initial` | The id of the state the machine starts in (`Graph::initial`), or `null` for a graph without states. |
| `states[]` | Same order as `Graph::states`, so `states[n].id == n`. |
| `states[].kind` | `atomic`, `compound`, `parallel`, `region`, `final`, `choice` |
| `states[].history` | `none`, `shallow`, `deep` |
| `states[].parent`, `.initial` | A state id, or `null` |
| `edges[].kind` | `normal`, `external`, `history`, `error`, `timer`, `always`, `done` |
| `edges[].to` | `{"type": "state", "id": n}`, `{"type": "choice", "id": n}` or `{"type": "dynamic"}` |
| `edges[].event` | The event pattern as text, or `null` |

The JSON is a faithful copy of the `Graph`. The tests of the crate validate the output of every example against the schema, so the schema and the exporter cannot drift apart.

## Analyses

`GraphAnalysis` is implemented for `Graph`; the same four functions are free functions of the crate (`estado_graph::reachable(&graph)`, and so on). Nothing here needs `std`.

```rust,ignore
pub trait GraphAnalysis {
    fn reachable(&self) -> Vec<StateId>;
    fn dead_ends(&self) -> Vec<StateId>;
    fn edge_cover(&self) -> Vec<Path>;
    fn shortest_path(&self, from: StateId, to: StateId) -> Option<Path>;
}
```

```rust
use estado::prelude::*;
use estado_graph::GraphAnalysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Play,
    Pause,
    Stop,
}

#[estado::machine(event = Event, allow(W0001, W0002))]
mod player {
    use super::*;

    #[root]
    pub enum Player {
        #[initial]
        Idle,
        Active(Active),
        Failed,
        Orphan,
    }

    #[state]
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

        #[on(Active)]
        fn on_active(_active: ActiveFields<'_>, event: &Event) -> Next<Self> {
            match event {
                Event::Stop => goto!(Failed),
                _ => unhandled!(),
            }
        }

        #[on(Orphan)]
        fn on_orphan(_event: &Event) -> Next<Self> {
            goto!(Idle)
        }
    }
}

use player::Player;

fn main() {
    let graph = <Player as Spec>::GRAPH;
    let name = |ids: Vec<StateId>| -> Vec<&'static str> {
        ids.into_iter().map(|id| graph.state(id).name).collect()
    };

    // `Orphan` has an edge out but none into it.
    assert_eq!(name(graph.reachable()), ["Idle", "Active", "Playing", "Paused", "Failed"]);
    // `Paused` is left by the handler of `Active`; `Failed` is a leaf with no edge.
    assert_eq!(name(graph.dead_ends()), ["Failed"]);

    let idle = graph.find("Idle").unwrap().id;
    let failed = graph.find("Failed").unwrap().id;
    let path = graph.shortest_path(idle, failed).unwrap();
    assert_eq!(path.len(), 2);
    assert!(graph.shortest_path(failed, idle).is_none());

    // One walk takes the three reachable edges: Idle -> Active -> Paused ->
    // Failed. The edge out of `Orphan` is in no path.
    assert_eq!(graph.edge_cover().len(), 1);
}
```

### Model

The analyses read only what the graph records: edges, their kinds and the state tree. ADR [0019](../decisions/0019-graph-analyses.md) has the reasons.

| Question | Rule |
| --- | --- |
| When can an edge be taken? | When the state that declares it is active. A handler of a compound or parallel state applies to everything inside it. |
| What does entering a state activate? | The state, its ancestors, its `#[initial]` chain, every region of a parallel state, and the other regions of the parallel states above it. |
| What is certainly active after an edge? | The target, its ancestors and what its entry activates, including the other regions of a parallel state the edge enters. A transition inside one region says nothing about the other regions. |
| `goto!(T, history)` | Entered as if nothing had been recorded: the initial substate. |
| Timer, `#[always]` and `#[on_done]` edges | Taken as soon as the source is active, whatever might make them fire. |
| `goto_dyn!` | Reserved. It can land anywhere: `reachable()` includes every state. Paths do not follow it. |
| `#[choice]` | Reserved. A choice is entered and left like a state. |
| `reenter!()` | Not an edge of the graph, so no analysis sees it. |

### reachable

The states that can become active, in declaration order, starting from `Graph::initial`. An edge counts when its source is reachable. Compound states, parallel states and regions are listed when something in them is. Empty when the graph has no initial state. This is the rule behind the macro warning W0001.

### dead_ends

The atomic states that are not `#[terminal]` and that neither declare an edge nor have an ancestor that does, in declaration order: once the machine gets there it stays. A self-transition counts as a way out. Compound and parallel states are never listed, only their leaves. Every state is checked, reachable or not. This is the rule behind W0002.

### shortest_path

`shortest_path(from, to)` starts as if `from` had just been entered, so its ancestors and initial substates are active, and ends when `to` is active. It returns the path with the fewest edges; between equal ones, the edges declared first win. If `to` is already active the path is empty; if it cannot be reached the result is `None`. Both states must belong to the graph.

### edge_cover

The fewest paths, all starting at the initial state, that together take every edge that can be taken. The count is the minimum, not a heuristic: edges that lead back to each other are taken by one walk, and the rest form a partial order whose width is the number of paths. Paths may overlap and repeat edges and are not the shortest ones. An edge whose source can only be activated through `goto_dyn!` is in no path. The output is sorted by edges, so it is the same on every run. It is empty when the graph has no initial state or no edge can be taken.

Each step of a path is declared by a state that the step before it entered, so a test can drive the machine edge by edge. After a `goto!(T, history)` the machine may be in another substate than the initial one, which the graph cannot tell.

### Path

```rust,ignore
impl Path {
    pub fn start(&self) -> StateId;          // active before the first edge
    pub fn edges(&self) -> &[usize];         // indices into Graph::edges
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn infos<'graph>(&'graph self, graph: &'graph Graph)
        -> impl Iterator<Item = &'graph EdgeInfo> + 'graph;
}
```

### worst_case_actions

`Graph::worst_case_actions()` is a `const fn` of `estado-core`, so it works without `alloc`. It is the largest number of `#[entry]` and `#[exit]` calls one transition can make: for each edge, the states that exit (the active chain from the leaf up to the child of the lowest common ancestor, every region of a parallel state, and the deepest configuration below the source) plus the states that enter (down to the target, its initial chain, every region of a parallel state). Spec table 5.2 gives the exits and entries of each form of edge, and the count follows it.

- Every state counts once, whether or not it declares an entry or exit handler: the graph does not say, so the result is an upper bound. Regions and `#[choice]` pseudostates count zero.
- One transition is one microstep. The start of the machine, and the transitions that `#[always]`, `#[on_done]` or a raised event cause afterwards, are not added in.
- `goto!(T, history)` counts the largest configuration below `T`. A target known only at run time counts as the costliest real one.
- `Graph::edge_actions(&edge)` gives the figure of one edge and `Graph::reenter_actions(state)` that of `reenter!()` in a handler of `state`.

`#[machine]` also generates the constant `Root::WORST_CASE_ACTIONS`: the graph's figure raised to the cost of every `reenter!()` in the machine, which is not an edge of the graph. It is an ordinary `const`, so it can bound a build:

```rust
use estado::prelude::*;

#[estado::machine(event = ())]
mod light {
    use super::*;

    #[root]
    pub enum Light {
        #[initial]
        Off,
        On,
    }

    #[handlers]
    impl Light {
        #[on(Off)]
        fn on_off(_event: &()) -> Next<Self> {
            goto!(On)
        }

        #[on(On)]
        fn on_on(_event: &()) -> Next<Self> {
            goto!(Off)
        }
    }
}

use light::Light;

// One state exits and one enters.
const _: () = assert!(Light::WORST_CASE_ACTIONS == 2);

fn main() {
    assert_eq!(<Light as Spec>::GRAPH.worst_case_actions(), 2);
}
```

A machine written by hand has no such constant; it writes `const BOUND: usize = Motor::GRAPH.worst_case_actions();`.

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

- **Events are text.** The label comes from the pattern of the `match` arm (`event_hint`). A handler without a `match`, or an arm with a wildcard or a binding, has no hint, and its edge has no label.
- **Mermaid has no history pseudostate and no names for regions.** History is drawn as a labelled state and regions as comments, as described above. Mermaid versions differ in what they render for edges that cross compound states; the DOT output gives exact control over clusters.
- **Choice and dynamic targets** (`#[choice]`, `goto_dyn!`) are reserved: the macro does not generate them yet ([E0090](../errors/E0090.md)). The exporters handle them for graphs built by hand.
- **Features.** The crate is `no_std` with `alloc`. The default `std` feature adds `assert_snapshot!` and the [`snapshot`](#snapshot-helpers) module.
