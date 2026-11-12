//! Name conversions shared by analysis and the generators.

use syn::ext::IdentExt;
use syn::{Ident, Path};

/// Facade crates a marker attribute or a transition macro may be qualified
/// with (`#[estado::state]`, `estado::goto!`).
pub const FACADE_CRATES: [&str; 2] = ["estado", "estado_macros"];

/// Name of the generic effect sink parameter of generated functions and
/// rewritten handlers.
pub const SINK_PARAMETER: &str = "EffectSink";

/// The name a marker or macro path stands for: `initial` for `initial` and
/// for `estado::initial`; `None` for any other path.
pub fn facade_name(path: &Path) -> Option<&Ident> {
    let segments = &path.segments;
    match segments.len() {
        1 => Some(&segments[0].ident),
        2 if FACADE_CRATES
            .iter()
            .any(|facade| segments[0].ident == facade) =>
        {
            Some(&segments[1].ident)
        }
        _ => None,
    }
}

/// `PlayingExiting`: the owned value a constructor receives (`Exiting![X]`).
pub fn exiting_name(base: &Ident) -> Ident {
    Ident::new(&format!("{}Exiting", plain_name(base)), base.span())
}

/// `PlayingFast` -> `PLAYING_FAST`, `keep_alive` -> `KEEP_ALIVE`,
/// `HTTPServer` -> `HTTP_SERVER`. Used for the `StateId` constants (spec 6.5).
pub fn screaming_snake_case(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut result = String::with_capacity(name.len() + 4);
    for (index, &character) in characters.iter().enumerate() {
        if character == '_' {
            if !result.ends_with('_') {
                result.push('_');
            }
            continue;
        }
        if index > 0 && character.is_uppercase() {
            let previous = characters[index - 1];
            let next_is_lowercase = characters
                .get(index + 1)
                .is_some_and(|next| next.is_lowercase());
            let word_boundary = previous.is_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_uppercase() && next_is_lowercase);
            if word_boundary && !result.ends_with('_') {
                result.push('_');
            }
        }
        result.extend(character.to_uppercase());
    }
    result
}

/// The name of an identifier as text, without the `r#` of a raw identifier:
/// the base of every generated name (`r#type` gives `typeMut`, not `r#typeMut`).
pub fn plain_name(name: &Ident) -> String {
    name.unraw().to_string()
}

/// The `StateId` constant name of a state or region.
pub fn state_constant_name(name: &Ident) -> String {
    screaming_snake_case(&name.unraw().to_string())
}

/// `ActiveSub` -> `active_sub`, `keep_alive` -> `keep_alive`: the base of the
/// names of generated functions and bindings.
pub fn snake_case_name(name: &Ident) -> String {
    screaming_snake_case(&name.unraw().to_string()).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::screaming_snake_case;

    #[test]
    fn converts_common_shapes() {
        assert_eq!(screaming_snake_case("Playing"), "PLAYING");
        assert_eq!(screaming_snake_case("PlayingFast"), "PLAYING_FAST");
        assert_eq!(screaming_snake_case("keep_alive"), "KEEP_ALIVE");
        assert_eq!(screaming_snake_case("HTTPServer"), "HTTP_SERVER");
        assert_eq!(screaming_snake_case("Idle2"), "IDLE2");
        assert_eq!(screaming_snake_case("State2Done"), "STATE2_DONE");
    }

    #[test]
    fn snake_case_of_identifiers() {
        let name: syn::Ident = syn::parse_quote!(ActiveSub);
        assert_eq!(super::snake_case_name(&name), "active_sub");
    }
}
