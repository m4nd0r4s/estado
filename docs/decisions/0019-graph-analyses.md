# 0019. Graph analyses: reachability, paths and the worst-case action count

Status: accepted
Date: 2026-11-17
Spec: 8.3, 5.2, 5.3, 6.3, 6.7

## Context

Spec 8.3 asks for five analyses and a constant: `reachable()`, `dead_ends()`, `edge_cover()`, `shortest_path(from, to)`, `worst_case_actions()` and `const WORST_CASE_ACTIONS: usize`. It gives one line each. It does not say what a path is in a hierarchical machine, how timer, completion and history edges count, what a parallel state does to a path, what "entry+exit calls" counts, where the constant lives, or what the functions return. This ADR decides them. The graph holds edges, kinds and the state tree; it has no guards, no entry or exit handlers and no `reenter!()`.

## Decision

### API

1. **`estado-graph`** (`no_std` with `alloc`): the trait `GraphAnalysis`, implemented for `Graph`, with `reachable() -> Vec<StateId>`, `dead_ends() -> Vec<StateId>`, `edge_cover() -> Vec<Path>` and `shortest_path(from, to) -> Option<Path>`, plus the same four as free functions. State lists come out in declaration order. A `Path` is a start state and a list of edge indices (`Graph::edges`), with `edges()`, `start()`, `len()`, `is_empty()` and `infos(&graph)`. Edges, not states, because the event hints of the edges are what a test needs to drive the machine.
2. **`estado-core`**: `Graph::worst_case_actions()`, `Graph::edge_actions(&EdgeInfo)` and `Graph::reenter_actions(StateId)` are `const fn`s with no allocation, so the bound is computable on a target without `alloc`. They are in the core, not in `estado-graph`, because `#[machine]` evaluates them into the constant: one implementation, tested once.
3. **`Root::WORST_CASE_ACTIONS`** is an inherent associated constant on the root type, like the state ids (`Player::PLAYING`). It is `GRAPH.worst_case_actions()` raised to the cost of every `reenter!()` of the machine. It costs nothing at compile time unless something reads it (BM-05 is unchanged). A hand-written machine writes `const BOUND: usize = Motor::GRAPH.worst_case_actions();`.
4. The graph function `worst_case_actions` is not repeated in `GraphAnalysis`: the inherent method of `Graph` would hide it.

### The model of a path

An edge declared by `from` can be taken whenever `from` is active; a handler of a compound state also fires for its descendants, so the edge is available from all of them. After an edge, the states it entered are certainly active: the target, its ancestors, the default entry below it (the `#[initial]` chain, every region of a parallel state) and, when the transition enters a parallel state, the other regions in their initial states (a transition between states of one region leaves the other regions as they were, so nothing is said about them). The next edge of a path is any edge whose `from` is one of those states. This is sound, every path can be walked on the machine, and complete for covering edges: a state that stays active outside the target was entered by an earlier edge, which can start a path of its own.

Per kind of edge:

- **`goto!(T)`, external, timer, `#[always]`, `#[on_done]`**: an edge like any other. A completion or timer edge is followed as soon as its source is reachable; the graph does not say whether the completion can happen, so this over-approximates reachability, which is the safe direction for a warning.
- **`goto!(T, history)`**: followed as if nothing had been recorded, entering the initial. The state a history restores was active before and is reachable by the edge that entered it then.
- **`goto_dyn!`** (`EdgeTarget::Dynamic`): reserved. For `reachable()` it can land anywhere, so every state is reachable. For paths its destination is unknown: the edge can end a path and nothing is said to follow it.
- **`#[choice]`** (`EdgeTarget::Choice`): reserved. A choice is a node that is entered and left like a state; the edges that leave it follow.

### The analyses

