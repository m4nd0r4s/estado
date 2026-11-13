//! Bead 4.2: analysis and rewrite of the functions of the `#[handlers]`
//! impls. Reads the role attributes, recognises the parameters by type,
//! scans the bodies for transition macros and rewrites them (ADR 0011).
//!
//! The result is [`MachineModel::analysed_handlers`], the contract with the
//! generators, and the rewritten impl blocks.

mod body;
mod body_names;
mod captures;
#[cfg(test)]
mod captures_tests;
mod checks;
mod event_coverage;
#[cfg(test)]
mod event_coverage_tests;
mod goto_resolve;
mod goto_syntax;
mod parallel_geometry;
mod parameters;
mod roles;
mod scopes;
mod signature;
#[cfg(test)]
mod tests;
mod unsupported_macros;

use std::collections::HashMap;

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{FnArg, Ident, ImplItem, ImplItemFn, ItemFn, Type};

use crate::model::{
    Diagnostic, Extent, HandlerModel, HandlerRole, MachineModel, StateId, StateTree,
};
use crate::naming::plain_name;
use crate::suggest::did_you_mean;
use body::{BodyEnvironment, BodyMode, rewrite_body};
use checks::{
    check_history_nesting, check_impl_target, check_parallel_support, check_root_initial,
    reject_roles_outside_functions,
};
use event_coverage::event_coverage;
use parameters::{ParameterEnvironment, ParameterList, classify_parameters};
use roles::{FunctionRole, RoleAttribute, take_role};
use signature::{EdgeType, rewrite_signature};
use unsupported_macros::reject_unsupported_macros;

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
        if !check_impl_target(model, &degraded_impl, &mut diagnostics) {
            // Its functions are not the machine's: keep none of them, so
            // rustc has nothing to add to the error.
            degraded_impl.items.clear();
            degraded_impls.push(degraded_impl.clone());
            full_impls.push(degraded_impl);
            continue;
        }
        let mut full_impl = degraded_impl.clone();
        for (degraded_item, full_item) in degraded_impl.items.iter_mut().zip(&mut full_impl.items) {
            if !matches!(degraded_item, ImplItem::Fn(_)) {
                reject_roles_outside_functions(degraded_item, &mut diagnostics);
                reject_roles_outside_functions(full_item, &mut Vec::new());
                continue;
            }
            let (ImplItem::Fn(degraded), ImplItem::Fn(full)) = (degraded_item, full_item) else {
                continue;
            };
            reject_unsupported_macros(degraded, &mut diagnostics);
            reject_unsupported_macros(full, &mut Vec::new());
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
    check_parallel_support(model, &analysed, &mut diagnostics);
    model.handlers = degraded_impls;
    model.rewritten_handlers = full_impls;
    model.analysed_handlers = analysed;
    diagnostics
}

/// The edge enum name of a handler: `PlayingEdges`, `PlayingAlwaysEdges`.
fn edges_enum_name(tree: &StateTree, state: StateId, role: HandlerRole) -> Ident {
    let name = &tree.node(state).name;
    let suffix = match role {
        HandlerRole::Always => "AlwaysEdges",
        HandlerRole::OnDone => "DoneEdges",
        _ => "Edges",
    };
    Ident::new(&format!("{}{suffix}", plain_name(name)), name.span())
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

    let effect: Type = model.options.effect_type();
    let environment = ParameterEnvironment {
        tree,
        state,
        event: &model.options.event,
        role: attribute.role,
    };
    let parameters = classify_parameters(&environment, degraded, diagnostics);

    let coverage = event_coverage(&degraded.block, event_identifier(degraded, &parameters));
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
            borrowed_names: &parameters.borrowed_names,
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
            borrowed_names: &parameters.borrowed_names,
        },
    );

    let has_constructor = report
        .gotos
        .iter()
        .any(|goto| matches!(goto.form, crate::model::GotoForm::Constructor { .. }));
    let edge_type = if report.gotos.is_empty() {
        EdgeType::None
    } else if has_constructor && tree.region_of(state).is_some() {
        EdgeType::OpaqueRegion
    } else if has_constructor {
        EdgeType::Opaque
    } else {
        EdgeType::Named(syn::parse_quote!(__estado::#edges_enum))
    };
    rewrite_signature(&mut full, &parameters, &effect, &edge_type);
    rewrite_signature(degraded, &parameters, &effect, &EdgeType::None);
    // Every `goto!` of the degraded copy is `unreachable!()`.
    degraded
        .attrs
        .push(syn::parse_quote!(#[allow(unreachable_code)]));

    let returns_result = matches!(
        &full.sig.output,
        syn::ReturnType::Type(_, returned)
            if matches!(&**returned, Type::Path(path)
                if path.path.segments.last().is_some_and(|segment| segment.ident == "Result"))
    );
    if returns_result {
        diagnostics.push(
            Diagnostic::new(
                "E0061",
                Extent::of(&full.sig.output),
                "a handler returns `Result` but there is no `#[on_error]`",
            )
            .with_help(
                "`#[on_error]` is not supported yet by this version of `estado`: return \
                 `Next<Self>` and turn the error into a transition in the body",
            ),
        );
    }
    let handler_model = HandlerModel {
        role: attribute.role,
        state,
        parameters: parameters.parameters,
        function: to_item_fn(&full),
        edges_enum,
        gotos: report.gotos,
        returns_result,
        uses: report.uses,
        defer_calls: report.defer_calls,
        event_coverage: coverage,
    };
    Some((handler_model, full))
}

/// The name the function binds its event parameter to, if it has one.
fn event_identifier<'function>(
    function: &'function ImplItemFn,
    parameters: &ParameterList,
) -> Option<&'function Ident> {
    let FnArg::Typed(typed) = function.sig.inputs.iter().nth(parameters.event_index?)? else {
        return None;
    };
    match &*typed.pat {
        syn::Pat::Ident(binding) => Some(&binding.ident),
        _ => None,
    }
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
    let effect: Type = model.options.effect_type();
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
            borrowed_names: &[],
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
        borrowed_names: Vec::new(),
    };
    rewrite_signature(function, &parameters, &effect, &EdgeType::None);
    function
        .attrs
        .push(syn::parse_quote!(#[allow(unreachable_code)]));
}
