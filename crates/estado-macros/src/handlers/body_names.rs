//! Names and hints the body rewrite derives from the tree and from match arms.

use std::collections::HashSet;

use syn::{Ident, Pat};

use crate::model::StateTree;
use crate::naming::plain_name;

/// The edge variant of a `goto!` to `target` given the variants the handler
/// already has: `ToFoo` for the first one, then `ToFoo2`, `ToFoo3`, ... A
/// numbered name never equals the plain name another state would get
/// (`ToFoo2` for a state called `Foo2`): that number is skipped (ADR 0013
/// item 10).
pub fn variant_name(tree: &StateTree, target: &Ident, used: &[String]) -> String {
    let plain = format!("To{}", plain_name(target));
    if !used.contains(&plain) {
        return plain;
    }
    let state_variants: HashSet<String> = tree
        .nodes
        .iter()
        .skip(1)
        .map(|node| format!("To{}", plain_name(&node.name)))
        .collect();
    (2..)
        .map(|occurrence| format!("{plain}{occurrence}"))
        .find(|candidate| !state_variants.contains(candidate) && !used.contains(candidate))
        .expect("an unused number exists")
}

fn path_text(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// The event a match arm is about, e.g. `Event::Play` for
/// `Event::Play(track)` (spec 8.1, `event_hint`).
pub fn pattern_hint(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::TupleStruct(inner) => Some(path_text(&inner.path)),
        Pat::Struct(inner) => Some(path_text(&inner.path)),
        Pat::Path(inner) => Some(path_text(&inner.path)),
        Pat::Reference(inner) => pattern_hint(&inner.pat),
        Pat::Paren(inner) => pattern_hint(&inner.pat),
        Pat::Or(inner) => {
            let parts: Option<Vec<String>> = inner.cases.iter().map(pattern_hint).collect();
            parts.map(|parts| parts.join(" | "))
        }
        _ => None,
    }
}
