//! Builds the synthetic tree: a fixed, deterministic recipe that mixes
//! atomic states, compound states a few levels deep and parallel states with
//! three regions, until the state budget is spent exactly.

use crate::tree::{HistoryKind, Node, NodeId, NodeKind, TimerKind, Tree};

/// Compound states nest up to this many levels below the root.
const MAXIMUM_NESTING: usize = 5;
/// Sizes (the state itself plus everything below it) tried in turn below the
/// top level.
const COMPOSITE_SIZES: [usize; 8] = [9, 14, 7, 12, 6, 10, 17, 8];
/// Sizes at the top level: a few big subsystems rather than many small ones.
const TOP_LEVEL_SIZES: [usize; 6] = [37, 29, 23, 31, 19, 26];
/// A parallel state needs its own node plus two states in each of 3 regions.
const MINIMUM_PARALLEL_SIZE: usize = 7;
/// Region names are state names, so they are unique across the machine.
const REGION_LETTERS: [&str; 3] = ["a", "b", "c"];
const MAXIMUM_TIMERS: usize = 8;

/// Builds a tree with exactly `state_budget` states below the root.
pub fn build_tree(state_budget: usize) -> Tree {
    let mut builder = Builder::default();
    builder
        .tree
        .nodes
        .push(Node::new("Plant".to_owned(), NodeKind::Root, None));
    builder.fill_level(0, state_budget, 0);
    builder.assign_timers();
    builder.assign_histories();
    builder.tree
}

#[derive(Default)]
struct Builder {
    tree: Tree,
    atomic_count: usize,
    compound_count: usize,
    parallel_count: usize,
    composite_count: usize,
    level_count: usize,
}

impl Builder {
    fn add_node(&mut self, name: String, kind: NodeKind, parent: NodeId) -> NodeId {
        let id = self.tree.nodes.len();
        self.tree.nodes.push(Node::new(name, kind, Some(parent)));
        self.tree.nodes[parent].children.push(id);
        id
    }

    /// Fills `owner` with states that total exactly `size`.
    fn fill_level(&mut self, owner: NodeId, size: usize, depth: usize) {
        let mut remaining = size;
        let mut position = 0;
        while remaining > 0 {
            let rhythm = if depth == 0 { 2 } else { 3 };
            let nests = depth < MAXIMUM_NESTING && remaining >= 4 && position % rhythm == 1;
            if nests {
                let sizes: &[usize] = if depth == 0 {
                    &TOP_LEVEL_SIZES
                } else {
                    &COMPOSITE_SIZES
                };
                let composite_size = sizes[self.composite_count % sizes.len()].min(remaining);
                self.composite_count += 1;
                let parallel_wanted =
                    (depth == 0 && self.parallel_count == 0) || self.composite_count % 4 == 0;
                let allowed = !self.tree.inside_parallel(owner)
                    && self.tree.node(owner).kind != NodeKind::Region;
                if parallel_wanted && allowed && composite_size >= MINIMUM_PARALLEL_SIZE {
                    self.add_parallel(owner, composite_size, depth, position);
                } else {
                    self.add_compound(owner, composite_size, depth, position);
                }
                remaining -= composite_size;
            } else {
                self.add_atomic(owner, position);
                remaining -= 1;
            }
            position += 1;
        }
        self.finish_level(owner);
    }

    fn add_atomic(&mut self, owner: NodeId, position: usize) {
        self.atomic_count += 1;
        let name = format!("Node{:03}", self.atomic_count);
        let id = self.add_node(name, NodeKind::Atomic, owner);
        self.tree.nodes[id].has_data = position != 0 && self.atomic_count % 4 == 1;
    }

    fn add_compound(&mut self, owner: NodeId, size: usize, depth: usize, position: usize) {
        self.compound_count += 1;
        let name = format!("Group{:03}", self.compound_count);
        let id = self.add_node(name, NodeKind::Compound, owner);
        self.tree.nodes[id].has_data = position != 0;
        self.fill_level(id, size - 1, depth + 1);
    }

