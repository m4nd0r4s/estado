//! Parser of the `#[machine(..)]` attribute arguments (spec 6.1).

use std::collections::HashSet;

use proc_macro2::{Span, TokenStream};
use syn::meta::ParseNestedMeta;
use syn::parse::Parser;
use syn::{Expr, Ident, LitInt, Token, Type};

use crate::model::{DeferOverflow, Diagnostic, MachineOptions, UnhandledPolicy, UnsupportedOption};
use crate::suggest::did_you_mean;

/// Code for malformed machine options (not in the spec table; errata 8).
const INVALID_OPTION: &str = "E0016";

/// Every option name, for the "did you mean" help.
const OPTION_NAMES: [&str; 15] = [
    "event",
    "effect",
    "ctx",
    "unhandled",
    "defer_cap",
    "defer_overflow",
    "raise_cap",
    "fx_inline",
    "max_microsteps",
    "size_warn",
    "typestate",
    "infallible",
    "tracks_active",
    "version",
    "allow",
];

/// Parses the attribute arguments. Always returns options (defaults for
/// whatever is missing or wrong) so the rest of the expansion can go on.
pub fn parse_options(attribute: TokenStream) -> (MachineOptions, Vec<Diagnostic>) {
    let mut options = MachineOptions::new(Span::call_site());
    let mut diagnostics = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let parser = syn::meta::parser(|meta| {
        let name = option_name(&meta);
        if !seen.insert(name.clone()) {
            diagnostics.push(
                Diagnostic::new(
                    INVALID_OPTION,
                    meta.path.segments[0].ident.span(),
                    format!("option `{name}` is given twice"),
                )
                .with_help("remove the repeated option"),
            );
            skip_value(&meta)?;
            return Ok(());
        }
        parse_one_option(&meta, &name, &mut options, &mut diagnostics)
    });
    if let Err(error) = parser.parse2(attribute) {
        diagnostics.push(Diagnostic::new(
            INVALID_OPTION,
            error.span(),
            error.to_string(),
        ));
    }
    (options, diagnostics)
}

fn option_name(meta: &ParseNestedMeta<'_>) -> String {
    meta.path
        .get_ident()
        .map_or_else(|| "<path>".to_owned(), ToString::to_string)
}

fn parse_one_option(
    meta: &ParseNestedMeta<'_>,
    name: &str,
    options: &mut MachineOptions,
    diagnostics: &mut Vec<Diagnostic>,
) -> syn::Result<()> {
    match name {
        "event" => {
            options.event = meta.value()?.parse::<Type>()?;
            options.event_declared = true;
        }
        "effect" => options.effect = Some(meta.value()?.parse::<Type>()?),
        "ctx" => options.ctx = Some(meta.value()?.parse::<Type>()?),
        "unhandled" => {
            let value: Ident = meta.value()?.parse()?;
            match value.to_string().as_str() {
                "ignore" => options.unhandled = UnhandledPolicy::Ignore,
                "error" => options.unhandled = UnhandledPolicy::Error,
                "panic" => options.unhandled = UnhandledPolicy::Panic,
                other => diagnostics.push(bad_choice(
                    &value,
                    "unhandled",
                    other,
                    &["ignore", "error", "panic"],
                )),
            }
        }
        "defer_overflow" => {
            let value: Ident = meta.value()?.parse()?;
            match value.to_string().as_str() {
                "error" => options.defer_overflow = DeferOverflow::Error,
                "drop_oldest" => options.defer_overflow = DeferOverflow::DropOldest,
                other => diagnostics.push(bad_choice(
                    &value,
                    "defer_overflow",
                    other,
                    &["error", "drop_oldest"],
                )),
            }
        }
        "defer_cap" => {
            options.defer_cap = parse_number(meta, diagnostics, name)?.unwrap_or(options.defer_cap);
            options.defer_cap_given = true;
        }
        "raise_cap" => {
            options.raise_cap = parse_number(meta, diagnostics, name)?.unwrap_or(options.raise_cap);
            options.raise_cap_given = true;
        }
        "tracks_active" => {
            options.tracks_active =
                parse_flag(meta, diagnostics, name)?.unwrap_or(options.tracks_active)
        }
        "fx_inline" => {
            options.fx_inline = parse_number(meta, diagnostics, name)?.unwrap_or(options.fx_inline)
        }
        "size_warn" => {
            options.size_warn = parse_number(meta, diagnostics, name)?.unwrap_or(options.size_warn)
        }
        "max_microsteps" => {
            options.max_microsteps =
                parse_number(meta, diagnostics, name)?.unwrap_or(options.max_microsteps);
        }
        "version" => {
            if let Some(version) = parse_number(meta, diagnostics, name)? {
                options.version = version;
                note_unsupported(meta, "version", options);
            }
        }
        "typestate" => {
            if parse_flag(meta, diagnostics, name)?.is_some() {
                options.typestate = true;
                note_unsupported(meta, "typestate", options);
            }
        }
        "infallible" => {
            if parse_flag(meta, diagnostics, name)?.is_some() {
                options.infallible = true;
                note_unsupported(meta, "infallible", options);
            }
        }
        "allow" => meta.parse_nested_meta(|warning| {
            match warning.path.get_ident() {
                Some(code) => options.allowed_warnings.push(code.clone()),
                None => diagnostics.push(Diagnostic::new(
                    INVALID_OPTION,
                    warning.path.segments[0].ident.span(),
                    "expected a warning code such as `W0001`",
                )),
            }
            Ok(())
        })?,
        _ => {
            diagnostics.push(unknown_option(meta, name));
            skip_value(meta)?;
        }
    }
    Ok(())
}

