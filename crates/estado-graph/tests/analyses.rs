//! Spec 8.3 on hand-built graphs: the corner cases of ADR 0019.

use estado_core::{EdgeInfo, EdgeKind, EdgeTarget, Graph, StateId, StateInfo, StateKind};
use estado_graph::GraphAnalysis;

/// A small builder for graphs of a few states. Ids are indices in the order
/// the states are added, so add parents before children.
#[derive(Default)]
struct Builder {
    states: Vec<StateInfo>,
    edges: Vec<EdgeInfo>,
}

impl Builder {
    fn state(&mut self, name: &'static str, parent: Option<StateId>, kind: StateKind) -> StateId {
        let id = StateId::new(u16::try_from(self.states.len()).unwrap());
        self.states.push(StateInfo::new(id, name, parent, kind, 0));
        id
    }

    fn atomic(&mut self, name: &'static str) -> StateId {
        self.state(name, None, StateKind::Atomic)
    }

    fn child(&mut self, name: &'static str, parent: StateId) -> StateId {
        self.state(name, Some(parent), StateKind::Atomic)
    }

    fn initial(&mut self, state: StateId, initial: StateId) {
        self.states[state.index()].initial = Some(initial);
    }

    fn edge(&mut self, from: StateId, to: StateId) -> usize {
        self.edge_with(from, EdgeTarget::State(to), EdgeKind::Normal)
    }

    fn edge_with(&mut self, from: StateId, to: EdgeTarget, kind: EdgeKind) -> usize {
        self.edges.push(EdgeInfo {
            from,
            to,
            kind,
            event_hint: None,
        });
        self.edges.len() - 1
    }

    fn build(self, initial: Option<StateId>) -> Graph {
        Graph {
            machine: "Test",
            initial,
            states: Box::leak(self.states.into_boxed_slice()),
            edges: Box::leak(self.edges.into_boxed_slice()),
        }
    }
}

fn names(graph: &Graph, ids: &[StateId]) -> Vec<&'static str> {
    ids.iter().map(|&id| graph.state(id).name).collect()
}

// ---------------------------------------------------------------- reachable

#[test]
fn reachable_follows_edges_from_the_initial_state() {
    let mut builder = Builder::default();
    let start = builder.atomic("Start");
    let middle = builder.atomic("Middle");
    let end = builder.atomic("End");
    let island = builder.atomic("Island");
    builder.edge(start, middle);
    builder.edge(middle, end);
    builder.edge(island, start);
    let graph = builder.build(Some(start));

    assert_eq!(
        names(&graph, &graph.reachable()),
        ["Start", "Middle", "End"]
    );
}

#[test]
fn reachable_is_empty_without_an_initial_state() {
    let mut builder = Builder::default();
    let only = builder.atomic("Only");
    builder.edge(only, only);
    let graph = builder.build(None);

    assert!(graph.reachable().is_empty());
    assert!(graph.edge_cover().is_empty());
}

#[test]
fn entering_a_compound_state_reaches_its_initial_chain_and_not_its_siblings() {
    let mut builder = Builder::default();
    let idle = builder.atomic("Idle");
    let active = builder.state("Active", None, StateKind::Compound);
    let playing = builder.child("Playing", active);
    let _paused = builder.child("Paused", active);
    builder.initial(active, playing);
    builder.edge(idle, active);
    let graph = builder.build(Some(idle));

    // `Paused` has no edge into it and is not the initial substate.
    assert_eq!(
        names(&graph, &graph.reachable()),
        ["Idle", "Active", "Playing"]
    );
}

#[test]
fn an_edge_declared_by_a_compound_state_is_available_from_its_substates() {
    let mut builder = Builder::default();
    let active = builder.state("Active", None, StateKind::Compound);
    let playing = builder.child("Playing", active);
    let off = builder.atomic("Off");
    builder.initial(active, playing);
    builder.edge(active, off);
    let graph = builder.build(Some(active));

    assert_eq!(
        names(&graph, &graph.reachable()),
        ["Active", "Playing", "Off"]
    );
    // `Playing` declares nothing itself, yet it is not a dead end.
    assert_eq!(names(&graph, &graph.dead_ends()), ["Off"]);
    let path = graph.shortest_path(playing, off).unwrap();
    assert_eq!(path.edges(), [0]);
}

