//! The value of the child of the LCA that a `goto!` carries: built by the
//! handler, or, when it contains an `#[initial(with = f)]` call that needs
//! `cx`, by the edge (errata 15).
//!
//! In that second case the initializer must run when the state is entered, after the exits of the
//! transition, with the fields of its parent, so the handler cannot build the
//! value. It evaluates the data the user wrote (in the user's scope, where
//! the user's expressions belong) and the edge carries those values. The
//! `apply` of the edge builds the value from them, with `cx` in scope.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Expr, FieldValue, Ident, Member, Token, Type};

use super::goto_resolve::{ResolvedPayload, malformed, missing_data_error};
use super::goto_syntax::LevelSyntax;
use crate::construct::{MissingData, SuppliedData, initial_value, variant_value};
use crate::construct_context::{composite_value_in, initial_value_in, needs_context, part_ident};
use crate::generate::naming::fields_binding;
use crate::model::{Diagnostic, Extent, Payload, StateId, StateKind, StateTree, TransitionKind};

/// The value of the child of the LCA, from the data of the path (E0020).
pub fn build_value(
    tree: &StateTree,
    lca: StateId,
    target: StateId,
    chain: &[StateId],
    levels: Vec<LevelSyntax>,
    macro_span: Extent,
) -> Result<Expr, Diagnostic> {
    let target_name = tree.node(target).name.clone();
    // The levels that are built: below the LCA, down to the target. When the
    // target is the LCA itself (a local transition to an ancestor) nothing is
    // built but the initial of its substates.
    let entered = entered_levels(tree, lca, target);
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

/// The levels a transition builds: below the LCA, down to the target, outermost
/// first. Empty for a local transition to an ancestor, which only builds the
/// initial of that ancestor's substates.
pub fn entered_levels(tree: &StateTree, lca: StateId, target: StateId) -> Vec<StateId> {
    let mut entered: Vec<StateId> = tree
        .ancestors(target)
        .into_iter()
        .take_while(|&node| node != lca)
        .collect();
    entered.reverse();
    entered
}

/// Whether the value of this transition calls an initializer that needs `cx`.
pub fn needs_context_path(tree: &StateTree, lca: StateId, target: StateId) -> bool {
    let entered = entered_levels(tree, lca, target);
    if tree.node(target).kind == StateKind::Compound && needs_context(tree, target) {
        return true;
    }
    entered.iter().any(|&node| {
        tree.node(node).kind == StateKind::Parallel
            && tree
                .node(node)
                .children
                .iter()
                .any(|&region| !entered.contains(&region) && needs_context(tree, region))
    })
}

/// The values the edge carries.
#[derive(Default)]
struct Parts {
    expressions: Vec<Expr>,
    types: Vec<Type>,
}

impl Parts {
    /// Takes `expression` out of the value and returns the binding that
    /// stands for it there.
    fn take(&mut self, expression: Expr, ty: Type) -> Expr {
        let binding = part_ident(self.expressions.len());
        self.expressions.push(expression);
        self.types.push(ty);
        syn::parse_quote!(#binding)
    }
}

/// The edge payload of a `goto!` whose value needs `cx`.
pub fn deferred_payload(
    tree: &StateTree,
    lca: StateId,
    target: StateId,
    chain: &[StateId],
    levels: Vec<LevelSyntax>,
    kind: TransitionKind,
    macro_span: Extent,
) -> Result<ResolvedPayload, Diagnostic> {
    let target_name = &tree.node(target).name;
    if kind == TransitionKind::History {
        return Err(Diagnostic::new(
            "E0090",
            macro_span,
            format!(
                "`goto!({target_name}, history)` enters a state whose `#[initial(with = ..)]` \
                 function takes a `Cx` or the parent fields"
            ),
        )
        .with_help(
            "the initializer would run even when a history is recorded and replaces it; \
             use a function without parameters there, or enter the state without `history`",
        ));
    }
    let entered = entered_levels(tree, lca, target);
    let mut parts = Parts::default();
    let mut supplied: Vec<(StateId, SuppliedData)> = Vec::new();
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
        supplied.push((id, take_parts(tree, id, level.data, &mut parts)?));
    }
    let to_error =
        |missing: MissingData| missing_data_error(tree, &missing, target_name, macro_span);
    let build = build_in_apply(tree, target, &entered, supplied).map_err(to_error)?;
    Ok(ResolvedPayload::Deferred {
        arguments: parts.expressions,
        types: parts.types,
        build,
    })
}

/// The value of the child of the LCA, over the part bindings, as `apply`
/// evaluates it.
fn build_in_apply(
    tree: &StateTree,
    target: StateId,
    entered: &[StateId],
    mut supplied: Vec<(StateId, SuppliedData)>,
) -> Result<TokenStream, MissingData> {
    if entered.is_empty() {
        // A local transition to an ancestor: its substates restart from the
        // initial and the ancestor keeps its own fields, which the walk of
        // `apply` has bound.
        let fields = fields_binding(tree, target);
        return initial_value_in(tree, target, Some(quote! { #fields.reborrow() }));
    }
    let mut below: Option<TokenStream> = None;
    let mut chosen_regions: Vec<(StateId, TokenStream)> = Vec::new();
    for &node in entered.iter().rev() {
        if tree.node(node).kind == StateKind::Region {
            let value = below.take().expect("a region holds the state below it");
            chosen_regions.push((node, value));
            continue;
        }
        let data = supplied
            .iter()
            .position(|(id, _)| *id == node)
            .map(|index| supplied.remove(index).1)
            .unwrap_or(SuppliedData::None);
        let value = match tree.node(node).kind {
            StateKind::Compound | StateKind::Parallel => {
                composite_value_in(tree, node, &data, below.take(), &chosen_regions, false)?
            }
            _ => variant_value(tree, node, &data, below.take(), &chosen_regions, false)?,
        };
        chosen_regions.clear();
        below = Some(value);
    }
    Ok(below.expect("at least one level"))
}

/// Moves the expressions the user wrote for one level into `parts`.
fn take_parts(
    tree: &StateTree,
    node: StateId,
    data: SuppliedData,
    parts: &mut Parts,
) -> Result<SuppliedData, Diagnostic> {
    let state = tree.node(node);
    match data {
        SuppliedData::None => Ok(SuppliedData::None),
        SuppliedData::Tuple(arguments) => {
            let types: Vec<Type> = match &state.payload {
                Payload::Newtype(ty) => vec![ty.clone()],
                Payload::Tuple(fields) => fields.iter().map(|field| field.ty.clone()).collect(),
                _ => Vec::new(),
            };
            if types.len() != arguments.len() {
                // The wrong number of values: `variant_value` reports it.
                return Ok(SuppliedData::Tuple(arguments));
            }
            Ok(SuppliedData::Tuple(
                arguments
                    .into_iter()
                    .zip(types)
                    .map(|(expression, ty)| parts.take(expression, ty))
                    .collect(),
            ))
        }
        SuppliedData::Fields { values, base } => {
            let declared: Vec<(&Ident, &Type)> =
                if state.composite_struct.is_some() && state.kind != StateKind::Atomic {
                    tree.shared_fields(node)
                        .into_iter()
                        .filter_map(|field| field.ident.as_ref().map(|name| (name, &field.ty)))
                        .collect()
                } else if let Payload::Struct(fields) = &state.payload {
                    fields
                        .iter()
                        .filter_map(|field| field.ident.as_ref().map(|name| (name, &field.ty)))
                        .collect()
                } else {
                    Vec::new()
                };
            let mut moved: Vec<FieldValue> = Vec::with_capacity(values.len());
            for mut value in values {
                let Member::Named(name) = &value.member else {
                    return Err(malformed(
                        Extent::of(&value),
                        "a field of a `goto!` is written by name",
                    ));
                };
                let Some((_, ty)) = declared.iter().find(|(declared, _)| *declared == name) else {
                    return Err(malformed(
                        name.span(),
                        format!("`{name}` is not a field of `{}`", state.name),
                    ));
                };
                value.expr = parts.take(value.expr, (*ty).clone());
                value.colon_token = Some(<Token![:]>::default());
                moved.push(value);
            }
            let base = base.map(|base| {
                let ty: Type = match &state.composite_struct {
                    Some(name) => syn::parse_quote!(#name),
                    None => {
                        let owner = state.owner_enum.as_ref().expect("owner enum");
                        syn::parse_quote!(#owner)
                    }
                };
                Box::new(parts.take(*base, ty))
            });
            Ok(SuppliedData::Fields {
                values: moved,
                base,
            })
        }
    }
}
