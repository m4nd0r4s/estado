//! "Did you mean" suggestions by edit distance (spec 6.7).

/// The candidate closest to `input`, when it is close enough to be a typo.
pub fn did_you_mean<'a>(
    input: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let threshold = (input.chars().count() / 3).max(1) + 1;
    candidates
        .into_iter()
        .map(|candidate| (edit_distance(input, candidate), candidate))
        .filter(|(distance, _)| *distance <= threshold)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate)
}

/// Levenshtein distance.
pub fn edit_distance(first: &str, second: &str) -> usize {
    let second_chars: Vec<char> = second.chars().collect();
    let mut previous_row: Vec<usize> = (0..=second_chars.len()).collect();
    for (row_index, first_char) in first.chars().enumerate() {
        let mut current_row = vec![row_index + 1];
        for (column_index, second_char) in second_chars.iter().enumerate() {
            let substitution = previous_row[column_index] + usize::from(first_char != *second_char);
            let insertion = current_row[column_index] + 1;
            let deletion = previous_row[column_index + 1] + 1;
            current_row.push(substitution.min(insertion).min(deletion));
        }
        previous_row = current_row;
    }
    previous_row[second_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::{did_you_mean, edit_distance};

    #[test]
    fn distance_counts_edits() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
    }

    #[test]
    fn suggests_close_names_only() {
        let options = ["event", "effect", "defer_cap"];
        assert_eq!(did_you_mean("evnt", options), Some("event"));
        assert_eq!(did_you_mean("defer_capp", options), Some("defer_cap"));
        assert_eq!(did_you_mean("completely_different", options), None);
    }
}