- **`reachable()`**: the closure from `Graph::initial`: entering a state activates it, its ancestors, the other regions of the parallel states among them, and its default entry. An edge counts when its source is reachable. Compound and region states are listed when something in them is reachable. Empty without an initial state. W0001 in the macro uses the same rule (this ADR fixes one disagreement: entering one region by an explicit path starts the others, and W0001 used to report the others).
- **`dead_ends()`**: atomic, non-final states that neither declare an edge nor have an ancestor that does. A self-loop is an exit. Compound, parallel and region states are not listed, only their leaves (what W0002 does); every state is checked, reachable or not.
- **`shortest_path(from, to)`**: starts as if `from` had just been entered; ends when `to` is active. Fewest edges; ties go to the lowest edge index. A `to` already active gives the empty path; `None` when there is none. Panics if a state is not in the graph, like `Graph::state`.
- **`edge_cover()`**: the **minimum** number of paths from the initial state that, together, take every edge that can be taken. Edges that follow each other in a cycle can be taken by one walk, so the strongly connected components of the "can follow" relation are the units; they form a partial order, and the least number of walks is the least number of chains of it (Dilworth: a maximum bipartite matching on the transitive closure). Each chain becomes one walk, taking the missing edges of each component by nearest first. Paths may overlap and repeat edges and are not shortest. An edge whose source can only be activated through `goto_dyn!` is in no path. Output is sorted by edges, so it is deterministic. Empty without an initial state or edges.

### What `worst_case_actions` counts

The number of `#[entry]` and `#[exit]` calls of one transition, the exits then the entries of 5.3:

- A transition is one microstep. Chained microsteps (`#[always]`, `#[on_done]`, raised events) are other transitions; the start of the machine is not a transition.
- **Every state that exits or enters counts once**, whether or not it declares an `#[entry]` or `#[exit]`: the graph does not say, so the result is an upper bound. Regions and `#[choice]` pseudostates have no handlers and count zero.
- Exits and entries follow table 5.2 by the relation of source and target. Unrelated: from the child of the LCA toward the source, down to the deepest configuration below the source, to the child of the LCA toward the target and down to the target. Local to a descendant: everything below the source exits. Local to an ancestor: from the child of the ancestor down to the source exits, and the initial below the ancestor enters; for a parallel ancestor everything below it exits. External or to itself: the target exits and enters again.
- **The configuration below the source is the worst one**: the deepest chain of a compound state, every region of a parallel state, and every other region of a parallel state on the way up or down.
- Entering a compound state enters its initial chain, entering a parallel state every region. `goto!(T, history)` may restore any configuration, so it counts the largest one below `T`.
- A target known only at run time (`goto_dyn!`, `#[choice]`) counts as the costliest real target.
- **`reenter!()`** is `goto!(H)` (table 5.2) but is not an edge of the graph, so `Graph::worst_case_actions()` cannot see it. The macro knows the handlers that use it and `WORST_CASE_ACTIONS` includes the cost of each. The constant can therefore be higher than the graph function (spec 6.3 keeps the graph the exact union of the generated edges; bead estado-99i.5.6 considers recording `reenter!()` as an edge).
- The helpers rescan the state table instead of memoising, to stay `const fn` without allocation. The compiler may stop a constant evaluation that runs very long (`long_running_const_eval`); the 200-state, 600-edge machine of BM-05 evaluates well within the limit, and a run-time call has none.

## Consequences

- `WORST_CASE_ACTIONS` is an upper bound for WCET analysis, exact when every state has both handlers.
- The tests pin each rule: `estado-core/tests/worst_case_actions.rs` (a hand-built graph with compound and parallel states, one test per row of 5.2), `estado-macros/tests/worst_case_actions.rs` (the constant, with and without `reenter!()`), `estado-graph/tests/analyses.rs` (corner cases: parallel regions, dynamic and choice targets, history and completion edges, cycles, sinks, unreachable edges) and `examples/tests/graph_analyses.rs` (scenarios A, B, D and I).
- Following the minimality result means the cover of a machine with `n` terminal edges has at least `n` paths (scenario A: six edges into the final state, six paths).
- A model-based harness (`Harness::cover_edges`, bead 5.3) can walk a path edge by edge: each edge is declared by a state the previous one entered. History can restore a substate other than the initial one, which the graph cannot tell; a driver should expect that.