#[test]
fn entering_a_parallel_state_reaches_every_region() {
    let mut builder = Builder::default();
    let idle = builder.atomic("Idle");
    let alive = builder.state("Alive", None, StateKind::Parallel);
    let movement = builder.state("movement", Some(alive), StateKind::Region);
    let standing = builder.child("Standing", movement);
    let walking = builder.child("Walking", movement);
    let weapon = builder.state("weapon", Some(alive), StateKind::Region);
    let ready = builder.child("Ready", weapon);
    let firing = builder.child("Firing", weapon);
    builder.initial(movement, standing);
    builder.initial(weapon, ready);
    builder.edge(idle, alive);
    builder.edge(standing, walking);
    builder.edge(ready, firing);
    let graph = builder.build(Some(idle));

    assert_eq!(graph.reachable().len(), graph.states.len());
}

#[test]
fn an_edge_to_a_state_inside_a_region_also_starts_the_other_regions() {
    let mut builder = Builder::default();
    let idle = builder.atomic("Idle");
    let alive = builder.state("Alive", None, StateKind::Parallel);
    let movement = builder.state("movement", Some(alive), StateKind::Region);
    let standing = builder.child("Standing", movement);
    let walking = builder.child("Walking", movement);
    let weapon = builder.state("weapon", Some(alive), StateKind::Region);
    let ready = builder.child("Ready", weapon);
    let firing = builder.child("Firing", weapon);
    builder.initial(movement, standing);
    builder.initial(weapon, ready);
    builder.edge(idle, walking);
    builder.edge(ready, firing);
    let graph = builder.build(Some(idle));

    // `Standing` is the initial of `movement` but `Walking` was entered
    // directly: it is only reachable because entering `Alive` started both.
    let reachable = names(&graph, &graph.reachable());
    assert!(reachable.contains(&"Ready"));
    assert!(reachable.contains(&"Firing"));
    assert!(reachable.contains(&"Walking"));
}

#[test]
fn completion_timer_and_history_edges_count_like_any_other() {
    let mut builder = Builder::default();
    let first = builder.atomic("First");
    let by_timer = builder.atomic("ByTimer");
    let by_always = builder.atomic("ByAlways");
    let by_done = builder.atomic("ByDone");
    let by_history = builder.atomic("ByHistory");
    builder.edge_with(first, EdgeTarget::State(by_timer), EdgeKind::Timer);
    builder.edge_with(by_timer, EdgeTarget::State(by_always), EdgeKind::Always);
    builder.edge_with(by_always, EdgeTarget::State(by_done), EdgeKind::Done);
    builder.edge_with(by_done, EdgeTarget::State(by_history), EdgeKind::History);
    let graph = builder.build(Some(first));

    assert_eq!(graph.reachable().len(), 5);
}

#[test]
fn a_choice_pseudostate_is_reached_and_followed() {
    let mut builder = Builder::default();
    let start = builder.atomic("Start");
    let pick = builder.state("Pick", None, StateKind::Choice);
    let left = builder.atomic("Left");
    let right = builder.atomic("Right");
    builder.edge_with(start, EdgeTarget::Choice(pick), EdgeKind::Normal);
    builder.edge(pick, left);
    builder.edge(pick, right);
    let graph = builder.build(Some(start));

    assert_eq!(graph.reachable().len(), 4);
    let path = graph.shortest_path(start, right).unwrap();
    assert_eq!(path.edges(), [0, 2]);
}

#[test]
fn a_dynamic_edge_makes_every_state_reachable_but_not_every_edge_coverable() {
    let mut builder = Builder::default();
    let start = builder.atomic("Start");
    let anywhere = builder.atomic("Anywhere");
    let beyond = builder.atomic("Beyond");
    builder.edge_with(start, EdgeTarget::Dynamic, EdgeKind::Normal);
    builder.edge(anywhere, beyond);
    let graph = builder.build(Some(start));

    assert_eq!(graph.reachable().len(), 3);
    assert!(graph.dead_ends().iter().all(|&id| id == beyond));
    // Paths only follow edges whose destination is known.
    assert!(graph.shortest_path(start, beyond).is_none());
    // The dynamic edge is taken; the edge after the unknown landing is not.
    let cover = graph.edge_cover();
    assert_eq!(cover.len(), 1);
    assert_eq!(cover[0].edges(), [0]);
}

