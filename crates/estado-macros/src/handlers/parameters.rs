//! Recognising handler parameters by type, in the order of spec 6.2 and
//! ADR 0011: own data, ancestor `*Fields` (nearest first), `&mut Cx<Self>`,
//! `&Event`. Anything else is E0006.

use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{FnArg, Ident, ImplItemFn, Type};

use crate::model::{
    Diagnostic, HandlerParameter, HandlerRole, Payload, StateId, StateKind, StateTree,
};

/// The parameters of one function.
pub struct ParameterList {
    /// What each parameter is, in order.
    pub parameters: Vec<HandlerParameter>,
    /// Index of the `Cx` parameter, if any.
    pub context_index: Option<usize>,
    /// Index of the event parameter, if any.
    pub event_index: Option<usize>,
}

/// What the recognition needs to know about the function.
pub struct ParameterEnvironment<'model> {
    /// The tree.
    pub tree: &'model StateTree,
    /// The state the function belongs to.
    pub state: StateId,
    /// The event type of the machine.
    pub event: &'model Type,
    /// The role of the function.
    pub role: HandlerRole,
}

/// Order of the parameter kinds; a parameter may not come before a kind of a
/// lower rank.
const RANK_OWN: u8 = 0;
const RANK_ANCESTOR: u8 = 1;
const RANK_OUTPUT: u8 = 2;
const RANK_CONTEXT: u8 = 3;
const RANK_EVENT: u8 = 4;

/// A recognised parameter before the order check.
struct Recognised {
    parameter: HandlerParameter,
    rank: u8,
    /// For ancestors: distance from the state (1 = parent).
    distance: usize,
}

