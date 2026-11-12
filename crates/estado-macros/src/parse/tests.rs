//! Unit tests of the tree builder.

use quote::quote;
use syn::{ItemMod, parse_quote};

use super::{Parsed, parse_machine};
use crate::model::{HistoryKind, StateKind, UnhandledPolicy};

fn parse(module: ItemMod) -> Parsed {
    let items = module.content.clone().expect("inline module").1;
    parse_machine(quote! { event = Ev }, &module, items)
}

fn names(parsed: &Parsed) -> Vec<String> {
    let model = parsed.model.as_ref().expect("a root");
    model
        .tree
        .nodes
        .iter()
        .map(|node| node.name.to_string())
        .collect()
}

fn media_player() -> ItemMod {
    parse_quote! {
        mod player {
            #[root]
            enum Player {
                #[initial] Idle,
                Active(Active),
                #[terminal] Failed { reason: String },
            }
            #[state]
            #[history(deep, keep)]
            #[after(Duration::from_secs(1) => Ev::Timeout)]
            struct Active { track: u32, #[substates] sub: ActiveSub }
            #[state]
            enum ActiveSub {
                #[initial] Playing { pos: u32 },
                #[unhandled(panic)] #[invoke(|s: &Paused| Cmd::Pause(s.pos))] Paused { pos: u32 },
            }
            #[handlers]
            impl Player {}
        }
    }
}

#[test]
fn builds_a_compound_tree_in_pre_order() {
    let parsed = parse(media_player());
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(
        names(&parsed),
        ["Player", "Idle", "Active", "Playing", "Paused", "Failed"]
    );
    let tree = &parsed.model.as_ref().expect("a root").tree;
    let active = tree.node(2);
    assert_eq!(active.kind, StateKind::Compound);
    assert_eq!(active.children, [3, 4]);
    assert_eq!(active.initial, Some(3));
    assert_eq!(active.history, HistoryKind::DeepKeep);
    assert_eq!(active.timers.len(), 1);
    assert_eq!(tree.node(3).depth, 1);
    assert_eq!(tree.node(4).unhandled, Some(UnhandledPolicy::Panic));
    assert_eq!(tree.node(4).invokes.len(), 1);
    assert!(tree.node(5).is_final);
    assert_eq!(tree.runtime_id(5), 4);
}

#[test]
fn builds_regions_as_nodes_named_after_the_fields() {
    let parsed = parse(parse_quote! {
        mod machine {
            #[root] enum Root { #[initial] Connected(Connected) }
            #[state] struct Connected { #[region] link: Link, #[region] keepalive: Keepalive }
            #[state] enum Link { #[initial] Open, Closed }
            #[state] enum Keepalive { #[initial] Idle, Pinging }
        }
    });
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(
        names(&parsed),
        [
            "Root",
            "Connected",
            "link",
            "Open",
            "Closed",
            "keepalive",
            "Idle",
            "Pinging"
        ]
    );
    let tree = &parsed.model.as_ref().expect("a root").tree;
    assert_eq!(tree.node(1).kind, StateKind::Parallel);
    assert_eq!(tree.node(2).kind, StateKind::Region);
    assert_eq!(tree.node(2).initial, Some(3));
    assert_eq!(tree.node(3).depth, 2);
}

#[test]
fn types_are_emitted_without_machine_attributes() {
    let parsed = parse(media_player());
    let model = parsed.model.expect("a root");
    let text = model
        .tree
        .types
        .iter()
        .map(|declaration| match declaration {
            crate::model::TypeDecl::Enum(item) => quote!(#item).to_string(),
            crate::model::TypeDecl::Struct(item) => quote!(#item).to_string(),
        })
        .collect::<String>();
    for marker in [
        "initial",
        "state",
        "root",
        "substates",
        "history",
        "after",
        "invoke",
        "unhandled",
        "terminal",
    ] {
        assert!(
            !text.contains(&format!("# [{marker}")),
            "`{marker}` left in {text}"
        );
    }
    assert!(model.handlers[0].attrs.is_empty());
}

#[test]
fn a_module_without_root_has_no_model() {
    let parsed = parse(parse_quote! { mod machine { #[state] enum Only { A } } });
    assert!(parsed.model.is_none());
    assert_eq!(parsed.diagnostics[0].code, "E0001");
    assert_eq!(parsed.fallback_items.len(), 1);
}

#[test]
fn structural_errors_get_their_codes() {
    let parsed = parse(parse_quote! {
        mod machine {
            #[root] enum Root { #[initial] Both(Both), One(One), Bad(Bad) }
            #[state] struct Both { #[substates] sub: Sub, #[region] region: Sub }
            #[state] struct One { #[region] only: Sub }
            #[state] struct Bad { #[substates] sub: u32 }
            #[state] enum Sub { #[initial] Leaf }
        }
    });
    let codes: Vec<_> = parsed
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(codes, ["E0013", "E0014", "E0015"]);
}
