//! Emits the `#[handlers]` impl: entries, exits and one `#[on]` handler per
//! state whose `match` holds the planned `goto!` edges.

use crate::edges::{Arm, Edge, EdgeForm, Plan, constructor_exiting_state};
use crate::events::render_pattern;
use crate::tree::{NodeId, NodeKind, Tree};

/// The whole `#[handlers] impl Plant { .. }` block.
pub fn render_handlers(tree: &Tree, plan: &Plan) -> String {
    let mut text = String::from("    #[handlers]\n    impl Plant {\n");
    for id in tree.state_ids() {
        render_entry(tree, id, &mut text);
        render_exit(tree, id, &mut text);
    }
    for id in tree.state_ids() {
        if !tree.node(id).terminal {
            render_handler(tree, id, &plan.arms[id], &mut text);
        }
    }
    text.push_str("    }\n");
    text
}

fn function_suffix(tree: &Tree, id: NodeId) -> String {
    tree.node(id).name.to_lowercase()
}

/// Compounds and parallel states always have an entry; every fourth atomic
/// state does.
fn has_entry(tree: &Tree, id: NodeId) -> bool {
    tree.node(id).kind != NodeKind::Atomic || id % 4 == 0
}

fn has_exit(tree: &Tree, id: NodeId) -> bool {
    tree.node(id).kind == NodeKind::Parallel || id % 6 == 0
}

fn render_entry(tree: &Tree, id: NodeId, text: &mut String) {
    if !has_entry(tree, id) {
        return;
    }
    let node = tree.node(id);
    let suffix = function_suffix(tree, id);
    let (parameter, body) = match (node.kind, node.has_data) {
        (NodeKind::Atomic, true) => (
            format!("state: {}Mut<'_>, ", node.name),
            "*state.attempts += 1;\n            *cx.ctx_mut() += 1;".to_owned(),
        ),
        (NodeKind::Compound | NodeKind::Parallel, true) => (
            format!("up: {}Fields<'_>, ", node.name),
            "*up.generation += 1;\n            cx.emit(Command::Notify(*up.generation));"
                .to_owned(),
        ),
        _ if id % 8 == 0 => (String::new(), "cx.emit(Command::Alarm);".to_owned()),
        _ => (String::new(), "*cx.ctx_mut() += 1;".to_owned()),
    };
    text.push_str(&format!(
        "        #[entry({name})]\n        fn entry_{suffix}({parameter}cx: &mut Cx<Self>) {{\n            {body}\n        }}\n\n",
        name = node.name
    ));
}

fn render_exit(tree: &Tree, id: NodeId, text: &mut String) {
    if !has_exit(tree, id) {
        return;
    }
    let node = tree.node(id);
    let suffix = function_suffix(tree, id);
    text.push_str(&format!(
        "        #[exit({name})]\n        fn exit_{suffix}(cx: &mut Cx<Self>) {{\n            cx.emit(Command::Cleanup(1));\n        }}\n\n",
        name = node.name
    ));
}

fn render_handler(tree: &Tree, id: NodeId, arms: &[Arm], text: &mut String) {
    let node = tree.node(id);
    let suffix = function_suffix(tree, id);
    let uses_context = arms
        .iter()
        .enumerate()
        .any(|(index, arm)| uses_context(arm, index));
    let context = if uses_context { "cx" } else { "_cx" };
    text.push_str(&format!(
        "        #[on({name})]\n        fn on_{suffix}({context}: &mut Cx<Self>, event: &Event) -> Next<Self> {{\n            match event {{\n",
        name = node.name
    ));
    for (index, arm) in arms.iter().enumerate() {
        render_arm(tree, id, arm, index, text);
    }
    text.push_str("                _ => bubble!(),\n            }\n        }\n\n");
}

/// Every other edge arm counts its transition in the context; `stay!()`
/// arms always do.
fn uses_context(arm: &Arm, index: usize) -> bool {
    arm.edge.is_none() || index % 2 == 1
}

fn render_arm(tree: &Tree, id: NodeId, arm: &Arm, index: usize, text: &mut String) {
    let pattern = render_pattern(arm.event, id + index);
    match arm.edge {
        None => text.push_str(&format!(
            "                {pattern} => {{\n                    *cx.ctx_mut() += 1;\n                    stay!()\n                }}\n"
        )),
        Some(edge) => {
            let transition = render_goto(tree, id, edge);
            if uses_context(arm, index) {
                text.push_str(&format!(
                    "                {pattern} => {{\n                    *cx.ctx_mut() += 1;\n                    {transition}\n                }}\n"
                ));
            } else {
                text.push_str(&format!("                {pattern} => {transition},\n"));
            }
        }
    }
}

/// One level of a `goto!` path: the name and the data the level needs.
fn render_level(tree: &Tree, id: NodeId) -> String {
    let node = tree.node(id);
    match (node.kind, node.has_data) {
        (NodeKind::Compound | NodeKind::Parallel, true) => {
            format!("{} {{ generation: 0 }}", node.name)
        }
        (NodeKind::Atomic, true) => format!("{} {{ attempts: 1 }}", node.name),
        _ => node.name.clone(),
    }
}

/// The path of a simple or history `goto!`: every state below the LCA down to
/// the target, with the data each one needs (spec 6.3, E0020). A target above
/// the handler needs no data.
fn render_path(tree: &Tree, source: NodeId, target: NodeId) -> String {
    let lca = tree.transition_lca(source, target);
    if target == lca {
        return tree.node(target).name.clone();
    }
    let mut levels = Vec::new();
    let mut current = target;
    while current != lca {
        if tree.is_state(current) {
            levels.push(render_level(tree, current));
        }
        current = tree.node(current).parent.expect("below the LCA");
    }
    levels.reverse();
    levels.join(" / ")
}

fn render_goto(tree: &Tree, source: NodeId, edge: Edge) -> String {
    match edge.form {
        EdgeForm::Simple => format!("goto!({})", render_path(tree, source, edge.target)),
        EdgeForm::History => format!("goto!({}, history)", render_path(tree, source, edge.target)),
        EdgeForm::Constructor => {
            let exiting = constructor_exiting_state(tree, source, edge.target)
                .expect("a constructor edge has an exiting state");
            let value = constructor_value(tree, edge.target);
            format!(
                "goto!({target}, |_old: Exiting![{exiting}]| {value})",
                target = tree.node(edge.target).name,
                exiting = tree.node(exiting).name
            )
        }
    }
}

fn constructor_value(tree: &Tree, target: NodeId) -> String {
    let owner = tree.owner_enum_name(target);
    let node = tree.node(target);
    if node.has_data {
        format!("{owner}::{} {{ attempts: 1 }}", node.name)
    } else {
        format!("{owner}::{}", node.name)
    }
}
