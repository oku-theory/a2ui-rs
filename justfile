# Formatting, lints and the wasm build.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    just wasm

# Every test, conformance suites included.
test:
    cargo nextest run --workspace

# The runtime must build for the browser.
wasm:
    cargo build -p a2ui-runtime --target wasm32-unknown-unknown

# Licenses and advisories.
audit:
    cargo deny check licenses
    cargo audit

# The full pre-merge gate.
ci: check test audit

# Writes the executed-case manifest (one process, so no duplicate records).
conformance-manifest dir:
    A2UI_CONFORMANCE_MANIFEST_DIR={{dir}} cargo test -p a2ui-conformance --test core

# Re-vendors the upstream specification and suites at a commit.
vendor-sync commit:
    ./vendor/sync.sh {{commit}}
