//! Unit tests of the W0001/W0002 analysis on small trees.

use quote::quote;

use super::reachability::{EdgeSource, dead_end_states, unreachable_states};
use crate::model::{StateId, StateTree};
use crate::parse;

/// Parses a machine module and returns the tree.
fn tree_of(module: proc_macro2::TokenStream) -> StateTree {
    let module: syn::ItemMod = syn::parse2(module).expect("a module");
    let items = module.content.clone().expect("inline").1;
    let parsed = parse::parse_machine(quote! { event = Ev }, &module, items);
    assert!(parsed.diagnostics.is_empty());
    parsed.model.expect("a model").tree
}

fn id_of(tree: &StateTree, name: &str) -> StateId {
    tree.nodes
        .iter()
        .position(|node| node.name == name)
        .unwrap_or_else(|| panic!("no state `{name}`"))
}

fn edge(tree: &StateTree, from: &str, targets: &[&str]) -> EdgeSource {
    EdgeSource {
        from: id_of(tree, from),
        targets: targets.iter().map(|name| id_of(tree, name)).collect(),
        has_dynamic_target: false,
    }
}

fn names(tree: &StateTree, ids: Vec<StateId>) -> Vec<String> {
    ids.into_iter()
        .map(|id| tree.node(id).name.to_string())
        .collect()
}

fn player() -> StateTree {
    tree_of(quote! {
        mod player {
            #[root]
            enum Player { #[initial] Idle, Active(Active), Orphan, #[terminal] Done }
            #[state]
            struct Active { #[substates] sub: ActiveSub }
            #[state]
            enum ActiveSub { #[initial] Playing, Paused }
        }
    })
}

#[test]
fn initial_configuration_and_edge_targets_are_reachable() {
    let tree = player();
    let edges = [
        edge(&tree, "Idle", &["Active"]),
        edge(&tree, "Playing", &["Paused"]),
        edge(&tree, "Active", &["Done"]),
    ];
    let unreachable = names(&tree, unreachable_states(&tree, &edges));
    assert_eq!(unreachable, ["Orphan"]);
}

#[test]
fn edges_from_unreachable_states_do_not_count() {
    let tree = player();
    let edges = [
        edge(&tree, "Idle", &["Active"]),
        edge(&tree, "Orphan", &["Done"]),
    ];
    let unreachable = names(&tree, unreachable_states(&tree, &edges));
    assert_eq!(unreachable, ["Paused", "Orphan", "Done"]);
}

#[test]
fn entering_a_deep_state_reaches_its_ancestors() {
    let tree = player();
    // `Playing` (the initial substate) is skipped by entering `Paused` directly.
    let edges = [edge(&tree, "Idle", &["Paused"])];
    let unreachable = names(&tree, unreachable_states(&tree, &edges));
    assert_eq!(unreachable, ["Playing", "Orphan", "Done"]);
}

#[test]
fn only_the_topmost_unreachable_state_is_reported() {
    let tree = player();
    // Nothing enters `Active`: its substates are not repeated.
    let unreachable = names(&tree, unreachable_states(&tree, &[]));
    assert_eq!(unreachable, ["Active", "Orphan", "Done"]);
}

#[test]
fn a_dynamic_target_silences_unreachability() {
    let tree = player();
    let edges = [EdgeSource {
        from: id_of(&tree, "Idle"),
        targets: Vec::new(),
        has_dynamic_target: true,
    }];
    assert!(unreachable_states(&tree, &edges).is_empty());
}

#[test]
fn parallel_regions_are_entered_together() {
    let tree = tree_of(quote! {
        mod machine {
            #[root]
            enum Root { #[initial] Both(Both), Elsewhere }
            #[state]
            struct Both { #[region] left: Left, #[region] right: Right }
            #[state]
            enum Left { #[initial] LeftA, LeftB }
            #[state]
            enum Right { #[initial] RightA }
        }
    });
    let edges = [edge(&tree, "LeftA", &["LeftB"])];
    let unreachable = names(&tree, unreachable_states(&tree, &edges));
    assert_eq!(unreachable, ["Elsewhere"]);
}

#[test]
fn dead_ends_are_leaves_without_edges_for_them_or_their_ancestors() {
    let tree = player();
    let edges = [
        edge(&tree, "Idle", &["Active"]),
        edge(&tree, "Playing", &["Paused"]),
    ];
    // `Paused` has no edge, `Playing` has one; `Orphan` has none;
    // `Done` is `#[terminal]` and `Idle` leaves.
    let dead = names(&tree, dead_end_states(&tree, &edges));
    assert_eq!(dead, ["Paused", "Orphan"]);
}

#[test]
fn an_ancestor_handler_lets_every_descendant_leave() {
    let tree = player();
    let edges = [edge(&tree, "Active", &["Idle"])];
    let dead = names(&tree, dead_end_states(&tree, &edges));
    assert_eq!(dead, ["Idle", "Orphan"]);
}

fn in_play(tree: &StateTree, name: &str) -> Vec<String> {
    let mut names: Vec<String> = super::scope::states_in_play(tree, id_of(tree, name))
        .into_iter()
        .filter(|&id| id != 0)
        .map(|id| tree.node(id).name.to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn a_state_in_play_sees_its_ancestors_and_everything_below() {
    let tree = player();
    assert_eq!(in_play(&tree, "Active"), ["Active", "Paused", "Playing"]);
    assert_eq!(in_play(&tree, "Paused"), ["Active", "Paused"]);
    assert_eq!(in_play(&tree, "Idle"), ["Idle"]);
}

#[test]
fn inside_a_parallel_state_the_sibling_regions_are_in_play() {
    let tree = tree_of(quote! {
        mod machine {
            #[root]
            enum Root { #[initial] Both(Both), Elsewhere }
            #[state]
            struct Both { #[region] left: Left, #[region] right: Right }
            #[state]
            enum Left { #[initial] LeftA }
            #[state]
            enum Right { #[initial] RightA }
        }
    });
    let seen = in_play(&tree, "LeftA");
    assert!(seen.contains(&"RightA".to_owned()));
    assert!(seen.contains(&"Both".to_owned()));
    assert!(!seen.contains(&"Elsewhere".to_owned()));
}
