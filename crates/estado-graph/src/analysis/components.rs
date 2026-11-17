//! Strongly connected components and minimum chain covers, the two graph
//! algorithms behind `edge_cover`.

use alloc::vec;
use alloc::vec::Vec;

/// The strongly connected components of the part of the graph reachable from
/// `roots` (Tarjan, without recursion). `successors[node]` lists the
/// neighbours of `node`.
///
/// A component comes after every component reachable from it, so the
/// result is in reverse topological order.
pub(crate) fn strongly_connected_components(
    successors: &[Vec<usize>],
    roots: &[usize],
) -> Vec<Vec<usize>> {
    let count = successors.len();
    let mut index = vec![usize::MAX; count];
    let mut low = vec![0; count];
    let mut on_stack = vec![false; count];
    let mut stack = Vec::new();
    let mut components = Vec::new();
    let mut counter = 0;
    for &root in roots {
        if index[root] != usize::MAX {
            continue;
        }
        index[root] = counter;
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;
        // Each frame is a node and the position of its next neighbour.
        let mut work = vec![(root, 0)];
        while let Some(&(node, next)) = work.last() {
            if let Some(&neighbour) = successors[node].get(next) {
                if let Some(frame) = work.last_mut() {
                    frame.1 += 1;
                }
                if index[neighbour] == usize::MAX {
                    index[neighbour] = counter;
                    low[neighbour] = counter;
                    counter += 1;
                    stack.push(neighbour);
                    on_stack[neighbour] = true;
                    work.push((neighbour, 0));
                } else if on_stack[neighbour] {
                    low[node] = low[node].min(index[neighbour]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                component.sort_unstable();
                components.push(component);
            }
        }
    }
    components
}

/// The fewest chains that together hold every node of a partial order,
/// given as its strict reachability: `reaches[a][b]` says `a` comes before
/// `b`. Chains may skip nodes, so a chain is a sequence in which each node
/// reaches the next (Dilworth: a maximum matching of the comparability
/// graph, Kuhn's algorithm).
///
/// `reaches` must be transitive and irreflexive.
pub(crate) fn minimum_chains(reaches: &[Vec<bool>]) -> Vec<Vec<usize>> {
    let count = reaches.len();
    let mut matched_to = vec![usize::MAX; count];
    for node in 0..count {
        let mut visited = vec![false; count];
        augment(node, reaches, &mut visited, &mut matched_to);
    }
    let mut next = vec![usize::MAX; count];
    for (node, &previous) in matched_to.iter().enumerate() {
        if previous != usize::MAX {
            next[previous] = node;
        }
    }
    let mut chains = Vec::new();
    for (start, &previous) in matched_to.iter().enumerate() {
        if previous != usize::MAX {
            continue;
        }
        let mut chain = vec![start];
        let mut current = start;
        while next[current] != usize::MAX {
            current = next[current];
            chain.push(current);
        }
        chains.push(chain);
    }
    chains
}

fn augment(
    node: usize,
    reaches: &[Vec<bool>],
    visited: &mut [bool],
    matched_to: &mut [usize],
) -> bool {
    for target in 0..reaches.len() {
        if !reaches[node][target] || visited[target] {
            continue;
        }
        visited[target] = true;
        if matched_to[target] == usize::MAX
            || augment(matched_to[target], reaches, visited, matched_to)
        {
            matched_to[target] = node;
            return true;
        }
    }
    false
}
