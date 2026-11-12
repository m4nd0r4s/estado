//! Rewriting the signature of a handler (ADR 0011): a generic effect sink,
//! `Cx<'_, Self, Sink>`, a named `'event` lifetime for constructors and the
//! return type with its edge set.

use syn::visit_mut::{self, VisitMut};
use syn::{
    GenericArgument, GenericParam, ImplItemFn, Lifetime, PathArguments, ReturnType, Type, TypePath,
    parse_quote,
};

use super::parameters::ParameterList;

/// Name of the generic sink parameter added to every handler.
pub const SINK_PARAMETER: &str = "EffectSink";

/// How the return type changes.
pub enum EdgeType {
    /// No `goto!`: the return type stays `Next<Self>` (the `NoEdge` default).
    None,
    /// `goto!` without a constructor: the named edge enum.
    Named(syn::Path),
    /// A constructor `goto!`: `impl Edge<Self> + use<..>`.
    Opaque,
    /// A constructor `goto!` in a handler inside a region: the edges must
    /// also say whether they leave the parallel state.
    OpaqueRegion,
}

/// Rewrites `function` in place. `effect` is the effect type of the machine.
pub fn rewrite_signature(
    function: &mut ImplItemFn,
    parameters: &ParameterList,
    effect: &Type,
    edges: &EdgeType,
) {
    let sink = syn::Ident::new(SINK_PARAMETER, proc_macro2::Span::call_site());
    let name_event_lifetime = matches!(edges, EdgeType::Opaque | EdgeType::OpaqueRegion)
        && parameters.event_index.is_some();
    let generics = &mut function.sig.generics;
    if name_event_lifetime {
        let lifetime: Lifetime = parse_quote!('event);
        generics
            .params
            .insert(0, GenericParam::Lifetime(parse_quote!(#lifetime)));
    }
    generics
        .params
        .push(parse_quote!(#sink: ::estado_core::Sink<#effect>));

    for (index, argument) in function.sig.inputs.iter_mut().enumerate() {
        let syn::FnArg::Typed(typed) = argument else {
            continue;
        };
        if Some(index) == parameters.context_index {
            complete_context_type(&mut typed.ty, &sink);
        } else if Some(index) == parameters.event_index && name_event_lifetime {
            if let Type::Reference(reference) = &mut *typed.ty {
                reference.lifetime = Some(parse_quote!('event));
            }
        }
    }

    if let ReturnType::Type(_, return_type) = &mut function.sig.output {
        let edge_type: Option<Type> = match edges {
            EdgeType::None => None,
            EdgeType::Named(path) => Some(parse_quote!(#path)),
            EdgeType::Opaque if name_event_lifetime => Some(parse_quote!(
                impl ::estado_core::Edge<Self> + use<'event, #sink>
            )),
            EdgeType::Opaque => Some(parse_quote!(
                impl ::estado_core::Edge<Self> + use<#sink>
            )),
            EdgeType::OpaqueRegion if name_event_lifetime => Some(parse_quote!(
                impl __estado::RegionEdge + use<'event, #sink>
            )),
            EdgeType::OpaqueRegion => Some(parse_quote!(
                impl __estado::RegionEdge + use<#sink>
            )),
        };
        if let Some(edge_type) = edge_type {
            AddEdgeType { edge_type }.visit_type_mut(return_type);
        }
    }

    // A handler without `cx` does not use the sink parameter it is given.
    if parameters.context_index.is_none() {
        function
            .attrs
            .push(parse_quote!(#[allow(clippy::extra_unused_type_parameters)]));
    }
    if !function
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("inline"))
    {
        function.attrs.push(parse_quote!(#[inline]));
    }
}

/// `&mut Cx<Self>` becomes `&mut Cx<'_, Self, EffectSink>`.
fn complete_context_type(context_type: &mut Type, sink: &syn::Ident) {
    let Type::Reference(reference) = context_type else {
        return;
    };
    let Type::Path(TypePath { path, .. }) = &mut *reference.elem else {
        return;
    };
    let Some(segment) = path.segments.last_mut() else {
        return;
    };
    let PathArguments::AngleBracketed(arguments) = &mut segment.arguments else {
        return;
    };
    if arguments.args.len() == 1 {
        arguments
            .args
            .insert(0, GenericArgument::Lifetime(parse_quote!('_)));
        arguments.args.push(parse_quote!(#sink));
    }
}

/// `Next<Self>` becomes `Next<Self, Edges>`, also inside `Result<Next<..>, E>`.
struct AddEdgeType {
    edge_type: Type,
}

impl VisitMut for AddEdgeType {
    fn visit_type_path_mut(&mut self, node: &mut TypePath) {
        if let Some(segment) = node.path.segments.last_mut() {
            if segment.ident == "Next" {
                if let PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                    if arguments.args.len() == 1 {
                        arguments
                            .args
                            .push(GenericArgument::Type(self.edge_type.clone()));
                        return;
                    }
                }
            }
        }
        visit_mut::visit_type_path_mut(self, node);
    }
}
