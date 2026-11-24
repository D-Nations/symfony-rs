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
#
# The checks run as an unprivileged user (`check`), not root: root reads a
# file whatever its mode, so tests of unreadable files (chmod 0) would
# fail under root (owner, November 24). Only installing rustfmt and
# clippy into the image's toolchain runs as root.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export MSYS_NO_PATHCONV=1

# rust:1.99-slim, as scripts/converter-fixture.sh
image=rust@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7
args=()
while (( $# )); do
    if [[ "$1" != "-p" || $# -lt 2 || ! "$2" =~ ^[a-zA-Z0-9_-]+$ ]]; then
        echo "usage: scripts/linux-test.sh [-p CRATE ...]" >&2
        exit 2
    fi
    args+=("-p" "$2")
    shift 2
done
if (( ${#args[@]} == 0 )); then
    args=(--workspace)
fi
mount=()
if [ -d upstream ]; then
    host=$(cd upstream && (pwd -W 2>/dev/null || pwd))
    mount=(-v "$host:/upstream:ro")
fi

echo "linux checks at $(git rev-parse --short HEAD): ${args[*]}"
git archive --format=tar HEAD | docker run --rm -i "${mount[@]}" "$image" bash -euo pipefail -c "
    rustup component add rustfmt clippy >/dev/null 2>&1
    useradd --create-home --uid 1000 check
    mkdir /work && chown check:check /work
    runuser -u check -- env HOME=/home/check CARGO_HOME=/home/check/.cargo \
        RUSTUP_HOME=\"\$RUSTUP_HOME\" PATH=\"\$PATH\" bash -euo pipefail -c '
        test \"\$(id -u)\" != 0
        cd /work && tar -x
        test -z \"\$(ls -A target 2>/dev/null)\"
        if [ -d /upstream ]; then ln -s /upstream upstream; fi
        if [[ "\$1" == "--workspace" ]]; then cargo fmt --all --check; else cargo fmt --check "\$@"; fi
        cargo clippy --locked --all-targets "\$@" -- -D warnings
        cargo test --locked "\$@" --no-fail-fast
    ' -- "\$@"
" -- "${args[@]}"
