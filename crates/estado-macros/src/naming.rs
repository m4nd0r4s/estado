//! Name conversions shared by analysis and the generators.

use syn::Ident;
use syn::ext::IdentExt;

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
