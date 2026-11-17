//! `edge_cover`: the fewest paths from the initial state that take every
//! edge (spec 8.3).

use alloc::vec;
use alloc::vec::Vec;

use estado_core::Graph;

use super::components::{minimum_chains, strongly_connected_components};
use super::model::EdgeModel;
use super::path::Path;

/// See [`GraphAnalysis::edge_cover`].
///
/// [`GraphAnalysis::edge_cover`]: super::GraphAnalysis::edge_cover
pub fn edge_cover(graph: &Graph) -> Vec<Path> {
    let Some(initial) = graph.initial else {
        return Vec::new();
    };
    let model = EdgeModel::new(graph);
    let first_edges = model.enabled_by(&model.entered(initial.index()));
    if first_edges.is_empty() {
        return Vec::new();
    }

    // Edges that follow each other forever can all be taken by one walk, so
    // a strongly connected component is one unit. The units form a partial
    // order, and the fewest walks are the fewest chains of it.
    let successors: Vec<Vec<usize>> = (0..graph.edges.len())
        .map(|edge| model.successors(edge).to_vec())
        .collect();
    let components = strongly_connected_components(&successors, &first_edges);
    let mut component_of = vec![usize::MAX; graph.edges.len()];
    for (position, members) in components.iter().enumerate() {
        for &edge in members {
            component_of[edge] = position;
        }
    }
    // A component comes after the ones it leads to, so its closure can be
    // built from theirs.
    let mut reaches = vec![vec![false; components.len()]; components.len()];
    for (position, members) in components.iter().enumerate() {
        for &edge in members {
            for &next in model.successors(edge) {
                let target = component_of[next];
                if target != position {
                    let beyond = reaches[target].clone();
                    reaches[position][target] = true;
                    for (later, &reached) in beyond.iter().enumerate() {
                        if reached {
                            reaches[position][later] = true;
                        }
                    }
                }
            }
        }
    }

    let mut paths: Vec<Path> = minimum_chains(&reaches)
        .iter()
        .map(|chain| {
            let walk = walk_through(&model, &first_edges, &components, chain);
            Path::new(initial, walk)
        })
        .collect();
    paths.sort_by(|left, right| left.edges().cmp(right.edges()));
    paths
}

/// One walk from the initial state that takes every edge of the components
/// of `chain`, in order.
fn walk_through(
    model: &EdgeModel<'_>,
    first_edges: &[usize],
    components: &[Vec<usize>],
    chain: &[usize],
) -> Vec<usize> {
    let mut taken = vec![false; model.graph().edges.len()];
    let mut walk: Vec<usize> = Vec::new();
    for &component in chain {
        let members = &components[component];
        while members.iter().any(|&edge| !taken[edge]) {
            let sources = match walk.last() {
                Some(&last) => model.successors(last),
                None => first_edges,
            };
            let leg = model
                .search(sources, |edge| {
                    !taken[edge] && members.binary_search(&edge).is_ok()
                })
                .expect("the edges of a chain follow each other");
            for &edge in &leg {
                taken[edge] = true;
            }
            walk.extend(leg);
        }
    }
    walk
}
