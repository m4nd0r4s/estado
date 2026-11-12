//! Parser of the per-state attributes of spec 6.1: `#[initial]`, `#[final]`,
//! `#[history]`, `#[after]`, `#[invoke]`, `#[unhandled]`.

use proc_macro2::Span;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, Ident, Meta, Path, Token};

use super::clean::marker_name;
use crate::model::{AfterSpec, Diagnostic, HistoryKind, InvokeSpec, UnhandledPolicy};
use crate::suggest::did_you_mean;

/// Code for a malformed state attribute (not in the spec table; errata 8).
const MALFORMED_ATTRIBUTE: &str = "E0017";

/// Where the attributes are written; some only make sense on one of them.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AttributeSite {
    /// On an enum variant.
    Variant,
    /// On the `#[state]` struct of a composite state.
    CompositeStruct,
}

/// `#[initial]` or `#[initial(with = path)]`.
pub struct InitialAttribute {
    /// The `with = path` constructor.
    pub with: Option<Path>,
}

/// Everything the attributes of one variant or struct say.
pub struct NodeAttributes {
    /// `#[initial]`.
    pub initial: Option<InitialAttribute>,
    /// `#[final]`.
    pub is_final: bool,
    /// `#[history]`.
    pub history: HistoryKind,
    /// `#[after(..)]`, in order.
    pub timers: Vec<AfterSpec>,
    /// `#[invoke(..)]`, in order.
    pub invokes: Vec<InvokeSpec>,
    /// `#[unhandled(..)]`.
    pub unhandled: Option<UnhandledPolicy>,
}

impl NodeAttributes {
    /// Merges attributes found on the struct of a composite into those of
    /// its variant (spec 6.1: "variant or struct").
    pub fn merge(&mut self, other: Self) {
        self.history = if other.history == HistoryKind::None {
            self.history
        } else {
            other.history
        };
        self.timers.extend(other.timers);
        self.invokes.extend(other.invokes);
        self.unhandled = other.unhandled.or(self.unhandled);
    }
}

fn malformed(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(MALFORMED_ATTRIBUTE, span, message)
}

/// Parses the attributes of one variant or struct; problems go to
/// `diagnostics` and the attribute is skipped.
pub fn parse_node_attributes(
    attributes: &[Attribute],
    site: AttributeSite,
    diagnostics: &mut Vec<Diagnostic>,
) -> NodeAttributes {
    let mut parsed = NodeAttributes {
        initial: None,
        is_final: false,
        history: HistoryKind::None,
        timers: Vec::new(),
        invokes: Vec::new(),
        unhandled: None,
    };
    for attribute in attributes {
        let Some(marker) = marker_name(attribute) else {
            continue;
        };
        let span = attribute.path().span();
        match (marker, site) {
            ("initial" | "final", AttributeSite::CompositeStruct) => diagnostics.push(
                malformed(
                    span,
                    format!(
                        "`{}` goes on the variant, not on the state struct",
                        marker_text(marker)
                    ),
                )
                .with_help("move it to the variant of the enum that holds this state"),
            ),
            ("history", AttributeSite::Variant) => diagnostics.push(
                malformed(
                    span,
                    "`#[history]` goes on the `#[state]` struct of the compound state",
                )
                .with_help("move it to the struct that has the `#[substates]` field"),
            ),
            ("initial", _) => parse_initial(attribute, &mut parsed, diagnostics),
            ("final", _) => match attribute.meta {
                Meta::Path(_) => parsed.is_final = true,
                _ => diagnostics.push(malformed(span, "`#[r#final]` takes no arguments")),
            },
            ("history", _) => parse_history(attribute, &mut parsed, diagnostics),
            ("after", _) => match attribute.parse_args_with(parse_after) {
                Ok(timer) => parsed.timers.push(timer),
                Err(error) => diagnostics.push(
                    malformed(error.span(), format!("invalid `#[after]`: {error}"))
                        .with_help("write `#[after(duration => event)]`"),
                ),
            },
            ("invoke", _) => match attribute.parse_args::<Expr>() {
                Ok(task) => parsed.invokes.push(InvokeSpec { task }),
                Err(error) => diagnostics.push(
                    malformed(error.span(), format!("invalid `#[invoke]`: {error}"))
                        .with_help("write `#[invoke(task)]` with an expression or a closure"),
                ),
            },
            ("unhandled", _) => parse_unhandled(attribute, &mut parsed, diagnostics),
            _ => {}
        }
    }
    parsed
}

