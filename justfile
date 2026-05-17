check:
    cargo clippy --all-targets --all-features -- -D warnings
test:
    cargo nextest run --no-fail-fast
    cargo test --doc
fix:
    cargo clippy --fix --allow-dirty --allow-staged
fmt:
    cargo fmt --all
harden:
    cargo deny check
    cargo machete
    cargo semver-checks 2>/dev/null || true
