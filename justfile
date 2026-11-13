# Formatting, lints, dependency rules, pinned actions and the wasm build.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    just deps
    just actions-pinned
    just wasm

# Every workflow step's action is a full commit SHA, its release in a comment for Renovate.
# Checks the form only, offline: that each SHA is its tag's commit was checked by hand (t-1619).
actions-pinned:
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    workflows=(.github/workflows/*.yml .github/workflows/*.yaml)
    if [ "${#workflows[@]}" -eq 0 ]; then
        echo "no workflow found under .github/workflows" >&2
        exit 1
    fi
    # No `|| true` here: an unreadable file, or no `uses:` at all, fails the recipe.
    uses="$(grep -HnE '^[[:space:]-]*uses:' "${workflows[@]}")"
    unpinned="$(grep -vE 'uses: [^@[:space:]]+@[0-9a-f]{40} # v?[0-9]+\.[0-9]+\.[0-9]+$' \
        <<< "$uses" || true)"
    if [ -n "$unpinned" ]; then
        echo "actions not pinned as owner/repo@<40-hex SHA> # vX.Y.Z:" >&2
        echo "$unpinned" >&2
        exit 1
    fi

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

# Every test, conformance suites included, then doctests, which nextest does not run.
test:
    cargo nextest run --workspace
    cargo test --doc --workspace

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

# Builds and tests on the declared MSRV, which overrides rust-toolchain.toml's pin. trybuild's
# .stderr files hold the pinned compiler's wording, so its cases are checked by code and line.
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
    cargo test --doc --workspace --locked
    just msrv-compile-fail

# On the toolchain in use, each trybuild case still fails with its .stderr's codes and lines.
[private]
msrv-compile-fail:
    #!/usr/bin/env bash
    set -euo pipefail
    crate=crates/a2ui-runtime
    # Absolute, since a test binary runs in its crate's directory: trybuild's own build would
    # otherwise land under crates/a2ui-runtime/target.
    if [ -n "${CARGO_TARGET_DIR:-}" ]; then
        case "$CARGO_TARGET_DIR" in /*) ;; *) export CARGO_TARGET_DIR="$PWD/$CARGO_TARGET_DIR" ;; esac
    fi
    # The cases alone, copied beside tests/ui, where each `#[path = "../../src/…"]` still
    # resolves. TRYBUILD=overwrite writes each case's .stderr there with this compiler's wording,
    # so a case trybuild did not run has none, and still fails a case that compiles.
    copy="$(mktemp -d "$crate/tests/ui-msrv.XXXXXX")"
    trap 'rm -rf "$copy"' EXIT
    cp "$crate"/tests/ui/*.rs "$copy"/
    TRYBUILD=overwrite A2UI_TRYBUILD_CASES="${copy#"$crate"/}/*.rs" \
        cargo test -p a2ui-runtime --test reason_compile_fail --locked
    # Each error's code and its line in the case, both kept when a release rewords the text.
    failures() { grep -oE "error\[E[0-9]+\]|/${2//./\\.}:[0-9]+" "$1" | sort || true; }
    for case in "$crate"/tests/ui/*.rs; do
        name="$(basename "$case")"
        stderr="${name%.rs}.stderr"
        if [ ! -f "$crate/tests/ui/$stderr" ]; then
            echo "tests/ui/$name has no .stderr: refresh it on the pinned toolchain" >&2
            exit 1
        fi
        expected="$(failures "$crate/tests/ui/$stderr" "$name")"
        if ! grep -q 'error\[E' <<< "$expected"; then
            echo "tests/ui/$stderr holds no error code to compare" >&2
            exit 1
        fi
        if [ ! -f "$copy/$stderr" ]; then
            echo "$name did not run" >&2
            exit 1
        fi
        actual="$(failures "$copy/$stderr" "$name")"
        if [ "$expected" != "$actual" ]; then
            echo "$name fails otherwise on $(rustc -V): expected" >&2
            echo "$expected" >&2
            echo "got" >&2
            echo "$actual" >&2
            exit 1
        fi
    done

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
