//! Resolving a parsed `goto!` against the tree: the target, the LCA of spec
//! table 5.2, the value of the child of the LCA and the E0003, E0020 and
//! E0043 diagnostics.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Expr, ExprClosure, Ident, Pat, PatType, Type};

use super::goto_syntax::{GotoExtra, GotoSyntax, LevelSyntax, MALFORMED_TRANSITION};
use crate::construct::{MissingData, SuppliedData, initial_value, variant_value};
use crate::model::{
    Diagnostic, HistoryKind, Payload, StateId, StateKind, StateTree, TransitionKind,
};
use crate::suggest::did_you_mean;

/// What the edge variant carries.
pub enum ResolvedPayload {
    /// The value of the child of the LCA (simple form).
    Value(Box<Expr>),
    /// The constructor closure, its parameter annotated with `Exiting![X]`
    /// resolved to a type.
    Constructor {
        closure: Box<ExprClosure>,
        exiting: StateId,
    },
}

/// A `goto!` resolved against the tree.
pub struct ResolvedGoto {
    /// The target.
    pub target: StateId,
    /// The LCA (0 = root).
    pub lca: StateId,
    /// The flavour.
    pub kind: TransitionKind,
    /// What the edge carries.
    pub payload: ResolvedPayload,
}

/// Resolves `syntax` for a handler of `handler_state`. `exiting_type` maps
/// the state named in `Exiting![X]` to the type of the value the constructor
/// receives.
pub fn resolve_goto(
    tree: &StateTree,
    handler_state: StateId,
    syntax: GotoSyntax,
    macro_span: Span,
    exiting_type: &dyn Fn(StateId) -> TokenStream,
) -> Result<ResolvedGoto, Diagnostic> {
    let chain = resolve_names(tree, &syntax.levels)?;
    let target = *chain.last().expect("at least one level");
    if tree.node(target).kind == StateKind::Region {
        return Err(malformed(
            syntax.levels[0].name.span(),
            format!(
                "`{}` is a region, not a state to go to",
                tree.node(target).name
            ),
        ));
    }
    let (kind, lca) = geometry(tree, handler_state, target, &syntax)?;
    super::parallel_geometry::check_parallel_geometry(
        tree,
        handler_state,
        target,
        lca,
        macro_span,
    )?;

    let payload = match syntax.extra {
        GotoExtra::Constructor(closure) => {
            let exiting = exiting_child(tree, handler_state, lca, target, macro_span)?;
            if let Some(deep) = deep_history_below(tree, exiting) {
                return Err(Diagnostic::new(
                    "E0090",
                    macro_span,
                    format!(
                        "a constructor receives `{}` by value and would drop the deep history of `{}`",
                        tree.node(exiting).name,
                        tree.node(deep).name
                    ),
                )
                .with_help("use the simple form, or move the history to `Exiting![..]` itself"));
            }
            constructor_payload(tree, closure, exiting, exiting_type)?
        }
        _ => ResolvedPayload::Value(Box::new(build_value(
            tree,
            lca,
            target,
            &chain,
            syntax.levels,
            macro_span,
        )?)),
    };
    Ok(ResolvedGoto {
        target,
        lca,
        kind,
        payload,
    })
}

fn malformed(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(MALFORMED_TRANSITION, span, message)
}

/// Names to ids (E0003), checking that each level is below the previous one.
fn resolve_names(tree: &StateTree, levels: &[LevelSyntax]) -> Result<Vec<StateId>, Diagnostic> {
    let mut chain: Vec<StateId> = Vec::new();
    for level in levels {
        let Some(id) = tree.find(&level.name) else {
            let candidates = tree.nodes.iter().skip(1).map(|node| node.name.to_string());
            let names: Vec<String> = candidates.collect();
            let help = did_you_mean(&level.name.to_string(), names.iter().map(String::as_str))
                .map_or_else(
                    || "the target must be a state declared in this machine".to_owned(),
                    |suggestion| format!("did you mean `{suggestion}`?"),
                );
            return Err(Diagnostic::new(
                "E0003",
                level.name.span(),
                format!("`{}` is not a state of this machine", level.name),
            )
            .with_help(help));
        };
        if let Some(&previous) = chain.last() {
            if !tree.ancestors(id).contains(&previous) || previous == id {
                return Err(malformed(
                    level.name.span(),
                    format!(
                        "`{}` is not below `{}` in the hierarchy",
                        level.name,
                        tree.node(previous).name
                    ),
                ));
            }
        }
        chain.push(id);
    }
    Ok(chain)
}

