//! P2 / D-017 — tiny offline provider registry + key-prefix resolution.
//!
//! When the user gives NO explicit upstream (`--to` / `--provider` /
//! `CTX_UPSTREAM_*` / `*_BASE_URL`), ctx infers the real
//! OpenAI-compatible upstream from the request's `Authorization` bearer
//! key prefix — so `ctx run -- <yourscript>` "just works" for the
//! well-known providers. Pure data + pure resolution; offline; never a
//! network call (the §8 registry seam, the moat unchanged). Explicit
//! always wins over inferred (resolved in `proxy`). Only DISTINCTIVE
//! key prefixes are mapped — a provider without a stable public prefix
//! is deliberately absent (resolve to `None` ⇒ caller keeps the
//! default; never a guessed/wrong upstream).
//!
//! STUB phase: returns `None` / `&[]` so the contract tests compile and
//! FAIL (red-first); the real table/logic land in the impl commit.

/// `(key-prefix, full upstream base)` — full base WITH the provider's
/// real path (D-017 verbatim-forward). Ordered MOST-SPECIFIC FIRST so
/// `base_for_token` takes the longest matching prefix (`sk-or-` must
/// win over the generic `sk-`).
pub const REGISTRY: &[(&str, &str)] = &[];

/// Longest (most-specific) registered prefix of `token` → its upstream
/// base, else `None` (unknown ⇒ caller keeps its default, never a
/// guess).
#[must_use]
pub fn base_for_token(_token: &str) -> Option<&'static str> {
    None
}

/// Extract the bearer token from request headers (`Authorization:
/// Bearer <token>`, header name case-insensitive). `None` if absent or
/// not a bearer.
#[must_use]
pub fn bearer_token(_headers: &[(String, String)]) -> Option<&str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(name: &str, val: &str) -> Vec<(String, String)> {
        vec![(name.to_string(), val.to_string())]
    }

    #[test]
    #[ignore = "P2/D-017 red: registry stubs return None; un-ignored at the impl commit"]
    fn registry_is_nonempty_and_most_specific_first() {
        // Structural pin: a generic `sk-` entry (if present) must come
        // AFTER the specific `sk-…` ones so longest-prefix wins.
        assert!(!REGISTRY.is_empty());
        let sk_generic = REGISTRY.iter().position(|(p, _)| *p == "sk-");
        for spec in ["sk-ant-", "sk-or-"] {
            let i = REGISTRY
                .iter()
                .position(|(p, _)| *p == spec)
                .unwrap_or_else(|| panic!("{spec} must be registered"));
            if let Some(g) = sk_generic {
                assert!(i < g, "{spec} must precede the generic sk-");
            }
        }
    }

    #[test]
    #[ignore = "P2/D-017 red: registry stubs return None; un-ignored at the impl commit"]
    fn base_for_token_longest_prefix_exact() {
        assert_eq!(
            base_for_token("sk-ant-api03-xxxx"),
            Some("https://api.anthropic.com")
        );
        assert_eq!(
            base_for_token("sk-or-v1-deadbeef"),
            Some("https://openrouter.ai/api/v1")
        );
        assert_eq!(
            base_for_token("gsk_abcdEFGH"),
            Some("https://api.groq.com/openai/v1")
        );
        assert_eq!(
            base_for_token("AIzaSyD-xxxx"),
            Some("https://generativelanguage.googleapis.com/v1beta/openai")
        );
        // generic OpenAI
        assert_eq!(base_for_token("sk-proj-xxxx"), Some("https://api.openai.com/v1"));
        assert_eq!(base_for_token("sk-xxxx"), Some("https://api.openai.com/v1"));
        // longest-prefix: sk-or-/sk-ant- must NOT fall to the generic sk-
        assert_ne!(base_for_token("sk-or-x"), Some("https://api.openai.com/v1"));
        assert_ne!(base_for_token("sk-ant-x"), Some("https://api.openai.com/v1"));
        // unknown / empty ⇒ None (caller keeps its default; never a guess)
        assert_eq!(base_for_token("xoxb-not-a-key"), None);
        assert_eq!(base_for_token(""), None);
    }

    #[test]
    #[ignore = "P2/D-017 red: registry stubs return None; un-ignored at the impl commit"]
    fn bearer_token_extraction_exact() {
        assert_eq!(
            bearer_token(&h("Authorization", "Bearer sk-or-v1-abc")),
            Some("sk-or-v1-abc")
        );
        // header name case-insensitive; scheme case-insensitive; spaces trimmed
        assert_eq!(
            bearer_token(&h("authorization", "bearer   sk-x  ")),
            Some("sk-x")
        );
        // not a bearer ⇒ None
        assert_eq!(bearer_token(&h("Authorization", "Basic dXNlcg==")), None);
        // empty token ⇒ None (no meaningful key)
        assert_eq!(bearer_token(&h("Authorization", "Bearer ")), None);
        // absent ⇒ None
        assert_eq!(bearer_token(&h("X-Other", "Bearer sk-x")), None);
        assert_eq!(bearer_token(&[]), None);
    }
}
