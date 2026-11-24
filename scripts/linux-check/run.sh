#!/usr/bin/env bash
set -euo pipefail
test "$(id -u)" != 0
tar -x
test ! -e target
if [ -d /upstream ]; then ln -s /upstream upstream; fi
if [[ "${1:-}" == --workspace ]]; then
    cargo fmt --all --check
else
    cargo fmt --check "$@"
fi
cargo clippy --locked --all-targets "$@" -- -D warnings
cargo test --locked "$@" --no-fail-fast
