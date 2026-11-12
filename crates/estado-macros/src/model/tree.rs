//! The state tree (spec 3): one node per state, region and pseudostate.

use syn::{Expr, Field, Ident, ItemEnum, ItemStruct, Type};

use super::options::UnhandledPolicy;

/// Index of a node in [`StateTree::nodes`]; also its runtime `StateId`.
pub type StateId = usize;

/// All states of a machine. `nodes[0]` is the root enum itself, which is not
/// a state of the graph; its variants are the top-level states.
pub struct StateTree {
    /// Nodes in pre-order (parent before children, declaration order).
    pub nodes: Vec<StateNode>,
    /// The `#[state]` and `#[root]` type declarations, by name, for
    /// re-emission and for views.
    pub types: Vec<TypeDecl>,
}

/// One state, region, choice pseudostate, or the root.
pub struct StateNode {
    /// Index in the tree.
    pub id: StateId,
    /// State name (variant name; field name for a region; enum name for the root).
    pub name: Ident,
    /// Parent node; `None` only for the root.
    pub parent: Option<StateId>,
    /// Children in declaration order: substates of a compound or region,
    /// regions of a parallel state, top-level states of the root.
    pub children: Vec<StateId>,
    /// Nesting depth; top-level states are 0, the root is not counted.
    pub depth: u8,
    /// What it is.
    pub kind: StateKind,
    /// The enum this state is a variant of (`None` for the root and regions).
    pub owner_enum: Option<Ident>,
    /// How the variant carries data.
    pub payload: Payload,
    /// For compound and parallel states: the `#[state]` struct holding the
    /// shared fields and the `#[substates]`/`#[region]` fields.
    pub composite_struct: Option<Ident>,
    /// For a compound: the `#[substates]` field name. For a region: the
    /// `#[region]` field name in the parallel struct (same as `name`).
    pub children_field: Option<Ident>,
    /// For a compound, region or the root: the enum listing the children.
    pub children_enum: Option<Ident>,
    /// The `#[initial]` child of a compound, region or the root (the first
    /// one when E0010 reports several).
    pub initial: Option<StateId>,
    /// This variant carries `#[initial]` (or `#[initial(with = ..)]`).
    /// Added in 4.1 so analysis can count initials per enum.
    pub is_initial: bool,
    /// `#[initial(with = path)]` constructor of this state, when it has data.
    pub initial_with: Option<syn::Path>,
    /// `#[history]` / `#[history(deep, keep)]` on a compound.
    pub history: HistoryKind,
    /// `#[final]`.
    pub is_final: bool,
    /// `#[after(..)]` timers, in declaration order (index = position).
    pub timers: Vec<AfterSpec>,
    /// `#[invoke(..)]` tasks, in declaration order (index = position).
    pub invokes: Vec<InvokeSpec>,
    /// `#[unhandled(..)]` override (spec 5.1 item 4).
    pub unhandled: Option<UnhandledPolicy>,
}

/// Kind of a node (spec 3.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StateKind {
    /// The `#[root]` enum.
    Root,
    /// Leaf.
    Atomic,
    /// Has `#[substates]`.
    Compound,
    /// Has two or more `#[region]`s.
    Parallel,
    /// One `#[region]` field.
    Region,
}

/// How a variant carries its data.
pub enum Payload {
    /// `Idle`.
    Unit,
    /// `Active(Active)` where `Active` is a `#[state]` struct, or any
    /// single-field tuple variant `Ready(u32)`.
    Newtype(Type),
    /// `Point(u8, u8)`.
    Tuple(Vec<Field>),
    /// `Playing { pos: Duration }`.
    Struct(Vec<Field>),
    /// The root and regions carry no variant payload.
    None,
}

/// History kept by a compound (spec 3.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HistoryKind {
    /// None.
    None,
    /// `#[history]`.
    Shallow,
    /// `#[history(deep, keep)]`.
    DeepKeep,
}

/// `#[after(duration => event)]`: both expressions may be closures over the
/// state data (`|s| ..`), evaluated at entry and at fire time (spec 7.1).
pub struct AfterSpec {
    /// Duration expression.
    pub duration: Expr,
    /// Event expression.
    pub event: Expr,
}

/// `#[invoke(task)]`: an expression, possibly a closure over the state data,
/// producing a user effect (spec 7.2).
pub struct InvokeSpec {
    /// Task expression.
    pub task: Expr,
}

/// A type declared in the machine module that is part of the machine.
pub enum TypeDecl {
    /// `#[root]` enum or `#[state]` enum, with machine attributes removed.
    Enum(ItemEnum),
    /// `#[state]` struct, with machine attributes removed.
    Struct(ItemStruct),
}

impl StateTree {
    /// The root node.
    pub fn root(&self) -> &StateNode {
        &self.nodes[0]
    }

    /// A node by id.
    pub fn node(&self, id: StateId) -> &StateNode {
        &self.nodes[id]
    }

    /// Looks a state up by name (any level). Names are unique (E0012).
    pub fn find(&self, name: &Ident) -> Option<StateId> {
        self.nodes
            .iter()
            .skip(1)
            .position(|node| node.name == *name)
            .map(|index| index + 1)
    }

    /// Ancestors from `id` (inclusive) up to, excluding, the root.
    pub fn ancestors(&self, id: StateId) -> Vec<StateId> {
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(node) = current.filter(|&node| node != 0) {
            chain.push(node);
            current = self.nodes[node].parent;
        }
        chain
    }

    /// Least common ancestor of two nodes (0 = the root).
    pub fn lca(&self, first: StateId, second: StateId) -> StateId {
        let first_chain = self.ancestors(first);
        let second_chain = self.ancestors(second);
        first_chain
            .iter()
            .copied()
            .find(|node| second_chain.contains(node))
            .unwrap_or(0)
    }

    /// Whether `ancestor` is `node` or one of its ancestors.
    pub fn is_ancestor_or_self(&self, ancestor: StateId, node: StateId) -> bool {
        ancestor == 0 || self.ancestors(node).contains(&ancestor)
    }

    /// Graph ids: every node but the root, shifted by one, so the first
    /// top-level state is `StateId(0)` at runtime.
    pub fn runtime_id(&self, id: StateId) -> u16 {
        u16::try_from(id - 1).expect("at most 65535 states")
    }
}
