//! The synthetic state tree: an arena of nodes plus the geometry the macro
//! applies to a `goto!` (ancestors, LCA, owner enum), so the generator only
//! emits transitions the macro accepts.

/// Index of a node in [`Tree::nodes`]. Node 0 is the root enum.
pub type NodeId = usize;

/// What a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// The root enum (not a state of its own).
    Root,
    /// A variant without substates.
    Atomic,
    /// A struct with `#[substates]`.
    Compound,
    /// A struct with `#[region]` fields.
    Parallel,
    /// One region enum of a parallel state (not a state of its own).
    Region,
}

/// Which history a compound state records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryKind {
    /// No history.
    None,
    /// `#[history]`.
    Shallow,
    /// `#[history(deep, keep)]`.
    DeepKeep,
}

/// The `#[after(..)]` timer of an atomic state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerKind {
    /// No timer.
    None,
    /// A constant duration, event `Timeout`.
    Constant,
    /// A closure over the data of the state, event `Deadline`.
    FromData,
}

/// One node of the tree.
#[derive(Clone, Debug)]
pub struct Node {
    /// Variant and struct name of a state; enum name of a region or the root.
    pub name: String,
    /// What the node is.
    pub kind: NodeKind,
    /// The enclosing node (the owner of the enum the node is a variant of).
    pub parent: Option<NodeId>,
    /// Substates of a compound or root, regions of a parallel state, states
    /// of a region.
    pub children: Vec<NodeId>,
    /// Compound and parallel states carry `generation: u32`; atomic states
    /// carry `attempts: u8`. Never set on an `#[initial]` state.
    pub has_data: bool,
    /// `#[terminal]`.
    pub terminal: bool,
    /// The `#[initial]` variant of its level.
    pub initial: bool,
    /// History recorded by a compound state.
    pub history: HistoryKind,
    /// Timer of an atomic state.
    pub timer: TimerKind,
    /// Field name of a region inside its parallel struct.
    pub region_field: String,
}

impl Node {
    /// A new node with everything switched off.
    pub fn new(name: String, kind: NodeKind, parent: Option<NodeId>) -> Self {
        Self {
            name,
            kind,
            parent,
            children: Vec::new(),
            has_data: false,
            terminal: false,
            initial: false,
            history: HistoryKind::None,
            timer: TimerKind::None,
            region_field: String::new(),
        }
    }
}

/// The arena.
#[derive(Debug, Default)]
pub struct Tree {
    /// All nodes, root first.
    pub nodes: Vec<Node>,
}

impl Tree {
    /// Borrows a node.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    /// Whether the node is a state of the machine (not the root, not a region).
    pub fn is_state(&self, id: NodeId) -> bool {
        matches!(
            self.node(id).kind,
            NodeKind::Atomic | NodeKind::Compound | NodeKind::Parallel
        )
    }

    /// Every state, in declaration order.
    pub fn state_ids(&self) -> Vec<NodeId> {
        (0..self.nodes.len())
            .filter(|&id| self.is_state(id))
            .collect()
    }

    /// Parent first, root last.
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = Vec::new();
        let mut current = self.node(id).parent;
        while let Some(parent) = current {
            chain.push(parent);
            current = self.node(parent).parent;
        }
        chain
    }

    /// Distance from the root.
    pub fn depth(&self, id: NodeId) -> usize {
        self.ancestors(id).len()
    }

    /// `ancestor` is `node` or above it.
    pub fn is_ancestor_or_self(&self, ancestor: NodeId, node: NodeId) -> bool {
        ancestor == node || self.ancestors(node).contains(&ancestor)
    }

    /// Lowest common ancestor of two distinct, unrelated nodes.
    pub fn lowest_common_ancestor(&self, first: NodeId, second: NodeId) -> NodeId {
        let above_second = self.ancestors(second);
        self.ancestors(first)
            .into_iter()
            .find(|candidate| above_second.contains(candidate))
            .unwrap_or(0)
    }

    /// The LCA of a plain `goto!` (spec table 5.2), as the macro computes it.
    pub fn transition_lca(&self, handler: NodeId, target: NodeId) -> NodeId {
        let parent_of = |id: NodeId| self.node(id).parent.unwrap_or(0);
        if target == handler {
            parent_of(handler)
        } else if self.is_ancestor_or_self(handler, target) {
            handler
        } else if self.is_ancestor_or_self(target, handler) {
            target
        } else {
            self.lowest_common_ancestor(handler, target)
        }
    }

    /// Name of the enum a node is a variant of.
    pub fn owner_enum_name(&self, id: NodeId) -> String {
        let owner = self.node(id).parent.unwrap_or(0);
        self.substates_enum_name(owner)
    }

    /// Name of the enum holding the substates of `owner`.
    pub fn substates_enum_name(&self, owner: NodeId) -> String {
        match self.node(owner).kind {
            NodeKind::Root | NodeKind::Region => self.node(owner).name.clone(),
            _ => format!("{}Sub", self.node(owner).name),
        }
    }

    /// Whether a node of `kind` exists strictly below `id`.
    pub fn has_kind_below(&self, id: NodeId, kind: NodeKind) -> bool {
        self.node(id)
            .children
            .iter()
            .any(|&child| self.node(child).kind == kind || self.has_kind_below(child, kind))
    }

    /// Whether `id` lies inside a parallel state.
    pub fn inside_parallel(&self, id: NodeId) -> bool {
        self.ancestors(id)
            .into_iter()
            .any(|ancestor| self.node(ancestor).kind == NodeKind::Parallel)
    }

    /// Whether a deep history lives at or below `id`.
    pub fn has_deep_history_at_or_below(&self, id: NodeId) -> bool {
        self.node(id).history == HistoryKind::DeepKeep
            || self
                .node(id)
                .children
                .iter()
                .any(|&child| self.has_deep_history_at_or_below(child))
    }
}