    fn add_parallel(&mut self, owner: NodeId, size: usize, depth: usize, position: usize) {
        self.parallel_count += 1;
        let name = format!("Zone{:03}", self.parallel_count);
        let id = self.add_node(name.clone(), NodeKind::Parallel, owner);
        self.tree.nodes[id].has_data = position != 0;
        let below = size - 1;
        for (index, letter) in REGION_LETTERS.iter().enumerate() {
            let part = below / 3 + usize::from(index < below % 3);
            let region = self.add_node(
                format!("{name}Lane{}", letter.to_uppercase()),
                NodeKind::Region,
                id,
            );
            self.tree.nodes[region].region_field =
                format!("lane_{letter}_{:03}", self.parallel_count);
            self.fill_level(region, part, depth + 1);
        }
    }

    /// Marks the initial state of the level and, now and then, a terminal one.
    fn finish_level(&mut self, owner: NodeId) {
        self.level_count += 1;
        let children = self.tree.nodes[owner].children.clone();
        if let Some(&first) = children.first() {
            self.tree.nodes[first].initial = true;
            self.tree.nodes[first].has_data = false;
        }
        let owner_kind = self.tree.node(owner).kind;
        let may_end = matches!(owner_kind, NodeKind::Root | NodeKind::Compound);
        if may_end && children.len() >= 3 && self.level_count % 3 == 0 {
            let last = *children.last().expect("a level has children");
            let node = &mut self.tree.nodes[last];
            if node.kind == NodeKind::Atomic && !node.has_data && !node.initial {
                node.terminal = true;
            }
        }
    }

    /// A few `#[after]` timers: constant ones on plain states, closures over
    /// the data on states that carry some.
    fn assign_timers(&mut self) {
        let mut assigned = 0;
        for (sequence, id) in self.tree.state_ids().into_iter().enumerate() {
            if assigned >= MAXIMUM_TIMERS {
                break;
            }
            let node = &mut self.tree.nodes[id];
            if node.kind != NodeKind::Atomic || node.terminal {
                continue;
            }
            if node.has_data && sequence % 5 == 0 {
                node.timer = TimerKind::FromData;
                assigned += 1;
            } else if !node.has_data && sequence % 9 == 2 {
                node.timer = TimerKind::Constant;
                assigned += 1;
            }
        }
    }

    fn has_deep_history_above(&self, id: NodeId) -> bool {
        self.tree
            .ancestors(id)
            .into_iter()
            .any(|ancestor| self.tree.node(ancestor).history == HistoryKind::DeepKeep)
    }

    /// `#[history]` on compounds whose substates are all unit variants, and
    /// `#[history(deep, keep)]` on a few compounds with nested states and no
    /// parallel state anywhere near them.
    fn assign_histories(&mut self) {
        let mut shallow_seen = 0;
        let mut deep_seen = 0;
        for id in self.tree.state_ids() {
            if self.tree.node(id).kind != NodeKind::Compound || self.tree.inside_parallel(id) {
                continue;
            }
            let children = self.tree.node(id).children.clone();
            let all_unit = children.iter().all(|&child| {
                let node = self.tree.node(child);
                node.kind == NodeKind::Atomic && !node.has_data
            });
            if all_unit {
                shallow_seen += 1;
                if shallow_seen % 2 == 1 {
                    self.tree.nodes[id].history = HistoryKind::Shallow;
                }
            } else if !self.tree.has_kind_below(id, NodeKind::Parallel)
                && !self.has_deep_history_above(id)
            {
                // Nested deep histories are rejected (E0090).
                deep_seen += 1;
                if deep_seen % 4 == 1 {
                    self.tree.nodes[id].history = HistoryKind::DeepKeep;
                }
            }
        }
    }
}
