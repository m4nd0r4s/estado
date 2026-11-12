//! The generator hits the BM-05 sizes exactly and only plans edges the macro
//! accepts.

use crate::edges::{EdgeForm, is_legal};
use crate::tree::NodeKind;
use crate::{EDGE_BUDGET, STATE_BUDGET, generate};

#[test]
fn the_machine_has_200_states_and_600_edges() {
    let (tree, plan, source) = generate(STATE_BUDGET, EDGE_BUDGET);
    assert_eq!(tree.state_ids().len(), 200);
    assert_eq!(plan.edge_count(), 600);
    assert_eq!(source.matches("goto!(").count(), 600);
}

#[test]
fn the_machine_mixes_atomic_compound_and_parallel_states() {
    let (tree, _, _) = generate(STATE_BUDGET, EDGE_BUDGET);
    let count = |kind: NodeKind| tree.nodes.iter().filter(|node| node.kind == kind).count();
    assert!(count(NodeKind::Atomic) > 100);
    assert!(count(NodeKind::Compound) > 10);
    assert!(count(NodeKind::Parallel) >= 1);
    assert!(count(NodeKind::Region) >= 3);
    assert!(tree.state_ids().iter().any(|&id| tree.depth(id) >= 5));
}

#[test]
fn every_planned_edge_is_legal() {
    let (tree, plan, _) = generate(STATE_BUDGET, EDGE_BUDGET);
    for (source, arms) in plan.arms.iter().enumerate() {
        for edge in arms.iter().filter_map(|arm| arm.edge) {
            assert!(is_legal(&tree, source, edge.target, edge.form));
        }
    }
    let forms: Vec<EdgeForm> = plan
        .arms
        .iter()
        .flatten()
        .filter_map(|arm| arm.edge)
        .map(|edge| edge.form)
        .collect();
    assert!(forms.contains(&EdgeForm::History));
    assert!(forms.contains(&EdgeForm::Constructor));
}
