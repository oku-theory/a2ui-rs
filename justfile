# The dated nightly that `-Z` flags need, never a build we ship. Renovate's custom manager moves it
# weekly to the newest nightly 14 days old or more; it reads the next two lines exactly as written,
# which scripts/nightly_pin_test.py checks (plan 2b, Decision 41).
# renovate: datasource=rust-version versioning=rust-release-channel
nightly := "nightly-2026-10-31"

# Formatting, lints, dependency rules, pinned and audited actions, and the wasm build.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    just deps
    just actions-pinned
    just zizmor
    just wasm
    just fuzz-check

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
    just lock-diff-test

# The unit tests in scripts/ (lock_diff.py, fuzz_lock.py), offline; -B leaves no __pycache__ behind.
lock-diff-test:
    python3 -B -m unittest discover -s scripts -p '*_test.py'

# Compares the working tree's Cargo.lock with the one where HEAD left `base`, fetched or not.
# What Cargo.lock added, removed and updated since `base`, each new release with its publish date.
lock-diff base="origin/main":
    python3 -B scripts/lock_diff.py "$(git merge-base "{{base}}" HEAD)"

# The runtime must build for the browser.
wasm:
    cargo build -p a2ui-runtime -p a2ui-catalog-basic --target wasm32-unknown-unknown

# Fuzzes `target` (catalog_parse, validate_component) for `seconds` with libFuzzer, on the pinned
# toolchain: cargo-fuzz's coverage flags, without its AddressSanitizer (nightly-only; our crates
# forbid unsafe). Seeds come from vendor/a2ui; a crash lands in fuzz/artifacts/<target>/, fails the
# recipe, and becomes a tests/regressions/issue_NNNN_*.rs test with its fix.
[positional-arguments]
fuzz target seconds="60":
    #!/usr/bin/env bash
    set -euo pipefail
    target="$1"
    seconds="$2"
    case "$target" in
        catalog_parse) max_len=1048576 timeout=2 ;;
        validate_component) max_len=262144 timeout=1 ;;
        *) echo "unknown fuzz target '$target': catalog_parse or validate_component" >&2; exit 2 ;;
    esac
    [[ "$seconds" =~ ^[0-9]+$ ]] || { echo "seconds must be a whole number" >&2; exit 2; }
    corpus="fuzz/corpus/$target"
    mkdir -p "$corpus" "fuzz/artifacts/$target"
    # One seed per catalog for catalog_parse, one per component instance for validate_component.
    python3 -I - "$target" "$corpus" <<'EOF'
    import glob, hashlib, json, pathlib, sys
    target, corpus = sys.argv[1], pathlib.Path(sys.argv[2])
    if target == "catalog_parse":
        files = glob.glob("vendor/a2ui/catalogs/*/v1/catalog.json")
        files += glob.glob("vendor/a2ui/conformance/test_data/catalogs/*.json")
        seeds = [pathlib.Path(f).read_bytes() for f in sorted(files)]
    else:
        seeds = []
        for f in sorted(glob.glob("vendor/a2ui/catalogs/basic/v1/examples/*.json")):
            for message in json.loads(pathlib.Path(f).read_text()).get("messages", []):
                for component in (message.get("updateComponents") or {}).get("components", []):
                    seeds.append(json.dumps(component, sort_keys=True).encode())
    for seed in seeds:
        (corpus / ("seed-" + hashlib.sha256(seed).hexdigest()[:16])).write_bytes(seed)
    print(f"{len(seeds)} seeds for {target}")
    EOF
    just fuzz-lockfile
    cd fuzz
    triple="$(rustc -vV | sed -n 's/^host: //p')"
    export CARGO_TARGET_DIR="$PWD/target"
    RUSTFLAGS="--cfg fuzzing -Cpasses=sancov-module -Cllvm-args=-sanitizer-coverage-level=4 \
        -Cllvm-args=-sanitizer-coverage-inline-8bit-counters \
        -Cllvm-args=-sanitizer-coverage-pc-table -Cllvm-args=-sanitizer-coverage-trace-compares \
        -Cdebug-assertions -Coverflow-checks" \
        cargo build --release --locked --target "$triple" --bin "$target"
    "target/$triple/release/$target" -max_total_time="$seconds" -max_len="$max_len" \
        -timeout="$timeout" -report_slow_units=1 -artifact_prefix="artifacts/$target/" \
        "corpus/$target"

# The fuzz workspace: formatted and lint-clean on its generated lock (fuzz-lockfile).
fuzz-check: fuzz-lockfile
    cargo fmt --manifest-path fuzz/Cargo.toml --check
    cargo clippy --manifest-path fuzz/Cargo.toml --locked --bins -- -D warnings

