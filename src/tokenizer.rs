//! Offline token approximation.
//!
//! HARD INVARIANT (`docs/PROJECT.md` §4): the count is a **local
//! approximation, honestly labeled, and never calls an API**. We use the
//! o200k-class BPE whose ranks are embedded in the `tiktoken-rs` crate —
//! no network, no file download, no provider call. It is exact for
//! `OpenAI` o200k models and an approximation for Anthropic (whose
//! server-side tokenizer is not public); the label says so.

use tiktoken_rs::o200k_base_singleton;

/// The honest accuracy label printed wherever a token count is shown
/// (`docs/PROJECT.md` §11 — the wording is fixed here, during F0).
pub const ACCURACY_LABEL: &str = "offline o200k approximation, ±10% (never calls an API)";

/// Count tokens with an offline o200k BPE. Pure, deterministic, no I/O.
#[must_use]
pub fn count(text: &str) -> usize {
    o200k_base_singleton().count_with_special_tokens(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_deterministic_and_offline() {
        let a = count("The quick brown fox jumps over the lazy dog.");
        let b = count("The quick brown fox jumps over the lazy dog.");
        assert_eq!(a, b);
        assert!(a > 0);
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(count(""), 0);
    }

    #[test]
    fn longer_text_has_more_tokens() {
        assert!(count("one two three four five") > count("one"));
    }

    #[test]
    fn label_is_honest_and_offline() {
        assert!(ACCURACY_LABEL.contains('±'));
        assert!(ACCURACY_LABEL.contains("never calls an API"));
    }
}
