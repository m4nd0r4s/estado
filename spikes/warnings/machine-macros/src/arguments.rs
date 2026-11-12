//! Parsing of `#[machine(allow(W0001, ...), strategy = "...")]`.

use proc_macro2::TokenStream;
use syn::LitStr;
use syn::parse::Parser;

use crate::warning::{SpanSource, WarningCode, WarningStrategy};

#[derive(Default)]
pub struct MachineArguments {
    pub allowed_codes: Vec<WarningCode>,
    pub strategy: WarningStrategy,
    pub span_source: SpanSource,
}

impl MachineArguments {
    pub fn allows(&self, code: WarningCode) -> bool {
        self.allowed_codes.contains(&code)
    }
}

pub fn parse(tokens: TokenStream) -> syn::Result<MachineArguments> {
    let mut arguments = MachineArguments::default();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("allow") {
            meta.parse_nested_meta(|code_meta| {
                let code_ident = code_meta
                    .path
                    .get_ident()
                    .ok_or_else(|| code_meta.error("expected a warning code such as W0001"))?;
                match WarningCode::from_ident(code_ident) {
                    Some(code) => arguments.allowed_codes.push(code),
                    None => {
                        return Err(syn::Error::new(
                            code_ident.span(),
                            format!("unknown warning code `{code_ident}`"),
                        ));
                    }
                }
                Ok(())
            })
        } else if meta.path.is_ident("strategy") {
            let value: LitStr = meta.value()?.parse()?;
            arguments.strategy = WarningStrategy::from_name(&value)?;
            Ok(())
        } else if meta.path.is_ident("span") {
            let value: LitStr = meta.value()?.parse()?;
            arguments.span_source = SpanSource::from_name(&value)?;
            Ok(())
        } else {
            Err(meta.error("unsupported `machine` argument"))
        }
    });
    parser.parse2(tokens)?;
    Ok(arguments)
}
