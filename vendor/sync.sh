#!/usr/bin/env bash
# Copies upstream A2UI v1.0 specification, catalogs and conformance suites at a pinned commit,
# plus the documents the crates embed (a crate cannot include files outside its directory).
set -euo pipefail
commit="${1:?usage: vendor/sync.sh <upstream-commit>}"
root="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$root/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone --quiet --filter=blob:none --no-checkout https://github.com/a2ui-project/a2ui.git "$work/a2ui"
git -C "$work/a2ui" sparse-checkout set --no-cone \
  /LICENSE /NOTICE /specification/v1_0 \
  /catalogs/basic/v1 /catalogs/mcp/v1 /catalogs/vegachart/v1 \
  /conformance/core /conformance/test_data /conformance/README.md \
  /conformance/conformance_schema.json /conformance/executed_case_manifest.schema.json \
  /conformance/manifest_corpora.json /conformance/conformance_harness_implementation_guide.md
git -C "$work/a2ui" checkout --quiet "$commit"
rm -rf "$root/a2ui"
mkdir -p "$root/a2ui/specification" "$root/a2ui/catalogs/basic" "$root/a2ui/catalogs/mcp" \
  "$root/a2ui/catalogs/vegachart"
cp "$work/a2ui/LICENSE" "$root/a2ui/LICENSE"
if [[ -f "$work/a2ui/NOTICE" ]]; then
  cp "$work/a2ui/NOTICE" "$root/a2ui/NOTICE"
fi
cp -R "$work/a2ui/specification/v1_0" "$root/a2ui/specification/v1_0"
cp -R "$work/a2ui/catalogs/basic/v1" "$root/a2ui/catalogs/basic/v1"
cp -R "$work/a2ui/catalogs/mcp/v1" "$root/a2ui/catalogs/mcp/v1"
cp -R "$work/a2ui/catalogs/vegachart/v1" "$root/a2ui/catalogs/vegachart/v1"
cp -R "$work/a2ui/conformance" "$root/a2ui/conformance"
schemas="$repo/crates/a2ui-runtime/schemas/v1_0"
mkdir -p "$schemas"
cp "$work/a2ui/specification/v1_0/json/common_types.json" \
  "$work/a2ui/specification/v1_0/json/agent_to_renderer.json" "$schemas/"
cp "$work/a2ui/LICENSE" "$schemas/LICENSE"
rm -f "$schemas/NOTICE-A2UI"
if [[ -f "$work/a2ui/NOTICE" ]]; then
  cp "$work/a2ui/NOTICE" "$schemas/NOTICE-A2UI"
fi
basic="$repo/crates/a2ui-catalog-basic"
mkdir -p "$basic"
cp "$work/a2ui/catalogs/basic/v1/catalog.json" "$basic/catalog.json"
cp "$work/a2ui/LICENSE" "$basic/LICENSE-A2UI"
rm -f "$basic/NOTICE-A2UI"
if [[ -f "$work/a2ui/NOTICE" ]]; then
  cp "$work/a2ui/NOTICE" "$basic/NOTICE-A2UI"
fi
printf '%s\n' "$commit" > "$root/UPSTREAM"
