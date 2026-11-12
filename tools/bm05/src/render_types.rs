//! Emits the types of the machine: root enum, state structs and enums.

use crate::tree::{HistoryKind, NodeId, NodeKind, TimerKind, Tree};

const DERIVES: &str = "#[derive(Debug, Clone, PartialEq, Eq)]";

/// Root enum, then for each compound or parallel state its struct and enums.
pub fn render_types(tree: &Tree) -> String {
    let mut text = String::new();
    render_enum(tree, 0, true, &mut text);
    for id in tree.state_ids() {
        match tree.node(id).kind {
            NodeKind::Compound => render_compound(tree, id, &mut text),
            NodeKind::Parallel => render_parallel(tree, id, &mut text),
            _ => {}
        }
    }
    text
}

fn render_compound(tree: &Tree, id: NodeId, text: &mut String) {
    let node = tree.node(id);
    text.push_str("    #[state]\n");
    match node.history {
        HistoryKind::None => {}
        HistoryKind::Shallow => text.push_str("    #[history]\n"),
        HistoryKind::DeepKeep => text.push_str("    #[history(deep, keep)]\n"),
    }
    text.push_str(&format!("    {DERIVES}\n    pub struct {} {{\n", node.name));
    if node.has_data {
        text.push_str("        pub generation: u32,\n");
    }
    text.push_str(&format!(
        "        #[substates]\n        pub sub: {},\n    }}\n\n",
        tree.substates_enum_name(id)
    ));
    render_enum(tree, id, false, text);
}

fn render_parallel(tree: &Tree, id: NodeId, text: &mut String) {
    let node = tree.node(id);
    text.push_str(&format!(
        "    #[state]\n    {DERIVES}\n    pub struct {} {{\n",
        node.name
    ));
    if node.has_data {
        text.push_str("        pub generation: u32,\n");
    }
    for &region in &node.children {
        let region_node = tree.node(region);
        text.push_str(&format!(
            "        #[region]\n        pub {}: {},\n",
            region_node.region_field, region_node.name
        ));
    }
    text.push_str("    }\n\n");
    for &region in &node.children {
        render_enum(tree, region, false, text);
    }
}

/// The enum that holds the children of `owner` (the root, a compound state
/// or a region).
fn render_enum(tree: &Tree, owner: NodeId, is_root: bool, text: &mut String) {
    let name = tree.substates_enum_name(owner);
    if is_root {
        text.push_str("    #[root]\n");
    } else {
        text.push_str("    #[state]\n");
    }
    text.push_str(&format!("    {DERIVES}\n    pub enum {name} {{\n"));
    for &child in &tree.node(owner).children {
        render_variant(tree, child, text);
    }
    text.push_str("    }\n\n");
}

fn render_variant(tree: &Tree, id: NodeId, text: &mut String) {
    let node = tree.node(id);
    if node.initial {
        text.push_str("        #[initial]\n");
    }
    if node.terminal {
        text.push_str("        #[terminal]\n");
    }
    match node.timer {
        TimerKind::None => {}
        TimerKind::Constant => {
            text.push_str("        #[after(TIMEOUT_SHORT => Event::Timeout)]\n");
        }
        TimerKind::FromData => text.push_str(
            "        #[after(|state| Duration::from_millis(u64::from(*state.attempts) * 100 + 50) => Event::Deadline)]\n",
        ),
    }
    let name = &node.name;
    match node.kind {
        NodeKind::Compound | NodeKind::Parallel => {
            text.push_str(&format!("        {name}({name}),\n"));
        }
        _ if node.has_data => text.push_str(&format!("        {name} {{ attempts: u8 }},\n")),
        _ => text.push_str(&format!("        {name},\n")),
    }
}