/// Remembers an option that parses but is not supported yet; analysis
/// reports it (E0090).
fn note_unsupported(meta: &ParseNestedMeta<'_>, name: &'static str, options: &mut MachineOptions) {
    options.unsupported.push(UnsupportedOption {
        name,
        span: meta.path.segments[0].ident.span(),
    });
}

fn unknown_option(meta: &ParseNestedMeta<'_>, name: &str) -> Diagnostic {
    let span = meta.path.segments[0].ident.span();
    let help = match did_you_mean(name, OPTION_NAMES) {
        Some(suggestion) => format!("did you mean `{suggestion}`?"),
        None => format!("the options are: {}", OPTION_NAMES.join(", ")),
    };
    Diagnostic::new(INVALID_OPTION, span, format!("unknown option `{name}`")).with_help(help)
}

fn bad_choice(value: &Ident, option: &str, given: &str, choices: &[&str]) -> Diagnostic {
    let help = match did_you_mean(given, choices.iter().copied()) {
        Some(suggestion) => format!("did you mean `{suggestion}`?"),
        None => format!("expected one of: {}", choices.join(", ")),
    };
    Diagnostic::new(
        INVALID_OPTION,
        value.span(),
        format!("`{given}` is not a valid value of `{option}`"),
    )
    .with_help(help)
}

/// Parses `name = <integer>`; a value that does not fit is reported and
/// yields `None` so the default stays.
fn parse_number<Number>(
    meta: &ParseNestedMeta<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    name: &str,
) -> syn::Result<Option<Number>>
where
    Number: std::str::FromStr,
    Number::Err: std::fmt::Display,
{
    let literal: LitInt = meta.value()?.parse()?;
    match literal.base10_parse::<Number>() {
        Ok(number) => Ok(Some(number)),
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                INVALID_OPTION,
                literal.span(),
                format!("invalid value of `{name}`: {error}"),
            ));
            Ok(None)
        }
    }
}

/// A flag option must not have a value (`typestate`, not `typestate = true`).
fn parse_flag(
    meta: &ParseNestedMeta<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    name: &str,
) -> syn::Result<Option<bool>> {
    if meta.input.peek(Token![=]) || meta.input.peek(syn::token::Paren) {
        diagnostics.push(
            Diagnostic::new(
                INVALID_OPTION,
                meta.path.segments[0].ident.span(),
                format!("`{name}` is a flag and takes no value"),
            )
            .with_help(format!("write just `{name}`")),
        );
        skip_value(meta)?;
        return Ok(None);
    }
    Ok(Some(true))
}

/// Consumes the value of an option we do not use, so parsing can continue.
fn skip_value(meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        let _value: Expr = meta.value()?.parse()?;
    } else if meta.input.peek(syn::token::Paren) {
        meta.parse_nested_meta(|_| Ok(()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::*;

    #[test]
    fn parses_every_option() {
        let (options, diagnostics) = parse_options(quote! {
            event = Ev, effect = Cmd, ctx = Env, unhandled = panic, defer_cap = 3,
            defer_overflow = drop_oldest, raise_cap = 5, fx_inline = 2, max_microsteps = 10,
            size_warn = 64, typestate, infallible, tracks_active, version = 7, allow(W0001, W0004)
        });
        assert!(diagnostics.is_empty());
        assert!(options.event_declared);
        assert!(options.effect.is_some() && options.ctx.is_some());
        assert_eq!(options.unhandled, UnhandledPolicy::Panic);
        assert_eq!(options.defer_cap, 3);
        assert_eq!(options.defer_overflow, DeferOverflow::DropOldest);
        assert_eq!((options.raise_cap, options.fx_inline), (5, 2));
        assert_eq!(
            (options.max_microsteps, options.size_warn, options.version),
            (10, 64, 7)
        );
        assert!(options.typestate && options.infallible && options.tracks_active);
        assert!(options.raise_cap_given && options.defer_cap_given);
        assert_eq!(options.allowed_warnings.len(), 2);
    }

    #[test]
    fn defaults_when_empty() {
        let (options, diagnostics) = parse_options(TokenStream::new());
        assert!(diagnostics.is_empty());
        assert!(!options.event_declared);
        assert!(!options.tracks_active && !options.raise_cap_given && !options.defer_cap_given);
        assert_eq!(
            (options.defer_cap, options.raise_cap, options.fx_inline),
            (8, 8, 4)
        );
        assert_eq!(
            (options.max_microsteps, options.size_warn, options.version),
            (64, 128, 1)
        );
    }

    #[test]
    fn unknown_option_suggests_the_closest_name() {
        let (_, diagnostics) = parse_options(quote! { event = Ev, defer_capp = 2 });
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0016");
        assert_eq!(
            diagnostics[0].help.as_deref(),
            Some("did you mean `defer_cap`?")
        );
    }

    #[test]
    fn bad_values_and_repeats_are_reported_and_parsing_goes_on() {
        let (options, diagnostics) = parse_options(
            quote! { event = Ev, unhandled = ignor, event = Other, typestate = true, raise_cap = 4 },
        );
        assert_eq!(diagnostics.len(), 3);
        assert_eq!(options.raise_cap, 4);
    }
}
