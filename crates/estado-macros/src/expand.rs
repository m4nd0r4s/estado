//! Entry points of the macros: parse, validate, emit (spec 6.6: on errors
//! the best-effort module is still emitted, with `compile_error!` next to it).

use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Item, ItemMod};

use crate::analysis;
use crate::generate;
use crate::handlers;
use crate::model::{Diagnostic, MachineModel, TypeDecl};
use crate::parse;
use crate::warnings;

/// `#[machine(..)]`.
pub fn machine(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let module = match syn::parse2::<ItemMod>(item.clone()) {
        Ok(module) => module,
        Err(_) => return not_a_module(item),
    };
    let Some((_, module_items)) = module.content.clone() else {
        return without_body(&module);
    };

    let mut parsed = parse::parse_machine(attribute, &module, module_items);
    let mut diagnostics = parsed.diagnostics;
    let mut items = parsed.fallback_items;
    if let Some(model) = parsed.model.as_mut() {
        diagnostics.extend(analysis::resolve_initial_arguments(model));
        diagnostics.extend(analysis::validate(model));
        diagnostics.extend(handlers::analyse(model));
        items = machine_items(model, diagnostics.is_empty());
    }
    items.extend(
        diagnostics
            .iter()
            .map(|diagnostic| Item::Verbatim(diagnostic.to_compile_error())),
    );

    let mut emitted = module;
    if let Some((_, content)) = emitted.content.as_mut() {
        *content = items;
    }
    emitted.into_token_stream()
}

/// The items of a machine module: cleaned types, passthrough, handlers and
/// the generated code. With `valid == false` (the machine has errors) the
/// handlers keep their degraded form and only the state ids are generated.
fn machine_items(model: &MachineModel, valid: bool) -> Vec<Item> {
    let mut items: Vec<Item> = model
        .tree
        .types
        .iter()
        .map(|declaration| match declaration {
            TypeDecl::Enum(declaration) => Item::Enum(declaration.clone()),
            TypeDecl::Struct(declaration) => Item::Struct(declaration.clone()),
        })
        .collect();
    items.extend(model.passthrough.iter().cloned());
    let handlers = if valid {
        &model.rewritten_handlers
    } else {
        &model.handlers
    };
    items.extend(handlers.iter().cloned().map(Item::Impl));
    items.push(Item::Verbatim(generate::generated_items(model, valid)));
    items.push(Item::Verbatim(warnings::warning_items(model, valid)));
    items
}

/// `#[machine]` on something that is not a module: report and keep the item.
fn not_a_module(item: TokenStream) -> TokenStream {
    let span = syn::parse2::<Item>(item.clone())
        .map_or_else(|_| Span::call_site(), |parsed| item_name_span(&parsed));
    let error = Diagnostic::new(
        "E0001",
        span,
        "`#[machine]` must be applied to an inline module",
    )
    .with_help("wrap the machine in `mod name { .. }` and mark its enum with `#[root]`");
    let mut output = item;
    output.extend(error.to_compile_error());
    output
}

/// `#[machine] mod name;`: the machine must be declared inline (ADR 0010).
fn without_body(module: &ItemMod) -> TokenStream {
    let error = Diagnostic::new(
        "E0001",
        module.ident.span(),
        format!("module `{}` has no body", module.ident),
    )
    .with_help("the machine must be declared inline: `mod name { .. }`");
    let mut output = module.to_token_stream();
    output.extend(error.to_compile_error());
    output
}

fn item_name_span(item: &Item) -> Span {
    match item {
        Item::Enum(inner) => inner.ident.span(),
        Item::Struct(inner) => inner.ident.span(),
        Item::Fn(inner) => inner.sig.ident.span(),
        other => other.span(),
    }
}

/// `#[state]`, `#[root]`, `#[handlers]` outside a `#[machine]` module.
pub fn outside_machine(name: &str, item: TokenStream) -> TokenStream {
    let error = Diagnostic::new(
        "E0001",
        Span::call_site(),
        format!("`#[{name}]` is only valid inside a `#[machine]` module"),
    )
    .with_help("move the item into a module annotated with `#[machine(..)]`");
    let mut output = item;
    output.extend(error.to_compile_error());
    output
}