# Generates fuzz/Cargo.lock (not committed): Cargo.lock's packages, so fuzz/ builds the reviewed
# releases that ship and a root lock change needs nothing here, then fuzz/fuzz-only.lock's pins
# for what fuzz/ alone uses. Fails if fuzz/ needs a release neither holds (plan 2b, Decision 45).
fuzz-lockfile:
    python3 -B scripts/fuzz_lock.py assemble
    cargo metadata --manifest-path fuzz/Cargo.toml --format-version 1 > /dev/null
    python3 -B scripts/fuzz_lock.py verify

# Resolves what fuzz/ alone needs and rewrites fuzz/fuzz-only.lock, listing each release to review.
fuzz-lock:
    python3 -B scripts/fuzz_lock.py assemble
    cargo metadata --manifest-path fuzz/Cargo.toml --format-version 1 > /dev/null
    python3 -B scripts/fuzz_lock.py pin

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

# Prints workspace member `crate`'s version, as cargo packages it, from cargo metadata.
[private]
crate-version crate:
    #!/usr/bin/env bash
    set -euo pipefail
    crate={{ quote(crate) }}
    version="$(cargo metadata --no-deps --format-version 1 \
        | jq -r --arg name "$crate" '.packages[] | select(.name == $name) | .version')"
    # Empty for an unknown name, which jq does not fail on.
    if [ -z "$version" ]; then
        echo "no workspace member named $crate" >&2
        exit 1
    fi
    echo "$version"

# On the MSRV, a user's crate resolves the packaged libraries afresh: builds unreviewed releases.
msrv-resolved:
    #!/usr/bin/env bash
    set -euo pipefail
    # The pinned toolchain reads the versions and packages the crates (the MSRV's cargo cannot
    # package a crate whose sibling is not yet published).
    unset RUSTUP_TOOLCHAIN
    # Each crate's own version, as cargo packages it: a pre-release or a per-crate one included.
    runtime="$(just crate-version a2ui-runtime)"
    catalog="$(just crate-version a2ui-catalog-basic)"
    # Cargo.lock proves our versions only. A crate of its own, with no lock, no dev-dependencies
    # and no rust-version, resolves the newest versions this rustc accepts (resolver 3, as in an
    # edition 2024 crate), the way a user's does; its `[workspace]` keeps it out of ours. A
    # killed run leaves it under the ignored target/.
    mkdir -p target
    user="$(mktemp -d target/msrv-user.XXXXXX)"
    trap 'rm -rf "$user"' EXIT
    # It depends on the packages crates.io would get, unpacked, not on our sources: a file missing
    # from `include` fails here too; --no-verify, as nothing is built yet.
    cargo package -p a2ui-runtime -p a2ui-catalog-basic --locked --no-verify --allow-dirty \
        --target-dir "$PWD/$user/package"
    mkdir "$user/packages" "$user/src"
    for package in "a2ui-runtime-$runtime" "a2ui-catalog-basic-$catalog"; do
        tar -xzf "$user/package/package/$package.crate" -C "$user/packages"
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
    a2ui-runtime = "=$runtime"
    a2ui-catalog-basic = "=$catalog"

    [patch.crates-io]
    a2ui-runtime = { path = "packages/a2ui-runtime-$runtime" }
    a2ui-catalog-basic = { path = "packages/a2ui-catalog-basic-$catalog" }

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

