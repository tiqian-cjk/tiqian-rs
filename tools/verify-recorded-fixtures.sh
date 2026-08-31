#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd)
tiqian_rs_root=$(cd "$script_dir/.." && pwd)
tiqian_root=${TIQIAN_ROOT:-"$tiqian_rs_root/../tiqian"}
evidence="$tiqian_root/engine/src/jvmTest/resources/golden/shaping-evidence.json"
golden_dir="$tiqian_root/engine/src/jvmTest/resources/golden/layout-dumps-recorded"

[[ -f "$evidence" ]] || { printf 'missing Tiqian shaping evidence: %s\n' "$evidence" >&2; exit 2; }
[[ -d "$golden_dir" ]] || { printf 'missing Tiqian recorded golden directory: %s\n' "$golden_dir" >&2; exit 2; }

count=0
failures=0
failed_fixture_ids=()
for golden in "$golden_dir"/*.txt; do
    fixture_id=$(basename "$golden" .txt)
    actual=$(mktemp)
    trap 'rm -f "$actual"' EXIT

    (
        cd "$tiqian_root"
        ./gradlew -q :engine:exportLayoutFixture -PfixtureId="$fixture_id"
    ) | (
        cd "$tiqian_rs_root"
        TIQIAN_SHAPING_EVIDENCE="$evidence" RUSTFLAGS="-A warnings" cargo run --quiet --example fixture-layout-dump
    ) > "$actual"

    if diff -u "$golden" "$actual"; then
        printf 'recorded fixture %s: golden matched\n' "$fixture_id"
    else
        awk 'FNR == NR { expected[FNR] = $0; expected_count = FNR; next } { actual_count = FNR; if (!reported && (FNR > expected_count || $0 != expected[FNR])) { printf "first differing recorded dump line %d:\n  golden: %s\n  rust:   %s\n", FNR, (FNR > expected_count ? "<missing>" : expected[FNR]), $0 > "/dev/stderr"; reported = 1 } } END { if (!reported && actual_count < expected_count) printf "first differing recorded dump line %d:\n  golden: %s\n  rust:   <missing>\n", actual_count + 1, expected[actual_count + 1] > "/dev/stderr" }' "$golden" "$actual"
        printf 'recorded fixture %s: golden differed\n' "$fixture_id" >&2
        failures=$((failures + 1))
        failed_fixture_ids+=("$fixture_id")
    fi
    rm -f "$actual"
    trap - EXIT
    count=$((count + 1))
done

if [[ $failures -gt 0 ]]; then
    printf '%d recorded fixtures differed: %s\n' "$failures" "${failed_fixture_ids[*]}" >&2
    exit 1
fi
printf 'all %d recorded fixtures: golden matched\n' "$count"
