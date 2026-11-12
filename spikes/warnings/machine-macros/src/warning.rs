//! Warning codes and the `#[deprecated]` item technique.
//!
//! A warning is two pieces of generated code:
//! 1. a deprecated item (const, fn or struct) in a hidden module, whose
//!    `note` carries the text, and
//! 2. a reference to that item written with the span of the offending
//!    identifier. rustc reports `deprecated` at the span of the *use*.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{Ident, LitStr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WarningCode {
    UnreachableState,
}

impl WarningCode {
    pub fn from_ident(ident: &Ident) -> Option<Self> {
        match ident.to_string().as_str() {
            "W0001" => Some(Self::UnreachableState),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::UnreachableState => "W0001",
        }
    }
}

#[derive(Clone, Copy, Default)]
pub enum WarningStrategy {
    Const,
    #[default]
    Function,
    Struct,
}

impl WarningStrategy {
    pub fn from_name(name: &LitStr) -> syn::Result<Self> {
        match name.value().as_str() {
            "const" => Ok(Self::Const),
            "fn" => Ok(Self::Function),
            "struct" => Ok(Self::Struct),
            _ => Err(syn::Error::new(name.span(), "expected const, fn or struct")),
        }
    }
}

/// Which span the reference carries. Spike-only: shows why the variant span
/// is required.
#[derive(Clone, Copy, Default)]
pub enum SpanSource {
    #[default]
    Variant,
    CallSite,
    MixedSite,
}

impl SpanSource {
    pub fn from_name(name: &LitStr) -> syn::Result<Self> {
        match name.value().as_str() {
            "variant" => Ok(Self::Variant),
            "call_site" => Ok(Self::CallSite),
            "mixed_site" => Ok(Self::MixedSite),
            _ => Err(syn::Error::new(
                name.span(),
                "expected variant, call_site or mixed_site",
            )),
        }
    }

    pub fn resolve(self, variant_span: Span) -> Span {
        match self {
            Self::Variant => variant_span,
            Self::CallSite => Span::call_site(),
            Self::MixedSite => Span::mixed_site(),
        }
    }
}

/// One warning occurrence: code, message and the span of the problem.
pub struct Warning {
    pub code: WarningCode,
    /// Distinguishes occurrences of the same code, e.g. the state name.
    pub subject: String,
    pub message: String,
    pub span: Span,
}

/// Generated pieces: definitions go in the hidden module, uses go next to the
/// user's code.
pub struct WarningTokens {
    pub definition: TokenStream,
    pub usage: TokenStream,
}

pub fn emit(
    warning: &Warning,
    strategy: WarningStrategy,
    span_source: SpanSource,
    module: &Ident,
) -> WarningTokens {
    let label = warning.code.label();
    let note = format!("estado::{label}: {}", warning.message);
    // Readable, hash-free name: `W0001_Idle`.
    let item = format_ident!("{}_{}", label, warning.subject);
    // The item ident carries the user's span so the use site is precise.
    let span = span_source.resolve(warning.span);
    let item_at_span = Ident::new(&item.to_string(), span);

    let (definition, usage) = match strategy {
        WarningStrategy::Const => (
            quote! { #[deprecated(note = #note)] pub const #item: () = (); },
            quote_spanned! { span => let _ = #module::#item_at_span; },
        ),
        WarningStrategy::Function => (
            quote! { #[deprecated(note = #note)] pub const fn #item() {} },
            quote_spanned! { span => #module::#item_at_span(); },
        ),
        WarningStrategy::Struct => (
            quote! { #[deprecated(note = #note)] pub struct #item; },
            quote_spanned! { span => let _ = #module::#item_at_span; },
        ),
    };
    WarningTokens { definition, usage }
}
