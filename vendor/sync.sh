#!/usr/bin/env bash
# Copies upstream A2UI v1.0 specification, basic catalog and conformance suites at a pinned commit.
set -euo pipefail
commit="${1:?usage: vendor/sync.sh <upstream-commit>}"
root="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone --quiet --filter=blob:none --no-checkout https://github.com/a2ui-project/a2ui.git "$work/a2ui"
git -C "$work/a2ui" sparse-checkout set --no-cone \
  /LICENSE /specification/v1_0 /catalogs/basic/v1 \
  /conformance/core /conformance/test_data /conformance/README.md \
  /conformance/conformance_schema.json /conformance/executed_case_manifest.schema.json \
  /conformance/manifest_corpora.json /conformance/conformance_harness_implementation_guide.md
git -C "$work/a2ui" checkout --quiet "$commit"
rm -rf "$root/a2ui"
mkdir -p "$root/a2ui/specification" "$root/a2ui/catalogs/basic"
cp "$work/a2ui/LICENSE" "$root/a2ui/LICENSE"
cp -R "$work/a2ui/specification/v1_0" "$root/a2ui/specification/v1_0"
cp -R "$work/a2ui/catalogs/basic/v1" "$root/a2ui/catalogs/basic/v1"
cp -R "$work/a2ui/conformance" "$root/a2ui/conformance"
printf '%s\n' "$commit" > "$root/UPSTREAM"
