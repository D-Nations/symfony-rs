#!/usr/bin/env bash
# Fresh, unprivileged Linux acceptance checks of committed HEAD.
#   scripts/linux-test.sh [-p CRATE ...]
#   scripts/linux-test.sh --cached [-p CRATE ...]  # development only
#
# Tool installation is cached in a digest-pinned image. Default runs keep
# Cargo downloads and build artifacts ephemeral. --cached opts into named
# volumes per worktree; it never replaces a fresh pre-merge run.
# Sources are always a git archive; upstream is mounted read-only.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export MSYS_NO_PATHCONV=1

args=()
cached=false
while (( $# )); do
    case "$1" in
        --cached) cached=true; shift ;;
        -p)
            if [[ $# -lt 2 || ! "$2" =~ ^[a-zA-Z0-9_-]+$ ]]; then
                echo "usage: scripts/linux-test.sh [--cached] [-p CRATE ...]" >&2
                exit 2
            fi
            args+=("-p" "$2")
            shift 2
            ;;
        *)
            echo "usage: scripts/linux-test.sh [--cached] [-p CRATE ...]" >&2
            exit 2
            ;;
    esac
done
if (( ${#args[@]} == 0 )); then args=(--workspace); fi

mount=()
if [ -d upstream ]; then
    host=$(cd upstream && (pwd -W 2>/dev/null || pwd))
    mount=(-v "$host:/upstream:ro")
fi

# Content-address the recipe so concurrent worktrees cannot run each
# other's tag after a build. Only this tiny directory is sent to Docker.
recipe=$(cat scripts/linux-check/Dockerfile scripts/linux-check/run.sh | git hash-object --stdin)
image="symfony-rs-linux-check:${recipe:0:12}"
docker build --quiet --tag "$image" scripts/linux-check >/dev/null

mode=fresh
if "$cached"; then
    mode="cached development"
    key=$(printf '%s' "$root" | git hash-object --stdin)
    prefix="symfony-rs-${recipe:0:12}-${key:0:12}"
    mount+=(
        --mount "type=volume,source=$prefix-cargo,target=/cache/cargo"
        --mount "type=volume,source=$prefix-target,target=/cache/target"
        -e CARGO_TARGET_DIR=/cache/target
    )
    echo "Development cache volumes: $prefix-cargo $prefix-target"
fi
echo "linux checks at $(git rev-parse --short HEAD) ($mode): ${args[*]}"
git archive --format=tar HEAD | docker run --rm -i "${mount[@]}" "$image" "${args[@]}"
