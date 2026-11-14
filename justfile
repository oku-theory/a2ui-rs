# Formatting, lints, dependency rules, pinned and audited actions, and the wasm build.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    just deps
    just actions-pinned
    just zizmor
    just wasm

# Every workflow step's action is a full commit SHA, its release in a comment for Renovate.
# Checks the form only, offline: `just zizmor-online` checks each SHA against its tag.
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

# Audits .github with zizmor, offline: pins, permissions, credentials, injections.
zizmor: zizmor-installed
    zizmor --offline .github

# zizmor's online audits too, with GH_TOKEN: each SHA is its commented tag's commit, in the named
# repository (no fork's impostor commit), and no pinned action has a known advisory. CI runs it.
zizmor-online: zizmor-installed
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${GH_TOKEN:-}" ]; then
        echo "zizmor-online needs GH_TOKEN, a token that reads public repositories" >&2
        exit 1
    fi
    # These would skip the online audits, or swap the config, and still pass.
    unset ZIZMOR_OFFLINE ZIZMOR_NO_ONLINE_AUDITS ZIZMOR_CONFIG ZIZMOR_NO_CONFIG
    zizmor --gh-token "$GH_TOKEN" .github

# zizmor is on PATH; prints its version, which CI takes from taiki-e/install-action's manifest.
[private]
zizmor-installed:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v zizmor > /dev/null; then
        echo "zizmor is missing: brew install zizmor, or cargo install --locked zizmor" >&2
        exit 1
    fi
    zizmor --version

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
    just msrv-toolchain
    cargo build --workspace --all-targets --locked
    cargo build -p a2ui-runtime -p a2ui-catalog-basic --target wasm32-unknown-unknown --locked
    cargo nextest run --workspace --locked -E 'not binary(reason_compile_fail)'
    cargo test --doc --workspace --locked
    just msrv-compile-fail