/// The kind and the LCA of the transition (spec 5.2).
fn geometry(
    tree: &StateTree,
    handler: StateId,
    target: StateId,
    syntax: &GotoSyntax,
) -> Result<(TransitionKind, StateId), Diagnostic> {
    let parent_of = |id: StateId| tree.node(id).parent.unwrap_or(0);
    let target_name = &tree.node(target).name;
    match &syntax.extra {
        GotoExtra::External(keyword) => {
            if !tree.is_ancestor_or_self(target, handler) {
                return Err(malformed(
                    keyword.span(),
                    format!(
                        "`external` needs a target that is the state itself or one of its \
                         ancestors, and `{target_name}` is neither"
                    ),
                ));
            }
            Ok((TransitionKind::External, parent_of(target)))
        }
        GotoExtra::History(keyword) => {
            let node = tree.node(target);
            if node.history == HistoryKind::None {
                return Err(malformed(
                    keyword.span(),
                    format!("`{target_name}` records no history"),
                )
                .with_help(format!(
                    "add `#[history]` or `#[history(deep, keep)]` to the struct of `{target_name}`"
                )));
            }
            if node.history == HistoryKind::Shallow
                && !node
                    .children
                    .iter()
                    .all(|&child| matches!(tree.node(child).payload, Payload::Unit))
            {
                return Err(Diagnostic::new(
                    "E0090",
                    keyword.span(),
                    format!(
                        "shallow history of `{target_name}` cannot rebuild substates that carry data"
                    ),
                )
                .with_help("use `#[history(deep, keep)]`, or make the substates unit variants"));
            }
            Ok((TransitionKind::History, normal_lca(tree, handler, target)))
        }
        _ => Ok((TransitionKind::Normal, normal_lca(tree, handler, target))),
    }
}

/// The LCA of a plain `goto!` (spec table 5.2).
fn normal_lca(tree: &StateTree, handler: StateId, target: StateId) -> StateId {
    let parent_of = |id: StateId| tree.node(id).parent.unwrap_or(0);
    if target == handler {
        parent_of(handler)
    } else if tree.is_ancestor_or_self(handler, target) {
        handler
    } else if tree.is_ancestor_or_self(target, handler) {
        target
    } else {
        tree.lca(handler, target)
    }
}

/// The child of `lca` on the path of `handler`: the state whose value a
/// constructor receives (`Exiting![X]`).
fn exiting_child(
    tree: &StateTree,
    handler: StateId,
    lca: StateId,
    target: StateId,
    macro_span: Span,
) -> Result<StateId, Diagnostic> {
    tree.ancestors(handler)
        .into_iter()
        .find(|&candidate| tree.node(candidate).parent.unwrap_or(0) == lca)
        .ok_or_else(|| {
            malformed(
                macro_span,
                format!(
                    "the constructor form needs to know which state exits: going into the \
                     substate `{}` of the handler's own state exits whichever substate is active",
                    tree.node(target).name
                ),
            )
            .with_help("use the simple form `goto!(Target { .. })` here")
        })
}

/// A `#[history(deep, keep)]` compound strictly below `node`, if any.
fn deep_history_below(tree: &StateTree, node: StateId) -> Option<StateId> {
    tree.node(node).children.iter().find_map(|&child| {
        if tree.node(child).history == HistoryKind::DeepKeep {
            Some(child)
        } else {
            deep_history_below(tree, child)
        }
    })
}

/// The closure of a constructor edge, its parameter typed with the owned
/// value of the exiting state (ADR 0001 rule 3).
fn constructor_payload(
    tree: &StateTree,
    mut closure: Box<ExprClosure>,
    exiting: StateId,
    exiting_type: &dyn Fn(StateId) -> TokenStream,
) -> Result<ResolvedPayload, Diagnostic> {
    if closure.inputs.len() != 1 {
        return Err(malformed(
            closure.span(),
            "a constructor takes exactly one parameter: the value of the exiting state",
        ));
    }
    let owned_type: Type = syn::parse2(exiting_type(exiting)).expect("a valid type");
    let parameter = closure.inputs.pop().expect("one parameter").into_value();
    let annotated = match parameter {
        Pat::Type(mut typed) => {
            if let Some(named) = exiting_macro_state(&typed.ty) {
                if named != tree.node(exiting).name {
                    return Err(malformed(
                        typed.ty.span(),
                        format!(
                            "this transition exits `{}`, not `{named}`",
                            tree.node(exiting).name
                        ),
                    )
                    .with_help(format!("write `Exiting![{}]`", tree.node(exiting).name)));
                }
                *typed.ty = owned_type;
            }
            Pat::Type(typed)
        }
        other => Pat::Type(PatType {
            attrs: Vec::new(),
            pat: Box::new(other),
            colon_token: Default::default(),
            ty: Box::new(owned_type),
        }),
    };
    closure.inputs.push(annotated);
    Ok(ResolvedPayload::Constructor { closure, exiting })
}

