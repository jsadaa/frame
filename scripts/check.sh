#!/usr/bin/env bash

set -euo pipefail

echo "==> Checking formatting"
cargo fmt --all --check

echo "==> Running Clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> Running tests"
cargo test --workspace