# Installs RUSTUP_TOOLCHAIN, set by the caller, and checks that rustc is that version.
[private]
msrv-toolchain:
    #!/usr/bin/env bash
    set -euo pipefail
    rustup toolchain install "${RUSTUP_TOOLCHAIN:?set by the calling recipe}" --profile minimal \
        --target wasm32-unknown-unknown
    # A green run must mean the MSRV ran, not a silently kept pin.
    if ! rustc -V | grep -q "^rustc $RUSTUP_TOOLCHAIN[. ]"; then
        echo "rustc is not $RUSTUP_TOOLCHAIN: $(rustc -V)" >&2
        exit 1
    fi

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
    # Kept when a case's errors differ, for its .stderr; ignored by git.
    keep=""
    trap '[ -n "$keep" ] || rm -rf "$copy"' EXIT
    cp "$crate"/tests/ui/*.rs "$copy"/
    TRYBUILD=overwrite A2UI_TRYBUILD_CASES="${copy#"$crate"/}/*.rs" \
        cargo test -p a2ui-runtime --test reason_compile_fail --locked
    # Each error as its code and the first case line among its spans (`-->` or `:::`, its own or
    # a help's or note's), both kept when a release rewords the text; later spans and notes,
    # which a release may add or drop, are not compared. A code-less `error:` counts as
    # `error`, a block with no case line as `-`; a `warning:` ends the error before it.
    failures() {
        awk -v at="/$2:" '
            function flush() { if (code != "") print code "\t" line; code = "" }
            /^warning(\[[^]]*\])?: / { flush(); next }
            /^error(\[E[0-9]+\])?: / {
                flush()
                code = substr($0, 1, index($0, ":") - 1)
                line = "-"
                next
            }
            code != "" && line == "-" && /^ *(-->|:::) / && index($0, at) {
                rest = substr($0, index($0, at) + length(at))
                if (match(rest, /^[0-9]+/)) line = substr(rest, 1, RLENGTH)
            }
            END { flush() }
        ' "$1" | sort
    }
    failed=""
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
        # Else a parser that found no line would compare codes alone, on both sides.
        if grep -q $'\t-$' <<< "$expected"; then
            echo "tests/ui/$stderr has an error pointing at no line of $name" >&2
            exit 1
        fi
        if [ ! -f "$copy/$stderr" ]; then
            echo "$name did not run" >&2
            exit 1
        fi
        actual="$(failures "$copy/$stderr" "$name")"
        if [ "$expected" != "$actual" ]; then
            keep=1
            failed=1
            echo "$name fails otherwise on $(rustc -V): expected" >&2
            echo "$expected" >&2
            echo "got" >&2
            echo "$actual" >&2
            echo "its .stderr, kept in $copy:" >&2
            diff -u "$crate/tests/ui/$stderr" "$copy/$stderr" >&2 || true
        fi
    done
    if [ -n "$failed" ]; then
        echo "remove the copy when done: rm -rf $copy" >&2
        exit 1
    fi

# On the MSRV, a user's crate resolves the packaged libraries afresh: builds unreviewed releases.
msrv-resolved:
    #!/usr/bin/env bash
    set -euo pipefail
    version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)"
    if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "Cargo.toml needs one workspace version such as 0.1.0, found: $version" >&2
        exit 1
    fi
    # Cargo.lock proves our versions only. A crate of its own, with no lock, no dev-dependencies
    # and no rust-version, resolves the newest versions this rustc accepts (resolver 3, as in an
    # edition 2024 crate), the way a user's does; its `[workspace]` keeps it out of ours. A
    # killed run leaves it under the ignored target/.
    mkdir -p target
    user="$(mktemp -d target/msrv-user.XXXXXX)"
    trap 'rm -rf "$user"' EXIT
    # It depends on the packages crates.io would get, unpacked, not on our sources: a file missing
    # from `include` fails here too. The pinned toolchain packages them (the MSRV's cargo cannot
    # package a crate whose sibling is not yet published); --no-verify, as nothing is built yet.
    unset RUSTUP_TOOLCHAIN
    cargo package -p a2ui-runtime -p a2ui-catalog-basic --locked --no-verify --allow-dirty \
        --target-dir "$PWD/$user/package"
    mkdir "$user/packages" "$user/src"
    for crate in a2ui-runtime a2ui-catalog-basic; do
        tar -xzf "$user/package/package/$crate-$version.crate" -C "$user/packages"
    done
    : > "$user/src/lib.rs"
    # [patch.crates-io] stands the packages in for releases crates.io may not have yet;
    # `exclude` keeps them out of its workspace, so their dev-dependencies stay unresolved.
    cat > "$user/Cargo.toml" <<EOF
    [package]
    name = "msrv-user"
    version = "0.0.0"
    edition = "2024"
    publish = false

    [dependencies]
    a2ui-runtime = "=$version"
    a2ui-catalog-basic = "=$version"

    [patch.crates-io]
    a2ui-runtime = { path = "packages/a2ui-runtime-$version" }
    a2ui-catalog-basic = { path = "packages/a2ui-catalog-basic-$version" }

    [workspace]
    exclude = ["packages"]
    EOF
    RUSTUP_TOOLCHAIN="$(just msrv-version)"
    export RUSTUP_TOOLCHAIN
    just msrv-toolchain
    # Resolving runs no crate's code: known advisories, known-malicious crates included, and yanked
    # releases stop the run before any build script does.
    cargo generate-lockfile --manifest-path "$user/Cargo.toml"
    # A patch cargo did not use only warns: our crates must come from the packages, sourceless.
    ours='^name = "a2ui-(runtime|catalog-basic)"$'
    if grep -A2 -E "$ours" "$user/Cargo.lock" | grep -q '^source'; then
        echo "the user crate resolved a2ui from a registry, not from the packages" >&2
        exit 1
    fi
    cargo audit --file "$user/Cargo.lock" --deny yanked
    # Its builds stay in the crate's own target/, gone with it, whatever the caller's setting.
    export CARGO_TARGET_DIR="$PWD/$user/target"
    # Every version this resolution picked, every target's and build dependencies, for the log.
    cargo tree --manifest-path "$user/Cargo.toml" --target all -e normal,build --prefix none \
        | sed -E 's/ \((\*|proc-macro)\)//g' | sort -u
    cargo check --manifest-path "$user/Cargo.toml" --locked
    cargo check --manifest-path "$user/Cargo.toml" --locked --target wasm32-unknown-unknown

# cargo-semver-checks against our libraries' crates.io releases: builds unreviewed releases.
semver-checks:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo semver-checks --version
    # The workspace members cargo would publish (`publish = false` reads as []).
    names="$(cargo metadata --no-deps --format-version 1 \
        | jq -r '.packages[] | select(.publish != []) | .name')"
    if [ -z "$names" ]; then
        echo "no publishable workspace member" >&2
        exit 1
    fi
    checked=()
    for name in $names; do
        # crates.io's sparse index file is /ab/cd/<name>, lowercase, for four characters or more:
        # any other name would 404 and be skipped, so it fails here instead.
        if ! [[ "$name" =~ ^[a-z0-9][a-z0-9_-]{3,}$ ]]; then
            echo "unexpected crate name for the index path: $name" >&2
            exit 1
        fi
        url="https://index.crates.io/${name:0:2}/${name:2:2}/$name"
        status="$(curl -sS --proto '=https' --retry 3 --max-time 30 -o /dev/null \
            -w '%{http_code}' "$url")"
        case "$status" in
            # Both sides resolve outside Cargo.lock: CI or a disposable machine only.
            200) checked+=(--package "$name") ;;
            # No release yet, so no baseline: said in the log and on the run's summary page.
            # Once every library is published, a 404 is a bug (plan 2b, Decision 28).
            404) echo "::notice::$name is not on crates.io yet: no baseline to check against" ;;
            *)
                echo "crates.io index answered $status for $name" >&2
                exit 1
                ;;
        esac
    done
    if [ "${#checked[@]}" -eq 0 ]; then
        echo "::notice::no library is on crates.io yet: nothing to check"
        exit 0
    fi
    cargo semver-checks "${checked[@]}"

# Licenses, sources (crates.io only) and advisories, yanked releases included: it reads the lock
# alone, so `ci` runs it before anything builds.
audit:
    cargo deny check licenses sources
    cargo audit --deny yanked

# Packages both libraries as crates.io receives them, each built from its package alone, so a
# file missing from `include` fails here rather than for a user; their license texts must ship.
package:
    #!/usr/bin/env bash
    set -euo pipefail
    # Locally from the working tree, as `just ci` runs before a commit; in CI (CI is set) cargo
    # refuses a tree a step dirtied, which would ship a file a fresh checkout lacks.
    dirty="--allow-dirty"
    if [ -n "${CI:-}" ]; then dirty=""; fi
    # A build misses no legal file: each crate's license texts, and upstream's NOTICE once it has
    # one.
    notice=()
    if [ -f vendor/a2ui/NOTICE ]; then notice=(NOTICE-A2UI); fi
    shipped() {
        local crate="$1" listed
        shift
        listed="$(cargo package --list -p "$crate" --locked --allow-dirty)"
        for file in "$@"; do
            if ! grep -qxF "$file" <<< "$listed"; then
                echo "$crate's package lacks $file" >&2
                exit 1
            fi
        done
    }
    shipped a2ui-runtime LICENSE-MIT LICENSE-APACHE schemas/v1_0/LICENSE \
        ${notice[@]+"${notice[@]/#/schemas/v1_0/}"}
    shipped a2ui-catalog-basic LICENSE-A2UI catalog.json ${notice[@]+"${notice[@]}"}
    cargo package -p a2ui-runtime -p a2ui-catalog-basic --locked $dirty

# The full pre-merge gate.
# `audit` first, before any build script runs; `package` before `test`, so no file a test writes
# under a crate slips into the package.
ci: audit check package test

# Writes the executed-case manifest (one process, so no duplicate records).
conformance-manifest dir:
    A2UI_CONFORMANCE_MANIFEST_DIR={{dir}} cargo test -p a2ui-conformance --test core

# Re-vendors the upstream specification and suites at a commit.
vendor-sync commit:
    ./vendor/sync.sh {{commit}}
