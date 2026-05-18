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
pub const REGISTRY: &[(&str, &str)] = &[
    ("sk-ant-", "https://api.anthropic.com"),
    ("sk-or-", "https://openrouter.ai/api/v1"),
    ("gsk_", "https://api.groq.com/openai/v1"),
    ("AIza", "https://generativelanguage.googleapis.com/v1beta/openai"),
    // Generic OpenAI — LEAST specific, MUST stay last (longest-prefix).
    ("sk-", "https://api.openai.com/v1"),
];

/// Longest (most-specific) registered prefix of `token` → its upstream
/// base, else `None` (unknown ⇒ caller keeps its default, never a
/// guess). `REGISTRY` is ordered most-specific-first, so the first
/// `starts_with` hit IS the longest prefix.
#[must_use]
pub fn base_for_token(token: &str) -> Option<&'static str> {
    REGISTRY
        .iter()
        .find(|(prefix, _)| token.starts_with(prefix))
        .map(|(_, base)| *base)
}

/// Extract the bearer token from request headers (`Authorization:
/// Bearer <token>`, both the header name and the `Bearer` scheme
/// case-insensitive; surrounding spaces trimmed). `None` if absent,
/// not a bearer, or the token is empty (no meaningful key).
#[must_use]
pub fn bearer_token(headers: &[(String, String)]) -> Option<&str> {
    let value = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str())?;
    let rest = value
        .get(..7)
        .filter(|p| p.eq_ignore_ascii_case("bearer "))
        .map(|_| value[7..].trim())?;
    (!rest.is_empty()).then_some(rest)
}

/// Choose the OpenAI-compatible upstream for one request. Precedence
/// (P2/D-017): an EXPLICIT user upstream (`--to`/`--provider`/
/// `CTX_UPSTREAM_*`/`*_BASE_URL`) ALWAYS wins; else infer from the
/// request's bearer-key prefix; else the caller's default. Pure;
/// the sole decision, isolated for exact-boundary mutation pinning.
#[must_use]
pub fn resolve_openai_base<'a>(
    explicit: bool,
    default_base: &'a str,
    headers: &'a [(String, String)],
) -> &'a str {
    if explicit {
        return default_base; // explicit upstream is authoritative
    }
    match bearer_token(headers).and_then(base_for_token) {
        Some(inferred) => inferred,
        None => default_base, // unknown ⇒ keep default, never a guess
    }
}

/// P3/D-017 — `--provider <name>` shortcuts. Same bases as the
/// key-prefix table; only providers with a single well-known base
/// (Azure is resource-specific ⇒ use `--to <url>` instead — by
/// design, not an omission).
pub const PROVIDERS: &[(&str, &str)] = &[
    ("openai", "https://api.openai.com/v1"),
    ("anthropic", "https://api.anthropic.com"),
    ("openrouter", "https://openrouter.ai/api/v1"),
    ("groq", "https://api.groq.com/openai/v1"),
    ("google", "https://generativelanguage.googleapis.com/v1beta/openai"),
];

/// Resolve a `--provider` NAME (case-insensitive) to its upstream
/// base, else `None` (caller must surface a clear error — never a
/// silent/guessed upstream).
#[must_use]
pub fn base_for_provider(name: &str) -> Option<&'static str> {
    PROVIDERS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, base)| *base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(name: &str, val: &str) -> Vec<(String, String)> {
        vec![(name.to_string(), val.to_string())]
    }

    #[test]
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

    #[test]
    fn resolve_openai_base_precedence_exact() {
        let or = h("Authorization", "Bearer sk-or-v1-x");
        let unknown = h("Authorization", "Bearer xoxb-x");
        let none: &[(String, String)] = &[];
        // EXPLICIT always wins — even with a recognised key present.
        assert_eq!(
            resolve_openai_base(true, "https://my.gw/v1", &or),
            "https://my.gw/v1"
        );
        // Not explicit + known key ⇒ inferred upstream (the P2 win).
        assert_eq!(
            resolve_openai_base(false, "https://api.openai.com/v1", &or),
            "https://openrouter.ai/api/v1"
        );
        // Not explicit + unknown key ⇒ the default (never a guess).
        assert_eq!(
            resolve_openai_base(false, "https://api.openai.com/v1", &unknown),
            "https://api.openai.com/v1"
        );
        // Not explicit + no auth ⇒ the default.
        assert_eq!(
            resolve_openai_base(false, "https://api.openai.com/v1", none),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn base_for_provider_exact_and_case_insensitive() {
        assert_eq!(base_for_provider("openai"), Some("https://api.openai.com/v1"));
        assert_eq!(base_for_provider("anthropic"), Some("https://api.anthropic.com"));
        assert_eq!(
            base_for_provider("OpenRouter"),
            Some("https://openrouter.ai/api/v1")
        );
        assert_eq!(base_for_provider("groq"), Some("https://api.groq.com/openai/v1"));
        assert_eq!(
            base_for_provider("GOOGLE"),
            Some("https://generativelanguage.googleapis.com/v1beta/openai")
        );
        // unknown ⇒ None (caller errors clearly; never a guess)
        assert_eq!(base_for_provider("azure"), None);
        assert_eq!(base_for_provider(""), None);
    }
}