/// `X` of `Exiting![X]`.
fn exiting_macro_state(ty: &Type) -> Option<Ident> {
    let Type::Macro(type_macro) = ty else {
        return None;
    };
    if !type_macro.mac.path.is_ident("Exiting") {
        return None;
    }
    syn::parse2(type_macro.mac.tokens.clone()).ok()
}

/// The value of the child of the LCA, from the data of the path (E0020).
fn build_value(
    tree: &StateTree,
    lca: StateId,
    target: StateId,
    chain: &[StateId],
    levels: Vec<LevelSyntax>,
    macro_span: Span,
) -> Result<Expr, Diagnostic> {
    let target_name = tree.node(target).name.clone();
    // The levels that are built: below the LCA, down to the target. When the
    // target is the LCA itself (a local transition to an ancestor) nothing is
    // built but the initial of its substates.
    let mut entered: Vec<StateId> = tree
        .ancestors(target)
        .into_iter()
        .take_while(|&node| node != lca)
        .collect();
    entered.reverse();
    let mut supplied: Vec<(StateId, SuppliedData, Span)> = Vec::new();
    for (&id, level) in chain.iter().zip(levels) {
        let has_data = !matches!(level.data, SuppliedData::None);
        if !entered.contains(&id) {
            if has_data {
                return Err(malformed(
                    level.name.span(),
                    format!(
                        "`{}` does not exit or enter in this transition, so no data can be given for it",
                        level.name
                    ),
                ));
            }
            continue;
        }
        supplied.push((id, level.data, level.name.span()));
    }

    let missing_to_error =
        |missing: MissingData| missing_data_error(tree, &missing, &target_name, macro_span);
    let mut below = if tree.node(target).kind == StateKind::Compound {
        Some(initial_value(tree, target).map_err(missing_to_error)?)
    } else {
        None
    };
    if entered.is_empty() {
        return parse_value(below.expect("a local transition to an ancestor enters its initial"));
    }
    // The region a path goes into, with the value of its state.
    let mut chosen_regions: Vec<(StateId, TokenStream)> = Vec::new();
    for &node in entered.iter().rev() {
        if tree.node(node).kind == StateKind::Region {
            let value = below.take().expect("a region holds the state below it");
            chosen_regions.push((node, value));
            continue;
        }
        let data = supplied
            .iter()
            .position(|(id, _, _)| *id == node)
            .map(|index| supplied.remove(index).1)
            .unwrap_or(SuppliedData::None);
        let value = variant_value(tree, node, &data, below.take(), &chosen_regions, false)
            .map_err(missing_to_error)?;
        chosen_regions.clear();
        below = Some(value);
    }
    parse_value(below.expect("at least one level"))
}

fn parse_value(tokens: TokenStream) -> Result<Expr, Diagnostic> {
    syn::parse2(quote! { #tokens }).map_err(|error| malformed(error.span(), error.to_string()))
}

/// The E0020 diagnostic for data that a level needs and nobody supplied.
pub fn missing_data_error(
    tree: &StateTree,
    missing: &MissingData,
    target: &Ident,
    macro_span: Span,
) -> Diagnostic {
    let node = tree.node(missing.node);
    let list = missing
        .fields
        .iter()
        .map(|field| format!("`{field}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let (message, help) = if missing.through_initial {
        (
            format!(
                "`{}` is entered through its `#[initial]` and needs data for {list}",
                node.name
            ),
            format!(
                "add `#[initial(with = constructor)]` to `{}` so the macro can build it",
                node.name
            ),
        )
    } else {
        (
            format!(
                "`goto!({target})` needs data for `{}`: missing {list}",
                node.name
            ),
            format!(
                "write `{} {{ .. }}` with a value for each missing field",
                node.name
            ),
        )
    };
    Diagnostic::new("E0020", macro_span, message)
        .with_help(help)
        .with_note(
            node.name.span(),
            format!("`{}` is declared here", node.name),
        )
}
