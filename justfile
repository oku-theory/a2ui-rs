# Formatting, lints, dependency rules and the wasm build.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    just deps
    just wasm

# a2ui-runtime: no direct UI, async or JS dependency; wasm-bindgen only through getrandom.
deps:
    #!/usr/bin/env bash
    set -euo pipefail
    direct="$(cargo tree -p a2ui-runtime --target all -e normal --depth 1 --prefix none)"
    if grep -E '^(leptos|tokio|async-std|wasm-bindgen|js-sys|web-sys) ' <<< "$direct"; then
        echo "a2ui-runtime has a forbidden direct dependency" >&2
        exit 1
    fi
    parents="$(cargo tree -p a2ui-runtime --target wasm32-unknown-unknown -e normal \
        -i wasm-bindgen --depth 1 --prefix none | sed 's/ v.*//' | grep -vx wasm-bindgen | sort -u)"
    test "$parents" = getrandom

# Every test, conformance suites included.
test:
    cargo nextest run --workspace

# The runtime must build for the browser.
wasm:
    cargo build -p a2ui-runtime -p a2ui-catalog-basic --target wasm32-unknown-unknown

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
