//! Builds the [`StateTree`] nodes from the root enum, recursively (spec 3).

use syn::spanned::Spanned;
use syn::{Fields, Ident, ItemEnum, ItemStruct, Type, Variant};

use super::attributes::{AttributeSite, NodeAttributes, parse_node_attributes};
use super::composite::{ChildField, Composition, classify};
use super::items::ModuleItems;
use crate::model::{Diagnostic, HistoryKind, Payload, StateId, StateKind, StateNode};

/// Builds every node, root first, in pre-order.
pub fn build_nodes(
    items: &ModuleItems,
    root: &ItemEnum,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<StateNode> {
    let mut builder = TreeBuilder {
        items,
        nodes: Vec::new(),
        enum_stack: Vec::new(),
        diagnostics,
    };
    let mut root_node = blank_node(0, root.ident.clone(), None, 0, StateKind::Root);
    root_node.payload = Payload::None;
    builder.nodes.push(root_node);
    builder.add_enum_children(0, root, 0);
    builder.nodes
}

struct TreeBuilder<'items, 'sink> {
    items: &'items ModuleItems,
    nodes: Vec<StateNode>,
    /// Enums being expanded, to stop recursive state types.
    enum_stack: Vec<Ident>,
    diagnostics: &'sink mut Vec<Diagnostic>,
}

fn blank_node(
    id: StateId,
    name: Ident,
    parent: Option<StateId>,
    depth: u8,
    kind: StateKind,
) -> StateNode {
    StateNode {
        id,
        name,
        parent,
        children: Vec::new(),
        depth,
        kind,
        owner_enum: None,
        payload: Payload::Unit,
        composite_struct: None,
        children_field: None,
        children_enum: None,
        initial: None,
        is_initial: false,
        initial_with: None,
        initial_arguments: Vec::new(),
        history: HistoryKind::None,
        is_final: false,
        timers: Vec::new(),
        invokes: Vec::new(),
        unhandled: None,
    }
}

fn payload_of(fields: &Fields) -> Payload {
    match fields {
        Fields::Unit => Payload::Unit,
        Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            Payload::Newtype(unnamed.unnamed[0].ty.clone())
        }
        Fields::Unnamed(unnamed) => Payload::Tuple(unnamed.unnamed.iter().cloned().collect()),
        Fields::Named(named) => Payload::Struct(named.named.iter().cloned().collect()),
    }
}

/// The single identifier of a plain path type such as `ActiveSub`.
fn plain_type_name(field_type: &Type) -> Option<&Ident> {
    match field_type {
        Type::Path(path) if path.qself.is_none() => path.path.get_ident(),
        _ => None,
    }
}

