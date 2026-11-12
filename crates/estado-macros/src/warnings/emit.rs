//! The warning technique of ADR 0003: a `#[deprecated]` function in a hidden
//! module, called from an anonymous `const _` with the span of the problem.
//!
//! Stable proc-macros cannot emit warnings, but rustc reports the use of a
//! deprecated item as a warning at the span of the use. The span MUST be a
//! user span (the offending identifier); a synthetic span points at the whole
//! attribute.
//!
//! A warning can be conditional on a value that only rustc knows, such as the
//! size of a type (W0004). A deprecation cannot depend on a `const` value
//! directly, but method *resolution* can: the function exists twice, in
//! `impl WarnIf<false>` (plain) and `impl WarnIf<true>` (deprecated),
//! and the call names `WarnIf::<{ condition }>`. rustc evaluates the const
//! argument while it selects the impl, so only a true condition resolves to
//! the deprecated function and warns.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::Ident;

use super::code::WarningCode;

/// Name of the hidden module that holds the deprecated functions.
const WARNING_MODULE: &str = "__estado_warnings";

/// One warning occurrence.
pub struct Warning {
    /// Which warning.
    pub code: WarningCode,
    /// Where it points: the span of the offending identifier.
    pub span: Span,
    /// Distinguishes occurrences of the same code; used in the item name,
    /// e.g. the state name gives `W0001_Orphan`.
    pub subject: String,
    /// The text after `estado::W0001: `; the `allow` hint is appended.
    pub message: String,
    /// When set, a boolean constant expression: the warning appears only
    /// when it evaluates to `true`. It is resolved in the machine module.
    pub condition: Option<TokenStream>,
}

/// The two generated pieces of one warning.
pub struct WarningTokens {
    /// Goes into the hidden warning module.
    pub definition: TokenStream,
    /// Goes into the anonymous `const _` block.
    pub usage: TokenStream,
}

/// Builds the deprecated function and its call (ADR 0003).
pub fn emit_warning(warning: &Warning) -> WarningTokens {
    let label = warning.code.label();
    let note = format!(
        "estado::{label}: {} (silence with `#[machine(allow({label}))]`)",
        warning.message
    );
    let module = Ident::new(WARNING_MODULE, Span::call_site());
    let function = format_ident!("{}_{}", label, warning.subject);
    // The ident carries the user's span so rustc reports the use there.
    let function_at_span = Ident::new(&function.to_string(), warning.span);
    let span = warning.span;

    match &warning.condition {
        None => WarningTokens {
            definition: quote! { #[deprecated(note = #note)] pub const fn #function() {} },
            usage: quote_spanned! { span => #module::#function_at_span(); },
        },
        Some(condition) => WarningTokens {
            definition: quote! {
                impl WarnIf<false> { pub const fn #function() {} }
                impl WarnIf<true> { #[deprecated(note = #note)] pub const fn #function() {} }
            },
            usage: quote_spanned! { span =>
                #module::WarnIf::<{ #condition }>::#function_at_span();
            },
        },
    }
}

/// Collects the warnings of a machine and renders them as items.
pub struct WarningSet<'options> {
    allowed: &'options [Ident],
    definitions: Vec<TokenStream>,
    usages: Vec<TokenStream>,
    has_conditional: bool,
}

impl<'options> WarningSet<'options> {
    /// A set that drops every warning whose code is in `allowed`
    /// (`#[machine(allow(..))]`, spec 6.7).
    pub fn new(allowed: &'options [Ident]) -> Self {
        Self {
            allowed,
            definitions: Vec::new(),
            usages: Vec::new(),
            has_conditional: false,
        }
    }

    /// Whether `#[machine(allow(..))]` silences `code`.
    pub fn is_allowed(&self, code: WarningCode) -> bool {
        self.allowed.iter().any(|ident| ident == code.label())
    }

    /// Adds a warning unless its code is allowed. Allowed warnings generate
    /// nothing at all, so they do not depend on lint levels.
    pub fn push(&mut self, warning: &Warning) {
        if self.is_allowed(warning.code) {
            return;
        }
        let tokens = emit_warning(warning);
        self.has_conditional |= warning.condition.is_some();
        self.definitions.push(tokens.definition);
        self.usages.push(tokens.usage);
    }

    /// The hidden module and the `const _` block; empty when nothing warns.
    pub fn into_tokens(self) -> TokenStream {
        if self.usages.is_empty() {
            return TokenStream::new();
        }
        let module = Ident::new(WARNING_MODULE, Span::call_site());
        let condition_type = self.has_conditional.then(|| {
            quote! { pub struct WarnIf<const ENABLED: bool>; }
        });
        let definitions = &self.definitions;
        let usages = &self.usages;
        quote! {
            #[doc(hidden)]
            #[allow(non_snake_case, non_camel_case_types, unreachable_pub, dead_code)]
            mod #module {
                #condition_type
                #(#definitions)*
            }

            const _: () = { #(#usages)* };
        }
    }
}

#[cfg(test)]
mod tests {
    use proc_macro2::Span;
    use quote::quote;

    use super::{Warning, WarningSet};
    use crate::warnings::WarningCode;

    fn orphan(condition: Option<proc_macro2::TokenStream>) -> Warning {
        Warning {
            code: WarningCode::UnreachableState,
            span: Span::call_site(),
            subject: "Orphan".to_owned(),
            message: "state `Orphan` is unreachable".to_owned(),
            condition,
        }
    }

    #[test]
    fn unconditional_warning_is_a_deprecated_function_and_its_call() {
        let allowed = [];
        let mut set = WarningSet::new(&allowed);
        set.push(&orphan(None));
        let text = set.into_tokens().to_string();
        assert!(text.contains("deprecated"));
        assert!(text.contains("estado::W0001: state `Orphan` is unreachable"));
        assert!(text.contains("allow(W0001)"));
        assert!(text.contains("W0001_Orphan ()"));
        assert!(!text.contains("WarnIf"));
    }

    #[test]
    fn conditional_warning_goes_through_the_condition_type() {
        let allowed = [];
        let mut set = WarningSet::new(&allowed);
        set.push(&orphan(Some(quote! { true })));
        let text = set.into_tokens().to_string();
        assert!(text.contains("WarnIf < true >"));
        assert!(text.contains("WarnIf :: < { true } > :: W0001_Orphan ()"));
    }

    #[test]
    fn allowed_codes_generate_nothing() {
        let allowed = [syn::Ident::new("W0001", Span::call_site())];
        let mut set = WarningSet::new(&allowed);
        set.push(&orphan(None));
        assert!(set.into_tokens().is_empty());
    }
}
