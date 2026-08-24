#!/usr/bin/env bash
set -euo pipefail

fixture_id=${1:?usage: verify-fixture.sh <fixture-id>}
tiqian_rs_root=$(cd "$(dirname "$0")/.." && pwd)
tiqian_root=${TIQIAN_ROOT:-"$tiqian_rs_root/../tiqian"}
golden="$tiqian_root/engine/src/jvmTest/resources/golden/layout-dumps/$fixture_id.txt"

[[ -f "$golden" ]] || { printf 'missing Tiqian golden: %s\n' "$golden" >&2; exit 2; }
actual=$(mktemp)
trap 'rm -f "$actual"' EXIT

(
    cd "$tiqian_root"
    ./gradlew -q :engine:exportLayoutFixture -PfixtureId="$fixture_id"
) | (
    cd "$tiqian_rs_root"
    cargo run --quiet --bin fixture_layout_dump
) > "$actual"

if diff -u "$golden" "$actual"; then
    printf 'fixture %s: golden matched\n' "$fixture_id"
else
    awk 'FNR == NR { expected[FNR] = $0; expected_count = FNR; next } { actual_count = FNR; if (!reported && (FNR > expected_count || $0 != expected[FNR])) { printf "first differing dump line %d:\n  golden: %s\n  rust:   %s\n", FNR, (FNR > expected_count ? "<missing>" : expected[FNR]), $0 > "/dev/stderr"; reported = 1 } } END { if (!reported && actual_count < expected_count) printf "first differing dump line %d:\n  golden: %s\n  rust:   <missing>\n", actual_count + 1, expected[actual_count + 1] > "/dev/stderr" }' "$golden" "$actual"
    exit 1
fi