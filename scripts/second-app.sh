#!/usr/bin/env bash
# Builds the M1-D second application (PLAN.md decision 10) for recording,
# from an empty directory: oracle/out/apps/easyadmin-demo.
#
#   scripts/second-app.sh install          copy the pin, apply the patch, composer install
#   scripts/second-app.sh database pinned  the var/data.db the pin ships (its demo data)
#   scripts/second-app.sh database empty   schema only, no rows
#
# The pinned checkout (upstream/apps/easyadmin-demo, from upstream.tsv) is
# never touched: the app is a `git archive` of it with
# oracle/apps/easyadmin-demo/prod-boot.patch applied and nothing else.
# Composer runs in its official image, pinned by digest, downloading the
# exact lock without running scripts. PHP runs only in the oracle image.
# The schema is created with APP_ENV=dev (the demo's `make fixtures`);
# recordings serve APP_ENV=prod. The demo's Foundry stories are not used:
# they are not seeded, so their rows would differ on every run.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export MSYS_NO_PATHCONV=1

pin=upstream/apps/easyadmin-demo
app=oracle/out/apps/easyadmin-demo
patch=oracle/apps/easyadmin-demo/prod-boot.patch
composer_image=composer@sha256:0c8e663f14c94d07b3c45b175c51f5a14718508f21fe4dd7e54f09488dd0ed71
oracle_image=symfony-rs-oracle:8.4.27

host_path() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }

console() { # env console-args...
    local env=$1
    shift
    docker run --rm -e APP_ENV="$env" -e APP_DEBUG=0 -e APP_SECRET=oracle-only \
        -v "$(host_path "$root/$app"):/app" -w /app --entrypoint php "$oracle_image" bin/console "$@"
}

install() {
    [[ -d "$pin/.git" ]] || scripts/fetch-upstream.sh apps/easyadmin-demo
    rm -rf "$app"
    mkdir -p "$app"
    git -C "$pin" archive --format=tar HEAD | tar -x -C "$app"
    # a repository of its own, so git apply resolves paths inside the app
    # (no CRLF conversion: the files stay byte for byte as pinned)
    git -C "$app" init -q
    (cd "$app" && git -c core.autocrlf=false apply -p1 "$(host_path "$root/$patch")")
    rm -rf "$app/.git"
    docker run --rm -v "$(host_path "$root/$app"):/app" -w /app "$composer_image" \
        install --no-scripts --no-interaction --no-progress --ignore-platform-reqs --quiet
    echo "installed $(git -C "$pin" rev-parse HEAD) with $(basename "$patch") into $app"
}

database() {
    local kind=${1:?usage: database pinned|empty}
    rm -f "$app/var/data.db"
    rm -rf "$app/var/cache"
    case "$kind" in
        pinned) git -C "$pin" show HEAD:var/data.db > "$app/var/data.db" ;;
        empty) console dev doctrine:schema:create --no-interaction >/dev/null 2>&1 ;;
        *) echo "usage: database pinned|empty" >&2; exit 2 ;;
    esac
    rm -rf "$app/var/cache"
    echo "database: $kind"
}

case "${1:-}" in
    install) install ;;
    database) database "${2:-}" ;;
    *) echo "usage: scripts/second-app.sh install | database pinned|empty" >&2; exit 2 ;;
esac