impl<'items> TreeBuilder<'items, '_> {
    fn push_node(&mut self, node: StateNode) -> StateId {
        let id = node.id;
        self.nodes.push(node);
        id
    }

    /// Adds one child per variant of `declaration` under `parent`, which
    /// becomes the owner of the enum. `depth` is the depth of the children.
    fn add_enum_children(&mut self, parent: StateId, declaration: &ItemEnum, depth: u8) {
        self.enum_stack.push(declaration.ident.clone());
        for variant in &declaration.variants {
            let child = self.add_variant(parent, &declaration.ident, variant, depth);
            self.nodes[parent].children.push(child);
        }
        self.enum_stack.pop();
        let first_initial = self.nodes[parent]
            .children
            .iter()
            .copied()
            .find(|&child| self.nodes[child].is_initial);
        let owner = &mut self.nodes[parent];
        owner.children_enum = Some(declaration.ident.clone());
        owner.initial = first_initial;
    }

    fn add_variant(
        &mut self,
        parent: StateId,
        owner: &Ident,
        variant: &Variant,
        depth: u8,
    ) -> StateId {
        let id = self.nodes.len();
        let mut attributes =
            parse_node_attributes(&variant.attrs, AttributeSite::Variant, self.diagnostics);
        let mut node = blank_node(
            id,
            variant.ident.clone(),
            Some(parent),
            depth,
            StateKind::Atomic,
        );
        node.owner_enum = Some(owner.clone());
        node.payload = payload_of(&variant.fields);
        let items = self.items;
        let composite = match &node.payload {
            Payload::Newtype(payload_type) => {
                plain_type_name(payload_type).and_then(|name| items.state_struct(name))
            }
            _ => None,
        };
        self.push_node(node);
        if let Some(declaration) = composite {
            let struct_attributes = parse_node_attributes(
                &declaration.attrs,
                AttributeSite::CompositeStruct,
                self.diagnostics,
            );
            attributes.merge(struct_attributes);
            self.expand_composite(id, declaration, depth);
            self.check_history(id, declaration, &attributes);
        }
        self.apply_attributes(id, attributes);
        id
    }

    fn apply_attributes(&mut self, id: StateId, attributes: NodeAttributes) {
        let node = &mut self.nodes[id];
        node.is_initial = attributes.initial.is_some();
        node.initial_with = attributes.initial.and_then(|initial| initial.with);
        node.is_final = attributes.is_final;
        node.history = attributes.history;
        node.timers = attributes.timers;
        node.invokes = attributes.invokes;
        node.unhandled = attributes.unhandled;
    }

    fn check_history(
        &mut self,
        id: StateId,
        declaration: &ItemStruct,
        attributes: &NodeAttributes,
    ) {
        if attributes.history != HistoryKind::None && self.nodes[id].kind != StateKind::Compound {
            self.diagnostics.push(
                Diagnostic::new(
                    "E0017",
                    declaration.ident.span(),
                    format!(
                        "`#[history]` on `{}`, which is not a compound state",
                        declaration.ident
                    ),
                )
                .with_help("history is kept by states that have a `#[substates]` field"),
            );
        }
    }

    fn expand_composite(&mut self, id: StateId, declaration: &ItemStruct, depth: u8) {
        match classify(declaration, self.diagnostics) {
            Composition::Plain => {}
            Composition::Compound(field) => {
                let node = &mut self.nodes[id];
                node.kind = StateKind::Compound;
                node.composite_struct = Some(declaration.ident.clone());
                node.children_field = Some(field.name.clone());
                self.expand_children(id, &field, depth.saturating_add(1));
            }
            Composition::Parallel(fields) => {
                let node = &mut self.nodes[id];
                node.kind = StateKind::Parallel;
                node.composite_struct = Some(declaration.ident.clone());
                for field in fields {
                    self.add_region(id, &field, depth.saturating_add(1));
                }
            }
        }
    }

    fn add_region(&mut self, parent: StateId, field: &ChildField, depth: u8) {
        let id = self.nodes.len();
        let mut region = blank_node(
            id,
            field.name.clone(),
            Some(parent),
            depth,
            StateKind::Region,
        );
        region.payload = Payload::None;
        region.children_field = Some(field.name.clone());
        self.push_node(region);
        self.nodes[parent].children.push(id);
        self.expand_children(id, field, depth.saturating_add(1));
    }

    /// Resolves the enum of a `#[substates]`/`#[region]` field and adds its
    /// variants under `parent`.
    fn expand_children(&mut self, parent: StateId, field: &ChildField, depth: u8) {
        let Some(declaration) = self.resolve_enum(field) else {
            return;
        };
        if self.enum_stack.contains(&declaration.ident) {
            self.diagnostics.push(
                Diagnostic::new(
                    "E0011",
                    field.field_type.span(),
                    format!(
                        "`{}` contains itself, so the nesting depth is unbounded",
                        declaration.ident
                    ),
                )
                .with_help("a state type cannot be one of its own descendants"),
            );
            return;
        }
        self.add_enum_children(parent, declaration, depth);
    }

    fn resolve_enum(&mut self, field: &ChildField) -> Option<&'items ItemEnum> {
        let items = self.items;
        let found = plain_type_name(&field.field_type).and_then(|name| items.state_enum(name));
        if found.is_none() {
            let mut diagnostic = Diagnostic::new(
                "E0015",
                field.field_type.span(),
                format!("the type of `{}` must be a `#[state]` enum", field.name),
            );
            let unmarked =
                plain_type_name(&field.field_type).and_then(|name| items.unmarked_enum(name));
            diagnostic = match unmarked {
                Some(declaration) => {
                    diagnostic.with_help(format!("add `#[state]` to `enum {}`", declaration.ident))
                }
                None => diagnostic.with_help(
                    "declare an enum of the substates in this module and mark it `#[state]`",
                ),
            };
            self.diagnostics.push(diagnostic);
        }
        found
    }
}
