#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

cargo fmt --all -- --check
./scripts/check-forbidden.sh
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo doc --no-deps
