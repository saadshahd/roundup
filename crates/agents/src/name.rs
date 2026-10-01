//! The Rail name an Agent takes from its first prompt.

/// What an Agent is called until its first prompt names it.
pub const UNNAMED: &str = "new-agent";

const FILLER: [&str; 10] = [
    "a", "an", "the", "to", "of", "in", "on", "for", "and", "please",
];

const MAX_CHARS: usize = 32;

/// The first three words of `prompt` that are not filler, lower-cased and joined by `-`. `None`
/// when no word is left.
pub fn from_prompt(prompt: &str) -> Option<String> {
    let lower = prompt.to_lowercase();
    let words: Vec<_> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty() && !FILLER.contains(word))
        .take(3)
        .collect();
    let joined: String = words.join("-").chars().take(MAX_CHARS).collect();
    let name = joined.trim_end_matches('-');
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::from_prompt;

    #[test]
    fn a9_filler_and_punctuation_drop_out_and_three_words_remain() {
        assert_eq!(
            from_prompt("Fix the refresh race in token.ts").as_deref(),
            Some("fix-refresh-race")
        );
    }

    #[test]
    fn a9_a_long_name_is_cut_at_32_characters_without_a_trailing_dash() {
        let name = from_prompt("internationalisation-free ABCDEFGHIJKLMNOPQRSTUVWXYZ").unwrap();
        assert_eq!(name, "internationalisation-free-abcdef");
        let cut_at_a_dash = from_prompt("abcdefghijklmnopqrstuvwxyzabcde next word").unwrap();
        assert_eq!(cut_at_a_dash, "abcdefghijklmnopqrstuvwxyzabcde");
    }

    #[test]
    fn a9_words_are_unicode_letters_and_digits() {
        assert_eq!(
            from_prompt("Über 2nd Straße!").as_deref(),
            Some("über-2nd-straße")
        );
    }

    #[test]
    fn a9_a_prompt_with_no_words_left_has_no_name() {
        assert_eq!(from_prompt("Please, the... of!"), None);
        assert_eq!(from_prompt(""), None);
    }
}
