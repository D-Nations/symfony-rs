#!/usr/bin/env bash
# The M1-D converter fixture on Linux from an empty directory (PLAN.md,
# "Early conversion proof"), run locally: the owner wants no paid CI jobs.
#
#   scripts/converter-fixture.sh
#
# The committed tree (`git archive HEAD`, so uncommitted edits are not
# seen) is unpacked into an empty directory of a removed Rust container,
# pinned by digest, which downloads the locked crates itself and runs the
# fixture's format, lint and test checks with the crates it links. The PHP
# recording is part of the tree (scripts/record.sh converter-fixture).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export MSYS_NO_PATHCONV=1

# rust:1.99-slim, the host's toolchain at the time (1.99.1)
image=rust@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7
packages=(-p converter-fixture-app -p symfony-http-kernel -p symfony-http-foundation -p symfony-routing -p symfony-runtime -p symfony-event-dispatcher -p psr-container)

echo "converter fixture at $(git rev-parse --short HEAD) in an empty Linux directory"
git archive --format=tar HEAD | docker run --rm -i "$image" bash -euo pipefail -c "
    rustup component add rustfmt clippy >/dev/null 2>&1
    mkdir /work && cd /work && tar -x
    test -z \"\$(ls -A target 2>/dev/null)\"
    cargo fmt --check ${packages[*]}
    cargo clippy --locked --all-targets ${packages[*]} -- -D warnings
    cargo test --locked ${packages[*]}
"