/// How a marker is written; `final` needs the raw identifier form.
fn marker_text(marker: &str) -> String {
    if marker == "final" {
        "#[r#final]".to_owned()
    } else {
        format!("#[{marker}]")
    }
}

fn parse_after(input: ParseStream<'_>) -> syn::Result<AfterSpec> {
    let duration: Expr = input.parse()?;
    input.parse::<Token![=>]>()?;
    let event: Expr = input.parse()?;
    Ok(AfterSpec { duration, event })
}

fn parse_initial(
    attribute: &Attribute,
    parsed: &mut NodeAttributes,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut with = None;
    if let Meta::List(_) = attribute.meta {
        let result = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("with") {
                with = Some(meta.value()?.parse::<Path>()?);
                Ok(())
            } else {
                Err(meta.error("expected `with = path`"))
            }
        });
        if let Err(error) = result {
            diagnostics.push(
                malformed(error.span(), format!("invalid `#[initial]`: {error}"))
                    .with_help("write `#[initial]` or `#[initial(with = constructor_path)]`"),
            );
            return;
        }
    } else if !matches!(attribute.meta, Meta::Path(_)) {
        diagnostics.push(malformed(
            attribute.path().span(),
            "write `#[initial]` or `#[initial(with = path)]`",
        ));
        return;
    }
    parsed.initial = Some(InitialAttribute { with });
}

fn parse_history(
    attribute: &Attribute,
    parsed: &mut NodeAttributes,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Meta::Path(_) = attribute.meta {
        parsed.history = HistoryKind::Shallow;
        return;
    }
    let (mut deep, mut keep) = (false, false);
    let result = attribute.parse_nested_meta(|meta| {
        if meta.path.is_ident("deep") {
            deep = true;
        } else if meta.path.is_ident("keep") {
            keep = true;
        } else {
            return Err(meta.error("expected `deep` or `keep`"));
        }
        Ok(())
    });
    match result {
        Err(error) => diagnostics.push(
            malformed(error.span(), format!("invalid `#[history]`: {error}"))
                .with_help("write `#[history]` or `#[history(deep, keep)]`"),
        ),
        Ok(()) if deep && keep => parsed.history = HistoryKind::DeepKeep,
        Ok(()) => diagnostics.push(
            malformed(
                attribute.path().span(),
                "deep history needs both `deep` and `keep`",
            )
            .with_help("write `#[history(deep, keep)]`"),
        ),
    }
}

fn parse_unhandled(
    attribute: &Attribute,
    parsed: &mut NodeAttributes,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const POLICIES: [&str; 3] = ["ignore", "error", "panic"];
    let policy: Ident = match attribute.parse_args() {
        Ok(policy) => policy,
        Err(error) => {
            diagnostics.push(
                malformed(error.span(), format!("invalid `#[unhandled]`: {error}"))
                    .with_help("write `#[unhandled(ignore)]`, `error` or `panic`"),
            );
            return;
        }
    };
    parsed.unhandled = match policy.to_string().as_str() {
        "ignore" => Some(UnhandledPolicy::Ignore),
        "error" => Some(UnhandledPolicy::Error),
        "panic" => Some(UnhandledPolicy::Panic),
        other => {
            let help = did_you_mean(other, POLICIES).map_or_else(
                || "expected one of: ignore, error, panic".to_owned(),
                |suggestion| format!("did you mean `{suggestion}`?"),
            );
            diagnostics.push(
                malformed(
                    policy.span(),
                    format!("`{other}` is not an unhandled policy"),
                )
                .with_help(help),
            );
            None
        }
    };
}
