#!/usr/bin/env bash
# cargo fmt, clippy and test on Linux from an empty directory, run locally
# (the owner wants no paid CI jobs). For tests that only run on Unix, such
# as the recorded translation replays, when the host is Windows.
#
#   scripts/linux-test.sh                 the whole workspace
#   scripts/linux-test.sh -p CRATE...     these packages only
#
# The committed tree (`git archive HEAD`, so uncommitted edits are not
# seen) is unpacked into an empty directory of a removed Rust container,
# pinned by digest, which downloads the locked crates itself. The ignored
# upstream/ checkout, which some tests read, is mounted read-only.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export MSYS_NO_PATHCONV=1

# rust:1.99-slim, as scripts/converter-fixture.sh
image=rust@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7
args="${*:-}"
mount=()
if [ -d upstream ]; then
    host=$(cd upstream && (pwd -W 2>/dev/null || pwd))
    mount=(-v "$host:/upstream:ro")
fi

echo "linux checks at $(git rev-parse --short HEAD): ${args:-workspace}"
git archive --format=tar HEAD | docker run --rm -i "${mount[@]}" "$image" bash -euo pipefail -c "
    rustup component add rustfmt clippy >/dev/null 2>&1
    mkdir /work && cd /work && tar -x
    test -z \"\$(ls -A target 2>/dev/null)\"
    if [ -d /upstream ]; then ln -s /upstream upstream; fi
    cargo fmt --check $args
    cargo clippy --locked --all-targets $args -- -D warnings
    cargo test --locked $args
"