/// Classifies every parameter of `function`. Errors are pushed to
/// `diagnostics`; the list then holds what could be recognised.
pub fn classify_parameters(
    environment: &ParameterEnvironment<'_>,
    function: &ImplItemFn,
    diagnostics: &mut Vec<Diagnostic>,
) -> ParameterList {
    let mut list = ParameterList {
        parameters: Vec::new(),
        context_index: None,
        event_index: None,
    };
    let mut last_key: Option<(u8, usize)> = None;
    for (index, argument) in function.sig.inputs.iter().enumerate() {
        let FnArg::Typed(typed) = argument else {
            diagnostics.push(
                Diagnostic::new(
                    "E0006",
                    argument.span(),
                    "handlers are associated functions without a `self` receiver",
                )
                .with_help("take the data of the state as a parameter, see spec 6.2"),
            );
            continue;
        };
        let recognised = match recognise(environment, &typed.ty) {
            Ok(recognised) => recognised,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        let key = (recognised.rank, recognised.distance);
        let out_of_order = last_key.is_some_and(|last| key <= last);
        if out_of_order {
            diagnostics.push(
                Diagnostic::new(
                    "E0006",
                    typed.ty.span(),
                    "parameter is out of order or repeated",
                )
                .with_help(
                    "parameters come in this order, all optional: own data, \
                     ancestor `*Fields` (nearest first), `cx`, event",
                ),
            );
            // Still locate it, so the signature rewrite completes `Cx<Self>`.
            match recognised.parameter {
                HandlerParameter::Context => list.context_index = Some(index),
                HandlerParameter::Event => list.event_index = Some(index),
                _ => {}
            }
            continue;
        }
        last_key = Some(key);
        match recognised.parameter {
            HandlerParameter::Context => list.context_index = Some(index),
            HandlerParameter::Event => list.event_index = Some(index),
            _ => {}
        }
        list.parameters.push(recognised.parameter);
    }
    list
}

fn error(
    span: proc_macro2::Span,
    message: impl Into<String>,
    help: impl Into<String>,
) -> Diagnostic {
    Diagnostic::new("E0006", span, message).with_help(help)
}

fn recognise(
    environment: &ParameterEnvironment<'_>,
    parameter_type: &Type,
) -> Result<Recognised, Diagnostic> {
    match parameter_type {
        Type::Reference(reference) => recognise_reference(
            environment,
            parameter_type,
            &reference.elem,
            reference.mutability.is_some(),
        ),
        Type::Path(path) if path.qself.is_none() => recognise_view(environment, parameter_type),
        other => Err(unrecognised(other)),
    }
}

fn unrecognised(parameter_type: &Type) -> Diagnostic {
    error(
        parameter_type.span(),
        format!(
            "parameter type `{}` is not recognised",
            parameter_type.to_token_stream()
        ),
        "a handler takes, in this order and all optional: the data of its state, \
         the `*Fields` of its ancestors, `&mut Cx<Self>` and `&Event` (spec 6.2)",
    )
}

fn last_segment(parameter_type: &Type) -> Option<&Ident> {
    match parameter_type {
        Type::Path(path) => path.path.segments.last().map(|segment| &segment.ident),
        _ => None,
    }
}

fn same_type(first: &Type, second: &Type) -> bool {
    first.to_token_stream().to_string() == second.to_token_stream().to_string()
        || last_segment(first).is_some_and(|name| Some(name) == last_segment(second))
}

fn recognise_reference(
    environment: &ParameterEnvironment<'_>,
    whole: &Type,
    inner: &Type,
    mutable: bool,
) -> Result<Recognised, Diagnostic> {
    let tree = environment.tree;
    let state = tree.node(environment.state);
    if last_segment(inner).is_some_and(|name| name == "Cx") {
        if !mutable {
            return Err(error(
                whole.span(),
                "the context is taken as `&mut Cx<Self>`",
                "write `cx: &mut Cx<Self>`",
            ));
        }
        return Ok(Recognised {
            parameter: HandlerParameter::Context,
            rank: RANK_CONTEXT,
            distance: 0,
        });
    }
    if same_type(inner, environment.event) {
        if mutable {
            return Err(error(
                whole.span(),
                "the event is taken by shared reference",
                "write `event: &Event`",
            ));
        }
        if !matches!(environment.role, HandlerRole::On) {
            return Err(error(
                whole.span(),
                "only `#[on]` handlers receive the event",
                "entries, exits, `#[always]` and `#[on_done]` run without an event of their own",
            ));
        }
        return Ok(Recognised {
            parameter: HandlerParameter::Event,
            rank: RANK_EVENT,
            distance: 0,
        });
    }
    // `&Output` of `#[on_done]`: the payload of a final substate.
    if environment.role == HandlerRole::OnDone && !mutable {
        let is_output = state.children.iter().any(|&child| {
            let child = tree.node(child);
            child.is_final
                && matches!(&child.payload, Payload::Newtype(output) if same_type(inner, output))
        });
        if is_output {
            return Ok(Recognised {
                parameter: HandlerParameter::Output,
                rank: RANK_OUTPUT,
                distance: 0,
            });
        }
    }
    // Own data of a newtype variant: `&mut T` / `&T`.
    if let Payload::Newtype(payload_type) = &state.payload {
        if state.kind != StateKind::Compound && same_type(inner, payload_type) {
            return Ok(Recognised {
                parameter: HandlerParameter::OwnData { mutable },
                rank: RANK_OWN,
                distance: 0,
            });
        }
    }
    if let Some(name) = last_segment(inner) {
        if let Some(owner) = tree.nodes.iter().skip(1).find(|node| node.name == *name) {
            return Err(error(
                whole.span(),
                format!("`{name}` is a state, not a parameter type"),
                own_data_help(tree, owner.id),
            ));
        }
        if let Some(owner) = tree
            .nodes
            .iter()
            .skip(1)
            .find(|node| node.composite_struct.as_ref() == Some(name))
        {
            return Err(error(
                whole.span(),
                format!(
                    "a handler never receives `&mut {name}`: its substates are borrowed separately"
                ),
                format!(
                    "take `{name}Fields<'_>` instead; to change the substate, use `goto!` ({})",
                    owner.name
                ),
            ));
        }
    }
    Err(unrecognised(whole))
}

fn own_data_help(tree: &StateTree, id: StateId) -> String {
    let node = tree.node(id);
    match &node.payload {
        Payload::Struct(_) | Payload::Tuple(_) => format!(
            "`{0}` has fields in its variant: take the view `{0}Mut<'_>`",
            node.name
        ),
        Payload::Unit => format!("`{}` has no data to take", node.name),
        _ => "take the payload of the variant by reference".to_owned(),
    }
}

/// `<Struct>Fields<'_>` and `<Variant>Mut<'_>`.
fn recognise_view(
    environment: &ParameterEnvironment<'_>,
    parameter_type: &Type,
) -> Result<Recognised, Diagnostic> {
    let tree = environment.tree;
    let Some(name) = last_segment(parameter_type) else {
        return Err(unrecognised(parameter_type));
    };
    let written = name.to_string();

    if let Some(struct_name) = written.strip_suffix("Fields") {
        let owner = tree.nodes.iter().skip(1).find(|node| {
            node.composite_struct
                .as_ref()
                .is_some_and(|declared| declared == struct_name)
        });
        let Some(owner) = owner else {
            return Err(unrecognised(parameter_type));
        };
        if owner.id == environment.state {
            return Ok(Recognised {
                parameter: HandlerParameter::Fields { ancestor: owner.id },
                rank: RANK_OWN,
                distance: 0,
            });
        }
        let distance = tree
            .ancestors(environment.state)
            .iter()
            .skip(1)
            .position(|&ancestor| ancestor == owner.id);
        return match distance {
            Some(index) => Ok(Recognised {
                parameter: HandlerParameter::Fields { ancestor: owner.id },
                rank: RANK_ANCESTOR,
                distance: index + 1,
            }),
            None => Err(error(
                parameter_type.span(),
                format!(
                    "`{written}` is the view of `{}`, which is not an ancestor of `{}`",
                    owner.name,
                    tree.node(environment.state).name
                ),
                "only the `*Fields` of the state itself and of its ancestors can be taken",
            )),
        };
    }

    if let Some(variant) = written.strip_suffix("Mut") {
        let owner = tree.nodes.iter().skip(1).find(|node| {
            node.name == variant && matches!(node.payload, Payload::Struct(_) | Payload::Tuple(_))
        });
        if let Some(owner) = owner {
            if owner.id != environment.state {
                return Err(error(
                    parameter_type.span(),
                    format!(
                        "`{written}` is the view of `{variant}`, but this function belongs to `{}`",
                        tree.node(environment.state).name
                    ),
                    "a handler takes the view of its own state only",
                ));
            }
            return Ok(Recognised {
                parameter: HandlerParameter::OwnView,
                rank: RANK_OWN,
                distance: 0,
            });
        }
    }
    Err(unrecognised(parameter_type))
}
