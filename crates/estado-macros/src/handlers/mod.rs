//! Bead 4.2: analysis and rewrite of the functions of the `#[handlers]`
//! impls. Reads the role attributes, recognises the parameters by type,
//! scans the bodies for transition macros and rewrites them (ADR 0011).
//!
//! The result is [`MachineModel::analysed_handlers`], the contract with the
//! generators, and the rewritten impl blocks.

mod body;
mod goto_resolve;
mod goto_syntax;
mod parameters;
mod roles;
mod signature;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{FnArg, Ident, ImplItem, ImplItemFn, ItemFn, Type};

use crate::construct::initial_value;
use crate::model::{Diagnostic, HandlerModel, HandlerRole, MachineModel, StateId, StateTree};
use crate::suggest::did_you_mean;
use body::{BodyEnvironment, BodyMode, rewrite_body};
use goto_resolve::missing_data_error;
use parameters::{ParameterEnvironment, ParameterList, classify_parameters};
use roles::{FunctionRole, RoleAttribute, take_role};
use signature::{EdgeType, rewrite_signature};

/// Analyses every handler of `model`: fills `analysed_handlers` and
/// `rewritten_handlers`, and returns the diagnostics. `model.handlers` keeps
/// the degraded versions, used when the machine has errors.
pub fn analyse(model: &mut MachineModel) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen: HashMap<(HandlerRole, StateId), Span> = HashMap::new();
    let mut analysed = Vec::new();
    let mut degraded_impls = Vec::new();
    let mut full_impls = Vec::new();

    let impls = std::mem::take(&mut model.handlers);
    for mut degraded_impl in impls {
        let mut full_impl = degraded_impl.clone();
        for (degraded_item, full_item) in degraded_impl.items.iter_mut().zip(&mut full_impl.items) {
            let (ImplItem::Fn(degraded), ImplItem::Fn(full)) = (degraded_item, full_item) else {
                continue;
            };
            let role = take_role(degraded, &mut diagnostics);
            let _ = take_role(full, &mut Vec::new());
            let handler = match role {
                FunctionRole::Helper => continue,
                FunctionRole::Invalid => {
                    neutralise(model, degraded);
                    *full = degraded.clone();
                    continue;
                }
                FunctionRole::Handler(attribute) => attribute,
            };
            let outcome = analyse_function(model, &handler, degraded, &mut seen, &mut diagnostics);
            match outcome {
                Some((handler_model, full_function)) => {
                    *full = full_function;
                    analysed.push(handler_model);
                }
                None => {
                    neutralise(model, degraded);
                    *full = degraded.clone();
                }
            }
        }
        degraded_impls.push(degraded_impl);
        full_impls.push(full_impl);
    }
    check_root_initial(model, &mut diagnostics);
    check_history_nesting(model, &mut diagnostics);
    model.handlers = degraded_impls;
    model.rewritten_handlers = full_impls;
    model.analysed_handlers = analysed;
    diagnostics
}

/// `Spec::initial` builds the `#[initial]` chain of the root: every level on
/// it must be buildable without data, or carry `#[initial(with = ..)]` (E0020).
fn check_root_initial(model: &MachineModel, diagnostics: &mut Vec<Diagnostic>) {
    let tree = &model.tree;
    if let Err(missing) = initial_value(tree, 0) {
        let node = tree.node(missing.node);
        diagnostics.push(missing_data_error(
            tree,
            &missing,
            &node.name,
            node.name.span(),
        ));
    }
}

/// A deep history stores the whole substate value, so a second deep history
/// inside it would need the same value twice (errata 5 covers one level).
fn check_history_nesting(model: &MachineModel, diagnostics: &mut Vec<Diagnostic>) {
    use crate::model::HistoryKind;
    let tree = &model.tree;
    for node in tree.nodes.iter().skip(1) {
        if node.history != HistoryKind::DeepKeep {
            continue;
        }
        let outer = node.id;
        let nested = tree.nodes.iter().skip(1).find(|candidate| {
            candidate.id != outer
                && candidate.history == HistoryKind::DeepKeep
                && tree.is_ancestor_or_self(outer, candidate.id)
        });
        if let Some(inner) = nested {
            diagnostics.push(
                Diagnostic::new(
                    "E0090",
                    inner.name.span(),
                    format!(
                        "`{}` keeps deep history inside `{}`, which keeps deep history too",
                        inner.name, node.name
                    ),
                )
                .with_help("nested deep histories are not supported yet; use `#[history]` (shallow) for the inner one"),
            );
        }
    }
}

/// The edge enum name of a handler: `PlayingEdges`, `PlayingAlwaysEdges`.
fn edges_enum_name(tree: &StateTree, state: StateId, role: HandlerRole) -> Ident {
    let name = &tree.node(state).name;
    let suffix = match role {
        HandlerRole::Always => "AlwaysEdges",
        HandlerRole::OnDone => "DoneEdges",
        _ => "Edges",
    };
    Ident::new(&format!("{name}{suffix}"), name.span())
}

