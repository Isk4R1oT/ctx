//! Static offline context-window registry (PROJECT.md §8 seam).
//!
//! C3 (D-012) needs the model's context-window size to express
//! `prompt_tokens` as a *fraction of the window*. The model id is on the
//! wire; window sizes are a **small maintained static table**, not an
//! API call (zero-config/offline is core, `docs/PROJECT.md` §4). Like
//! the tokenizer's `±N%`, this table is a **labeled approximation**:
//! context-window sizes drift per model release
//! (`docs/CONTEXT-SIGNALS-RESEARCH.md` §(c) C3, §(e) `[INFER]`).
//!
//! HARD INVARIANT (evalint KILLED): this module is pure data + a pure
//! lookup. It returns a window size or `None` (unknown ⇒ no window claim,
//! never a guess). It makes NO prediction about overflow/truncation —
//! that projection is a neutral `--deep` arithmetic, not a fate.

/// Honest label printed wherever a window fraction is shown — the C3
/// analogue of `tokenizer::ACCURACY_LABEL`. The table is a maintained
/// offline approximation; window sizes change per model release.
pub const WINDOW_LABEL: &str = "offline static window table, approximate (never calls an API)";

/// `(model-id substring, context-window tokens)`. Matched by
/// case-insensitive substring so a dated/suffixed wire id
/// (`claude-sonnet-4-5-20250929`, `gpt-4o-2024-08-06`,
/// `openai/gpt-4o`) still resolves. The lookup takes the **longest**
/// matching key so a specific family wins over a shorter generic one.
///
/// Sources are the providers' own public model docs (May 2026); this is
/// a `[INFER]`-flagged approximation by construction (`WINDOW_LABEL`),
/// not a guarantee — an unknown id returns `None`, never a fabricated
/// size.
const TABLE: &[(&str, usize)] = &[
    // Anthropic — Claude family (200k standard context window).
    ("claude-opus-4", 200_000),
    ("claude-sonnet-4", 200_000),
    ("claude-haiku-4", 200_000),
    ("claude-3-7-sonnet", 200_000),
    ("claude-3-5-sonnet", 200_000),
    ("claude-3-5-haiku", 200_000),
    ("claude-3-opus", 200_000),
    ("claude-3-sonnet", 200_000),
    ("claude-3-haiku", 200_000),
    ("claude-2", 200_000),
    // OpenAI — GPT / o family.
    ("gpt-4o-mini", 128_000),
    ("gpt-4o", 128_000),
    ("gpt-4.1", 1_047_576),
    ("gpt-4-turbo", 128_000),
    ("gpt-4-32k", 32_768),
    ("gpt-4", 8_192),
    ("gpt-3.5-turbo", 16_385),
    ("o1-mini", 128_000),
    ("o1", 200_000),
    ("o3-mini", 200_000),
    ("o3", 200_000),
    ("o4-mini", 200_000),
];

/// Context-window token budget for a wire model id, or `None` when the
/// id is not in the static table (C3 then makes **no window claim** —
/// skipped honestly, never guessed). Pure, offline, deterministic.
///
/// Match is the **longest** table key that is a case-insensitive
/// substring of `model` (so `gpt-4o-mini` is not shadowed by `gpt-4`,
/// and a dated wire id like `claude-sonnet-4-5-20250929` resolves to the
/// `claude-sonnet-4` family). Returns the size, not a judgement.
#[must_use]
pub fn window_for(model: &str) -> Option<usize> {
    let m = model.to_ascii_lowercase();
    TABLE
        .iter()
        .filter(|(key, _)| m.contains(key))
        .max_by_key(|(key, _)| key.len())
        .map(|(_, size)| *size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_models_resolve_to_their_documented_window() {
        // Exact-value pins (mutation-hardening: every asserted size is a
        // load-bearing constant the headline math depends on).
        assert_eq!(window_for("claude-3-5-sonnet-20241022"), Some(200_000));
        assert_eq!(window_for("claude-sonnet-4-5-20250929"), Some(200_000));
        assert_eq!(window_for("gpt-4o-2024-08-06"), Some(128_000));
        assert_eq!(window_for("openai/gpt-4o"), Some(128_000));
        assert_eq!(window_for("gpt-4.1"), Some(1_047_576));
        assert_eq!(window_for("gpt-4-32k"), Some(32_768));
        assert_eq!(window_for("gpt-3.5-turbo-0125"), Some(16_385));
    }

    #[test]
    fn longest_substring_wins_so_specific_family_is_not_shadowed() {
        // `gpt-4o-mini` contains both `gpt-4o` and `gpt-4o-mini`; the
        // longer (more specific) key must win — pins `max_by_key(len)`
        // against a `min`/first-match mutant that would mis-size it.
        assert_eq!(window_for("gpt-4o-mini-2024-07-18"), Some(128_000));
        // `gpt-4-turbo` contains `gpt-4` (8_192) and `gpt-4-turbo`
        // (128_000); the specific one must win.
        assert_eq!(window_for("gpt-4-turbo-preview"), Some(128_000));
    }

    #[test]
    fn case_insensitive_match() {
        assert_eq!(window_for("GPT-4O"), Some(128_000));
        assert_eq!(window_for("Claude-3-Opus-20240229"), Some(200_000));
    }

    #[test]
    fn unknown_model_returns_none_never_a_guess() {
        // The discipline rule: an unknown id ⇒ NO window claim. The
        // headline must skip honestly, never fabricate a size.
        assert_eq!(window_for("some-unreleased-model-9000"), None);
        assert_eq!(window_for(""), None);
        assert_eq!(window_for("llama-3-70b"), None);
        assert_eq!(window_for("mistral-large"), None);
    }

    #[test]
    fn label_is_honest_and_offline() {
        // The C3 analogue of the tokenizer ±N% honesty label.
        assert!(WINDOW_LABEL.contains("approximate"));
        assert!(WINDOW_LABEL.contains("never calls an API"));
    }

    #[test]
    fn table_keys_are_unique_and_nonempty() {
        // A duplicate/empty key would make the longest-match lookup
        // ill-defined; pin the table's structural invariant.
        let mut keys: Vec<&str> = TABLE.iter().map(|(k, _)| *k).collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n, "TABLE keys must be unique");
        assert!(TABLE.iter().all(|(k, _)| !k.is_empty()));
        assert!(TABLE.iter().all(|(_, sz)| *sz > 0));
    }
}
