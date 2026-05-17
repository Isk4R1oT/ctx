//! ctx — htop / EXPLAIN ANALYZE for LLM prompts.
//!
//! Wire-proxy at the LLM-API boundary (canonical mechanism, see
//! `docs/PROJECT.md` + `docs/DECISIONS.md` D-001). This is the green
//! scaffold baseline; F0 modules replace `parse_count` next.

mod error;
pub use error::{Error, Result};

/// Parse `s` into a positive count.
///
/// # Examples
/// ```
/// assert_eq!(ctx::parse_count("3").unwrap(), 3);
/// ```
pub fn parse_count(s: &str) -> Result<u32> {
    s.trim()
        .parse()
        .map_err(|_| Error::Io(std::io::Error::other("not a number")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        assert_eq!(parse_count(" 7 ").unwrap(), 7);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_count("x").is_err());
    }
}