// ---------------------------------------------------------------- dead_ends

#[test]
fn dead_ends_are_atomic_states_nothing_can_leave() {
    let mut builder = Builder::default();
    let start = builder.atomic("Start");
    let stuck = builder.atomic("Stuck");
    let done = builder.state("Done", None, StateKind::Final);
    let looping = builder.atomic("Looping");
    builder.edge(start, stuck);
    builder.edge(start, done);
    builder.edge(start, looping);
    builder.edge(looping, looping);
    let graph = builder.build(Some(start));

    // A final state is expected to be a dead end; a self-loop is an exit.
    assert_eq!(names(&graph, &graph.dead_ends()), ["Stuck"]);
}

#[test]
fn dead_ends_list_leaves_not_compound_or_parallel_states() {
    let mut builder = Builder::default();
    let outer = builder.state("Outer", None, StateKind::Compound);
    let inner = builder.child("Inner", outer);
    let alive = builder.state("Alive", None, StateKind::Parallel);
    let region = builder.state("region", Some(alive), StateKind::Region);
    let leaf = builder.child("Leaf", region);
    builder.initial(outer, inner);
    builder.initial(region, leaf);
    let graph = builder.build(Some(outer));

    assert_eq!(names(&graph, &graph.dead_ends()), ["Inner", "Leaf"]);
}

#[test]
fn an_edge_of_the_parallel_state_lets_its_region_leaves_out() {
    let mut builder = Builder::default();
    let alive = builder.state("Alive", None, StateKind::Parallel);
    let region = builder.state("region", Some(alive), StateKind::Region);
    let leaf = builder.child("Leaf", region);
    let dead = builder.state("Dead", None, StateKind::Final);
    builder.initial(region, leaf);
    builder.edge(alive, dead);
    let graph = builder.build(Some(alive));

    assert!(graph.dead_ends().is_empty());
}

// ------------------------------------------------------------ shortest_path

fn line_graph() -> (Graph, [StateId; 4]) {
    // A -> B -> D, A -> C -> D, and D -> A.
    let mut builder = Builder::default();
    let a = builder.atomic("A");
    let b = builder.atomic("B");
    let c = builder.atomic("C");
    let d = builder.atomic("D");
    builder.edge(a, b); // 0
    builder.edge(a, c); // 1
    builder.edge(b, d); // 2
    builder.edge(c, d); // 3
    builder.edge(d, a); // 4
    (builder.build(Some(a)), [a, b, c, d])
}

#[test]
fn shortest_path_takes_the_fewest_edges_and_the_first_declared_on_a_tie() {
    let (graph, [a, _, _, d]) = line_graph();

    let path = graph.shortest_path(a, d).unwrap();
    assert_eq!(path.start(), a);
    assert_eq!(path.edges(), [0, 2]);
    assert_eq!(path.len(), 2);
    let hints: Vec<_> = path.infos(&graph).map(|edge| edge.to).collect();
    assert_eq!(
        hints,
        [
            EdgeTarget::State(StateId::new(1)),
            EdgeTarget::State(StateId::new(3))
        ]
    );
}

#[test]
fn shortest_path_to_the_same_state_is_empty() {
    let (graph, [a, ..]) = line_graph();

    let path = graph.shortest_path(a, a).unwrap();
    assert!(path.is_empty());
    assert_eq!(path.start(), a);
}

#[test]
fn shortest_path_is_none_when_the_target_cannot_be_reached() {
    let mut builder = Builder::default();
    let a = builder.atomic("A");
    let b = builder.atomic("B");
    let c = builder.atomic("C");
    builder.edge(a, b);
    let graph = builder.build(Some(a));

    assert!(graph.shortest_path(a, c).is_none());
    assert!(graph.shortest_path(b, a).is_none());
}

