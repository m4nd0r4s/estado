# 0018. Graph export: formats, snapshots and the JSON schema

Status: accepted
Date: 2026-11-13
Spec: 8.1, 8.2; spec errata 19

## Context

Spec 8.2 asks `estado-graph` for four exports (`to_mermaid()`, `to_dot()`, `to_scxml()`, `to_json()`), a JSON schema in `docs/graph.schema.json`, and `estado_graph::assert_snapshot!(Player::GRAPH, "docs/player.mmd")`, which fails when the file is out of date and rewrites it with `ESTADO_BLESS=1`. The spec leaves open how labels look, how Mermaid and DOT draw history, how SCXML expresses the kinds of edge, which snapshot tool to use and how the schema is checked. Bead 5.1 also needs the output to be stable and escaped for each format.

## Decision

1. **API.** Free functions `to_mermaid`, `to_dot`, `to_scxml` and `to_json` take a `&Graph`, and the trait `GraphExport` offers the same as methods (`Player::GRAPH.to_mermaid()`, the form the spec writes). `Format` names a format and maps file extensions to it. The crate stays `no_std` with `alloc`; a default `std` feature adds the snapshot module.
2. **Determinism.** States and edges come out in the order of the graph. Nothing is sorted, hashed or timestamped. Mermaid ids, SCXML ids and DOT node ids are derived from the state name and index, and clashes are resolved by appending the index.
3. **Labels.** An edge label is the event hint (white space collapsed) followed by the kind in brackets for every kind but `normal`: `Event::Land [timer]`. The spec defines no guard information (`EdgeInfo` has none), so there is none to show. In Mermaid, labels and aliases are written with entities for `: ; # " < > & \` (`Event#58;#58;Pause`): mermaid-cli 11 rejects a second `:` in a transition line, and renders the entities as the characters.
4. **History** is drawn as a pseudostate (`H`, `H*`) inside the compound state in Mermaid and DOT, with `history` edges ending at it, and as `<history type="shallow|deep">` with a default transition in SCXML. Mermaid has no history syntax; a labelled state is the closest portable form.
5. **SCXML.** Event hints become event descriptors (`::` to `.`, an or-pattern is a list of tokens). `#[always]` edges have no event, `#[on_done]` edges listen to `done.state.<id>`. `type` is `internal` only for an edge to a strict descendant of its source that is not `external` (the only case SCXML allows), else `external`. A final state with outgoing edges is written as `<state>`, as SCXML finals cannot have transitions. No executable content.
6. **Edges in Mermaid** are written inside the innermost compound state or region containing both ends, so nested diagrams read like the source. DOT draws every edge at the top level and uses anchor nodes with `ltail`/`lhead` for edges at clusters.
7. **Snapshots: an in-house helper, no `insta`.** The spec names `assert_snapshot!` and `ESTADO_BLESS`, not `insta`; a 150-line module does the whole job (exact comparison modulo line endings, first-difference report, create-or-rewrite when blessing) with no dependency. `insta` stays available for bead 5.3 (`Harness::trace`), where the spec names it. The format comes from the file extension; the path is relative to the manifest directory of the calling crate (`env!("CARGO_MANIFEST_DIR")` in the macro), which for a test is also its working directory.
8. **JSON schema check.** `docs/graph.schema.json` is JSON Schema 2020-12. The tests validate the output of a graph that uses every kind of state, edge and target, and of the committed example snapshots, against it with a 150-line validator for the keywords the schema uses (`serde_json` as a dev-dependency only). A test fails if the schema starts using a keyword the validator does not know, and further tests check that mutated documents are rejected. A full validator crate would pull in dozens of dependencies for one file.
9. **Example snapshots** live in `docs/graphs/<scenario>.{mmd,dot,scxml,json}` for the macro-generated machines of scenarios A, B, D and I (AC-14, and the "mermaid of all the examples in `docs/`" of milestone M4). The hand-written versions list their edges in another order, so they are not snapshotted.
10. **The initial state of the machine** is not in `Graph`; see spec errata 19.

## Consequences

- Any change to the generated graph, or to an exporter, shows up as a diff of the files in `docs/graphs/`, to be reviewed and blessed with `ESTADO_BLESS=1`.
- Every committed `.mmd` renders with mermaid-cli 11.4.2, every `.dot` with Graphviz `dot`, the SCXML is well-formed XML and the JSON follows the schema. The tests do not run these tools; re-check by hand after changing an exporter.
- A root enum whose `#[initial]` is not its first variant exports a wrong initial state until `Graph` records it.
