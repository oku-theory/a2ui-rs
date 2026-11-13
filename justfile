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

# The declared minimum Rust version, `rust-version` in the workspace Cargo.toml.
msrv-version:
    #!/usr/bin/env bash
    set -euo pipefail
    version="$(sed -n 's/^rust-version = "\(.*\)"$/\1/p' Cargo.toml)"
    # One line of digits and dots: CI writes it to $GITHUB_ENV, where a newline adds a variable.
    if ! [[ "$version" =~ ^[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
        echo "Cargo.toml needs one rust-version such as 1.88, found: $version" >&2
        exit 1
    fi
    echo "$version"

# Builds and tests on the declared MSRV, which overrides rust-toolchain.toml's pin. The
# trybuild test is left out: its .stderr files hold the pinned compiler's wording.
msrv:
    #!/usr/bin/env bash
    set -euo pipefail
    # Assigned, then exported: `export "$(…)"` would hide the recipe's failure from `set -e`.
    RUSTUP_TOOLCHAIN="$(just msrv-version)"
    export RUSTUP_TOOLCHAIN
    # Its own target directory, so the pinned toolchain's build stays whole and this one can go.
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/msrv}"
    rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --target wasm32-unknown-unknown
    # A green run must mean the MSRV ran, not a silently kept pin.
    if ! rustc -V | grep -q "^rustc $RUSTUP_TOOLCHAIN[. ]"; then
        echo "rustc is not $RUSTUP_TOOLCHAIN: $(rustc -V)" >&2
        exit 1
    fi
    cargo build --workspace --all-targets --locked
    cargo build -p a2ui-runtime -p a2ui-catalog-basic --target wasm32-unknown-unknown --locked
    cargo nextest run --workspace --locked -E 'not binary(reason_compile_fail)'

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