/// Analyses one handler. Returns its model and the rewritten function, or
/// `None` when the state is unknown (already reported).
fn analyse_function(
    model: &MachineModel,
    attribute: &RoleAttribute,
    degraded: &mut ImplItemFn,
    seen: &mut HashMap<(HandlerRole, StateId), Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(HandlerModel, ImplItemFn)> {
    let tree = &model.tree;
    let Some(state) = tree.find(&attribute.state) else {
        diagnostics.push(unknown_state(tree, &attribute.state));
        return None;
    };
    if let Some(previous) = seen.insert((attribute.role, state), attribute.state.span()) {
        diagnostics.push(
            Diagnostic::new(
                "E0005",
                attribute.state.span(),
                format!(
                    "`{}` already has a handler for this role",
                    attribute.state
                ),
            )
            .with_help("a state has at most one `#[on]`, `#[entry]`, `#[exit]`, `#[always]` and `#[on_done]`")
            .with_note(previous, "the first one"),
        );
    }
    if attribute.role == HandlerRole::OnDone
        && !matches!(
            tree.node(state).kind,
            crate::model::StateKind::Compound | crate::model::StateKind::Parallel
        )
    {
        diagnostics.push(
            Diagnostic::new(
                "E0044",
                attribute.state.span(),
                format!(
                    "`{}` has no substates, so it never completes",
                    attribute.state
                ),
            )
            .with_help("`#[on_done]` reacts to a final substate of a compound state"),
        );
    }
    if let Some(asyncness) = degraded.sig.asyncness {
        diagnostics.push(
            Diagnostic::new("E0060", asyncness.span(), "handlers cannot be `async`")
                .with_help("do asynchronous work with `#[invoke]` and let the result arrive as an event (spec 7)"),
        );
    }

    let effect: Type = model
        .options
        .effect
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(()));
    let environment = ParameterEnvironment {
        tree,
        state,
        event: &model.options.event,
        role: attribute.role,
    };
    let parameters = classify_parameters(&environment, degraded, diagnostics);

    let edges_enum = edges_enum_name(tree, state, attribute.role);
    let mut full = degraded.clone();
    let report = rewrite_body(
        &mut full.block,
        &BodyEnvironment {
            tree,
            state,
            role: attribute.role,
            edges_enum: &edges_enum,
            mode: BodyMode::Full,
        },
    );
    diagnostics.extend(report.diagnostics);
    rewrite_body(
        &mut degraded.block,
        &BodyEnvironment {
            tree,
            state,
            role: attribute.role,
            edges_enum: &edges_enum,
            mode: BodyMode::Degraded,
        },
    );

    let has_constructor = report
        .gotos
        .iter()
        .any(|goto| matches!(goto.form, crate::model::GotoForm::Constructor { .. }));
    let edge_type = if report.gotos.is_empty() {
        EdgeType::None
    } else if has_constructor {
        EdgeType::Opaque
    } else {
        EdgeType::Named(syn::parse_quote!(__estado::#edges_enum))
    };
    rewrite_signature(&mut full, &parameters, &effect, &edge_type);
    rewrite_signature(degraded, &parameters, &effect, &EdgeType::None);

    let returns_result = matches!(
        &full.sig.output,
        syn::ReturnType::Type(_, returned)
            if matches!(&**returned, Type::Path(path)
                if path.path.segments.last().is_some_and(|segment| segment.ident == "Result"))
    );
    let handler_model = HandlerModel {
        role: attribute.role,
        state,
        parameters: parameters.parameters,
        function: to_item_fn(&full),
        edges_enum,
        gotos: report.gotos,
        returns_result,
        uses: report.uses,
    };
    Some((handler_model, full))
}

fn to_item_fn(function: &ImplItemFn) -> ItemFn {
    ItemFn {
        attrs: function.attrs.clone(),
        vis: function.vis.clone(),
        sig: function.sig.clone(),
        block: Box::new(function.block.clone()),
    }
}

fn unknown_state(tree: &StateTree, name: &Ident) -> Diagnostic {
    let names: Vec<String> = tree
        .nodes
        .iter()
        .skip(1)
        .map(|node| node.name.to_string())
        .collect();
    let help = did_you_mean(&name.to_string(), names.iter().map(String::as_str)).map_or_else(
        || "the name must be a state declared in this machine".to_owned(),
        |suggestion| format!("did you mean `{suggestion}`?"),
    );
    Diagnostic::new(
        "E0004",
        name.span(),
        format!("`{name}` is not a state of this machine"),
    )
    .with_help(help)
}

/// Makes a function whose role could not be analysed compile on its own:
/// complete `Cx<Self>` and neutralise the transition macros, so rust-analyzer
/// keeps working and errors do not cascade.
fn neutralise(model: &MachineModel, function: &mut ImplItemFn) {
    let effect: Type = model
        .options
        .effect
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(()));
    let dummy = Ident::new("Neutralised", Span::call_site());
    let tree = &model.tree;
    rewrite_body(
        &mut function.block,
        &BodyEnvironment {
            tree,
            state: 0,
            role: HandlerRole::On,
            edges_enum: &dummy,
            mode: BodyMode::Degraded,
        },
    );
    let context_index = function
        .sig
        .inputs
        .iter()
        .position(|argument| match argument {
            FnArg::Typed(typed) => matches!(&*typed.ty, Type::Reference(reference)
            if matches!(&*reference.elem, Type::Path(path)
                if path.path.segments.last().is_some_and(|segment| segment.ident == "Cx"))),
            FnArg::Receiver(_) => false,
        });
    let parameters = ParameterList {
        parameters: Vec::new(),
        context_index,
        event_index: None,
    };
    rewrite_signature(function, &parameters, &effect, &EdgeType::None);
}
