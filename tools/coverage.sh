#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd)
tiqian_rs_root=$(cd "$script_dir/.." && pwd)
coverage_dir="$tiqian_rs_root/target/llvm-cov"

cd "$tiqian_rs_root"
cargo llvm-cov clean --workspace
eval "$(cargo llvm-cov show-env --export-prefix)"
RUSTFLAGS=${RUSTFLAGS:-"-A warnings"}
export RUSTFLAGS

cargo test --all-targets
bash "$script_dir/verify-all-fixtures.sh"
bash "$script_dir/verify-recorded-fixtures.sh"
cargo llvm-cov report --html --output-dir "$coverage_dir"

printf 'coverage report: %s/html/index.html\n' "$coverage_dir"