#[test]
fn shortest_path_starts_from_any_state_not_only_the_initial_one() {
    let (graph, [_, _, c, d]) = line_graph();

    assert_eq!(graph.shortest_path(c, d).unwrap().edges(), [3]);
    assert_eq!(graph.shortest_path(d, c).unwrap().edges(), [4, 1]);
}

#[test]
fn shortest_path_counts_what_an_entry_activates() {
    let mut builder = Builder::default();
    let idle = builder.atomic("Idle");
    let active = builder.state("Active", None, StateKind::Compound);
    let playing = builder.child("Playing", active);
    builder.initial(active, playing);
    builder.edge(idle, active);
    let graph = builder.build(Some(idle));

    // Entering `Active` makes `Playing` and `Active` active in one edge.
    assert_eq!(graph.shortest_path(idle, playing).unwrap().edges(), [0]);
    assert_eq!(graph.shortest_path(idle, active).unwrap().edges(), [0]);
    // `Active` is active whenever `Playing` is: nothing to do.
    assert!(graph.shortest_path(playing, active).unwrap().is_empty());
    // The other way round, `Active` activates its initial substate.
    assert!(graph.shortest_path(active, playing).unwrap().is_empty());
}

#[test]
fn shortest_path_uses_an_edge_of_an_ancestor() {
    let mut builder = Builder::default();
    let outer = builder.state("Outer", None, StateKind::Compound);
    let inner = builder.child("Inner", outer);
    let other = builder.atomic("Other");
    builder.initial(outer, inner);
    builder.edge(outer, other);
    let graph = builder.build(Some(outer));

    assert_eq!(graph.shortest_path(inner, other).unwrap().edges(), [0]);
}

// --------------------------------------------------------------- edge_cover

fn assert_covers_every_edge(graph: &Graph, cover: &[estado_graph::Path]) {
    let mut covered = vec![false; graph.edges.len()];
    for path in cover {
        assert_eq!(Some(path.start()), graph.initial);
        for &edge in path.edges() {
            covered[edge] = true;
        }
    }
    assert!(covered.iter().all(|&edge| edge), "{covered:?}");
}

#[test]
fn one_path_covers_a_cycle() {
    let (graph, _) = line_graph();

    let cover = graph.edge_cover();
    assert_eq!(cover.len(), 1);
    assert_covers_every_edge(&graph, &cover);
    // A -> B -> D -> A -> C -> D: all five edges.
    assert_eq!(cover[0].edges(), [0, 2, 4, 1, 3]);
}

#[test]
fn a_branch_that_cannot_be_rejoined_needs_a_path_of_its_own() {
    // Start has three exits and none of the targets leads anywhere.
    let mut builder = Builder::default();
    let start = builder.atomic("Start");
    for name in ["One", "Two", "Three"] {
        let leaf = builder.atomic(name);
        builder.edge(start, leaf);
    }
    let graph = builder.build(Some(start));

    let cover = graph.edge_cover();
    assert_eq!(cover.len(), 3);
    assert_covers_every_edge(&graph, &cover);
    assert!(cover.iter().all(|path| path.len() == 1));
}

#[test]
fn a_chain_with_a_side_branch_needs_two_paths() {
    // Start -> Middle -> End and Middle -> Side: two sinks.
    let mut builder = Builder::default();
    let start = builder.atomic("Start");
    let middle = builder.atomic("Middle");
    let end = builder.atomic("End");
    let side = builder.atomic("Side");
    builder.edge(start, middle);
    builder.edge(middle, end);
    builder.edge(middle, side);
    let graph = builder.build(Some(start));

    let cover = graph.edge_cover();
    assert_eq!(cover.len(), 2);
    assert_covers_every_edge(&graph, &cover);
    assert_eq!(cover[0].edges(), [0, 1]);
    assert_eq!(cover[1].edges(), [0, 2]);
}

#[test]
fn a_path_may_pass_through_a_cycle_to_reach_a_sink() {
    // Start -> Loop (self-loop) -> End.
    let mut builder = Builder::default();
    let start = builder.atomic("Start");
    let spin = builder.atomic("Spin");
    let end = builder.atomic("End");
    builder.edge(start, spin);
    builder.edge(spin, spin);
    builder.edge(spin, end);
    let graph = builder.build(Some(start));

    let cover = graph.edge_cover();
    assert_eq!(cover.len(), 1);
    assert_eq!(cover[0].edges(), [0, 1, 2]);
}

