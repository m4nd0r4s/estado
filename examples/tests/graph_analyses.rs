//! The analyses of spec 8.3 (ADR 0019) on the graphs of the reference
//! scenarios A, B, D and I, as the macro generates them.

use estado_core::{EdgeTarget, Graph, Spec, StateId, StateKind};
use estado_graph::{GraphAnalysis, Path};

use estado_examples::a_http_parser_macro::HttpParser;
use estado_examples::b_connection_macro::Conn;
use estado_examples::d_game_character_macro::Character;
use estado_examples::i_media_player_macro::Player;

fn id(graph: &Graph, name: &str) -> StateId {
    graph
        .find(name)
        .unwrap_or_else(|| panic!("no state {name}"))
        .id
}

fn hints(graph: &Graph, path: &Path) -> Vec<&'static str> {
    path.infos(graph)
        .map(|edge| edge.event_hint.unwrap_or(""))
        .collect()
}

/// Whether `state` is active right after `target` was entered.
fn active_after_entering(graph: &Graph, target: StateId, state: StateId) -> bool {
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

/// Every edge of the graph is in some path, and every step of a path is
/// declared by a state that the step before entered.
fn assert_cover_is_complete_and_walkable(graph: &Graph, cover: &[Path]) {
    let mut covered = vec![false; graph.edges.len()];
    for path in cover {
        assert_eq!(Some(path.start()), graph.initial);
        let mut entered = path.start();
        for (&index, edge) in path.edges().iter().zip(path.infos(graph)) {
            covered[index] = true;
            assert!(
                active_after_entering(graph, entered, edge.from),
                "{} is not active after entering {}",
                graph.state(edge.from).name,
                graph.state(entered).name
            );
            if let EdgeTarget::State(target) = edge.to {
                entered = target;
            }
        }
    }
    let missing: Vec<usize> = (0..covered.len()).filter(|&edge| !covered[edge]).collect();
    assert!(missing.is_empty(), "edges in no path: {missing:?}");
}

// ---------------------------------------------------------------------- I

#[test]
fn i_media_player_analyses() {
    let graph = Player::GRAPH;

    assert_eq!(graph.reachable().len(), graph.states.len());
    assert!(graph.dead_ends().is_empty());

    // Idle -> Active (Playing is its initial) -> Paused.
    let path = graph
        .shortest_path(id(graph, "Idle"), id(graph, "Paused"))
        .unwrap();
    assert_eq!(hints(graph, &path), ["Event::Play", "Event::Pause"]);
    // `Active` declares Stop and Error: from `Paused` they are one edge.
    let path = graph
        .shortest_path(id(graph, "Paused"), id(graph, "Failed"))
        .unwrap();
    assert_eq!(hints(graph, &path), ["Event::Error"]);

    // Every state can be left and re-entered: one walk takes every edge.
    let cover = graph.edge_cover();
    assert_eq!(cover.len(), 1);
    assert_cover_is_complete_and_walkable(graph, &cover);

    // The costliest transition is `Active -> Active`: Playing and Active
    // exit, Active and Playing enter.
    assert_eq!(graph.worst_case_actions(), 4);
    assert_eq!(Player::WORST_CASE_ACTIONS, 4);
}

// ---------------------------------------------------------------------- D

#[test]
fn d_game_character_analyses() {
    let graph = Character::GRAPH;

    assert_eq!(graph.reachable().len(), graph.states.len());
    assert!(graph.dead_ends().is_empty());

    // `Alive` declares the fatal `Damage`, so it is one edge from anywhere.
    let path = graph
        .shortest_path(id(graph, "Walking"), id(graph, "Dead"))
        .unwrap();
    assert_eq!(hints(graph, &path), ["Event::Damage"]);
    // After `Respawn` the regions are in their initial states.
    let path = graph
        .shortest_path(id(graph, "Dead"), id(graph, "Reloading"))
        .unwrap();
    assert_eq!(
        hints(graph, &path),
        ["Event::Respawn", "Event::ReloadRequest"]
    );
    // Entering `Alive` starts every region: no edge needed for `Ready`.
    let path = graph
        .shortest_path(id(graph, "Dead"), id(graph, "Ready"))
        .unwrap();
    assert_eq!(hints(graph, &path), ["Event::Respawn"]);

    let cover = graph.edge_cover();
    assert_cover_is_complete_and_walkable(graph, &cover);
    assert_eq!(cover.len(), 1);

    // `Alive -> Dead` and back: Alive and one leaf per region (three), plus
    // Dead, on either side.
    assert_eq!(graph.worst_case_actions(), 5);
    assert_eq!(Character::WORST_CASE_ACTIONS, 5);
}

// ---------------------------------------------------------------------- B

#[test]
fn b_connection_analyses() {
    let graph = Conn::GRAPH;

    assert_eq!(graph.reachable().len(), graph.states.len());
    assert!(graph.dead_ends().is_empty());

    let path = graph
        .shortest_path(id(graph, "Disconnected"), id(graph, "Dead"))
        .unwrap();
    assert_eq!(
        hints(graph, &path),
        [
            "Event::Connect",
            "Event::Connected",
            "Event::PingDue",
            "Event::PongTimeout",
        ]
    );
    // `Connected` declares Disconnect: from any leaf of the regions it is one edge.
    let path = graph
        .shortest_path(id(graph, "Sending"), id(graph, "Disconnected"))
        .unwrap();
    assert_eq!(hints(graph, &path), ["Event::Disconnect"]);
    // `Connected` is active whenever `Dead` is.
    assert!(
        graph
            .shortest_path(id(graph, "Dead"), id(graph, "Connected"))
            .unwrap()
            .is_empty()
    );

    let cover = graph.edge_cover();
    assert_cover_is_complete_and_walkable(graph, &cover);
    assert_eq!(cover.len(), 1);

    // `Dead -> Backoff` leaves two regions and the parallel state, and
    // enters Backoff; `Connecting -> Connected` and `Connected ->
    // Disconnected` cost the same.
    assert_eq!(graph.worst_case_actions(), 4);
    // `Pinging` uses `reenter!()`: one exit and one enter, cheaper than that.
    assert_eq!(Conn::WORST_CASE_ACTIONS, 4);
}

// ---------------------------------------------------------------------- A

#[test]
fn a_http_parser_analyses() {
    let graph = HttpParser::GRAPH;

    assert_eq!(graph.reachable().len(), graph.states.len());
    // `Error` is `#[terminal]`: a dead end by design, not reported.
    assert!(graph.dead_ends().is_empty());

    // Method -> Target -> Version -> Headers (LineStart) -> BlockLf -> Body.
    let path = graph
        .shortest_path(id(graph, "Method"), id(graph, "Body"))
        .unwrap();
    assert_eq!(path.len(), 5);
    let path = graph
        .shortest_path(id(graph, "Body"), id(graph, "Method"))
        .unwrap();
    assert_eq!(path.len(), 1);

    // Six edges lead into the terminal state and nothing leaves it: six
    // paths, each ending in one of them.
    let into_error = graph
        .edges
        .iter()
        .filter(|edge| edge.to == EdgeTarget::State(id(graph, "Error")))
        .count();
    assert_eq!(into_error, 6);
    let cover = graph.edge_cover();
    assert_eq!(cover.len(), into_error);
    assert_cover_is_complete_and_walkable(graph, &cover);

    // `Version -> Headers` leaves Version and RequestLine and enters
    // Headers and LineStart; so does `BlockLf -> RequestLine` the other way.
    assert_eq!(graph.worst_case_actions(), 4);
    assert_eq!(HttpParser::WORST_CASE_ACTIONS, 4);
}