# Our lower bounds, each direct dependency at its lowest allowed release, checked on the MSRV.
# Nightly cargo resolves them (-Z direct-minimal-versions), so it builds unreviewed releases.
minimal-versions: nightly-toolchain
    #!/usr/bin/env bash
    set -euo pipefail
    nightly={{ quote(nightly) }}
    # A copy of the workspace, so Cargo.lock stays ours; a killed run leaves it under target/.
    mkdir -p target
    copy="$(mktemp -d target/minimal-versions.XXXXXX)"
    trap 'rm -rf "$copy"' EXIT
    # The tracked files as the working tree holds them, symlinks kept.
    git ls-files -z | tar --null -T - -cf - | tar -xf - -C "$copy"
    rm "$copy/Cargo.lock"
    # The MSRV runs everything but the resolver, so the pinned toolchain is never installed.
    RUSTUP_TOOLCHAIN="$(just msrv-version)"
    export RUSTUP_TOOLCHAIN
    just msrv-toolchain
    # Only the resolver runs on the pinned nightly, installed by `nightly-toolchain` (`+` beats
    # RUSTUP_TOOLCHAIN): nothing builds with it.
    cargo "+$nightly" generate-lockfile --manifest-path "$copy/Cargo.toml" \
        -Z direct-minimal-versions
    # Resolving runs no crate's code: known advisories, known-malicious crates included, and
    # yanked releases stop the run before any build script does.
    cargo audit --file "$copy/Cargo.lock" --deny yanked
    # Its builds stay in the copy's own target/, gone with it, whatever the caller's setting.
    export CARGO_TARGET_DIR="$PWD/$copy/target"
    # The libraries' dependencies as resolved, for the log.
    cargo tree --manifest-path "$copy/Cargo.toml" --locked -p a2ui-runtime -p a2ui-catalog-basic \
        --target all -e normal,build --prefix none | sed -E 's/ \((\*|proc-macro)\)//g' | sort -u
    cargo check --manifest-path "$copy/Cargo.toml" --locked -p a2ui-runtime -p a2ui-catalog-basic
    cargo check --manifest-path "$copy/Cargo.toml" --locked -p a2ui-runtime -p a2ui-catalog-basic \
        --target wasm32-unknown-unknown

# Checks the pinned nightly is dated, installs it and logs its cargo's version.
[private]
nightly-toolchain:
    #!/usr/bin/env bash
    set -euo pipefail
    nightly={{ quote(nightly) }}
    # Plain `nightly` would float again, and Renovate bumps only a dated name.
    if ! [[ "$nightly" =~ ^nightly-[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
        echo "nightly must be a dated nightly-YYYY-MM-DD, found: $nightly" >&2
        exit 1
    fi
    rustup toolchain install "$nightly" --profile minimal
    cargo "+$nightly" -V

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

# Whether cargo-semver-checks reads this toolchain's rustdoc JSON: a crate with no dependency,
# checked against itself, so no API change and no third-party code can fail it, only the tool.
semver-checks-tool:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo semver-checks --version
    # Under target/, not $TMPDIR: rust-toolchain.toml applies only within the repository.
    mkdir -p target
    fixture="$(mktemp -d target/semver-tool.XXXXXX)"
    trap 'rm -rf "$fixture"' EXIT
    mkdir "$fixture/src"
    # Its own `[workspace]` keeps it out of ours; no dependency, so nothing is resolved or built
    # but this file.
    cat > "$fixture/Cargo.toml" <<'EOF'
    [package]
    name = "semver-tool-fixture"
    version = "0.1.0"
    edition = "2024"

    [workspace]
    EOF
    # Common item kinds; a format change limited to another kind passes here (plan 2b, Decision 31).
    cat > "$fixture/src/lib.rs" <<'EOF'
    //! A fixture.
    /// A struct.
    pub struct Point<T> { pub x: T, y: T }
    impl<T: Copy> Point<T> {
        pub fn new(x: T, y: T) -> Self { Self { x, y } }
        pub fn y(&self) -> T { self.y }
    }
    /// An enum.
    #[non_exhaustive]
    pub enum Shape { Unit, Tuple(u8), Named { side: u32 } }
    /// A trait.
    pub trait Area { type Out; const SIDES: u8; fn area(&self) -> Self::Out; }
    impl Area for Shape { type Out = u32; const SIDES: u8 = 4; fn area(&self) -> u32 { 0 } }
    /// A function.
    pub fn measure<'a, A: Area + ?Sized>(shape: &'a A) -> A::Out { shape.area() }
    /// A constant.
    pub const LIMIT: usize = 8;
    /// A module.
    pub mod nested { /// An alias.
        pub type Pair = (u8, u8); }
    EOF
    export CARGO_TARGET_DIR="$PWD/$fixture/target"
    cargo semver-checks --manifest-path "$fixture/Cargo.toml" --baseline-root "$fixture"

# Licenses, sources (crates.io only) and advisories, yanked releases included: it reads the lock
# alone, so `ci` runs it before anything builds.
audit:
    cargo deny check licenses sources
    cargo audit --deny yanked
    # fuzz/'s lock as assembled from Cargo.lock and the pins, by Python alone: no build script.
    python3 -B scripts/fuzz_lock.py assemble
    cargo audit --deny yanked --file fuzz/Cargo.lock

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
    # Nor does it ship tests/, which nothing builds from a package (trybuild's cases hold the
    # pinned rustc's wording; plan 2b, Decision 42).
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
        if grep '^tests/' <<< "$listed" >&2; then
            echo "$crate's package ships the files above from tests/" >&2
            exit 1
        fi
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