#[test]
fn edges_whose_source_is_unreachable_are_in_no_path() {
    let mut builder = Builder::default();
    let start = builder.atomic("Start");
    let end = builder.atomic("End");
    let island = builder.atomic("Island");
    builder.edge(start, end);
    builder.edge(island, end);
    let graph = builder.build(Some(start));

    let cover = graph.edge_cover();
    assert_eq!(cover.len(), 1);
    assert_eq!(cover[0].edges(), [0]);
}

#[test]
fn edges_of_a_machine_without_edges_have_no_cover() {
    let mut builder = Builder::default();
    let only = builder.atomic("Only");
    let graph = builder.build(Some(only));

    assert!(graph.edge_cover().is_empty());
}

#[test]
fn an_edge_of_a_region_follows_the_entry_of_the_parallel_state() {
    // After `Idle -> Alive` both regions are in their initial states, so the
    // edges of the two regions can each come right after it.
    let mut builder = Builder::default();
    let idle = builder.atomic("Idle");
    let alive = builder.state("Alive", None, StateKind::Parallel);
    let movement = builder.state("movement", Some(alive), StateKind::Region);
    let standing = builder.child("Standing", movement);
    let walking = builder.child("Walking", movement);
    let weapon = builder.state("weapon", Some(alive), StateKind::Region);
    let ready = builder.child("Ready", weapon);
    let firing = builder.child("Firing", weapon);
    builder.initial(movement, standing);
    builder.initial(weapon, ready);
    builder.edge(idle, alive); // 0
    builder.edge(standing, walking); // 1
    builder.edge(ready, firing); // 2
    let graph = builder.build(Some(idle));

    let cover = graph.edge_cover();
    assert_covers_every_edge(&graph, &cover);
    // The two region edges are independent sinks: two paths.
    assert_eq!(cover.len(), 2);
    assert_eq!(cover[0].edges(), [0, 1]);
    assert_eq!(cover[1].edges(), [0, 2]);
}

#[test]
fn every_step_of_a_path_is_declared_by_a_state_the_step_before_entered() {
    let mut builder = Builder::default();
    let idle = builder.atomic("Idle");
    let active = builder.state("Active", None, StateKind::Compound);
    let playing = builder.child("Playing", active);
    let paused = builder.child("Paused", active);
    let failed = builder.atomic("Failed");
    builder.initial(active, playing);
    builder.edge(idle, active);
    builder.edge(playing, paused);
    builder.edge(paused, playing);
    builder.edge(active, failed);
    builder.edge(failed, idle);
    let graph = builder.build(Some(idle));

    let cover = graph.edge_cover();
    assert_covers_every_edge(&graph, &cover);
    for path in &cover {
        assert_walkable(&graph, path);
    }
}

/// Whether `state` is active right after `target` was entered: the target,
/// an ancestor, or a state the default entry reaches.
pub fn active_after_entering(graph: &Graph, target: StateId, state: StateId) -> bool {
    if graph.is_ancestor_or_self(state, target) {
        return true;
    }
    let info = graph.state(target);
    match info.kind {
        StateKind::Parallel => graph
            .states
            .iter()
            .filter(|child| child.parent == Some(target))
            .any(|region| active_after_entering(graph, region.id, state)),
        StateKind::Compound | StateKind::Region => info
            .initial
            .is_some_and(|initial| active_after_entering(graph, initial, state)),
        _ => false,
    }
}

fn assert_walkable(graph: &Graph, path: &estado_graph::Path) {
    let mut entered = path.start();
    for edge in path.infos(graph) {
        assert!(
            active_after_entering(graph, entered, edge.from),
            "{} is not active after entering {}",
            graph.state(edge.from).name,
            graph.state(entered).name
        );
        match edge.to {
            EdgeTarget::State(target) | EdgeTarget::Choice(target) => entered = target,
            EdgeTarget::Dynamic => break,
        }
    }
}
