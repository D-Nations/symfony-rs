#!/usr/bin/env bash
# Record test vectors from the PHP oracle (oracle/, Docker) into the crates.
#
#   scripts/record.sh routing      crates/routing/tests/data/matcher.json
#   scripts/record.sh multipart    crates/http-foundation/tests/data/multipart_vectors.json
#   scripts/record.sh event-dispatcher  crates/event-dispatcher/tests/data/edges.txt
#   scripts/record.sh all
#
# The oracle loads the pinned packages from ../symfony-demo/vendor
# (verify with scripts/fetch-upstream.sh --verify ../symfony-demo/vendor).
# After recording, `git diff` shows whether PHP's answers changed.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export MSYS_NO_PATHCONV=1

oracle() { docker compose -f oracle/compose.yaml run --rm -T php "$@"; }

routing() {
    local out=crates/routing/tests/data/matcher.json tmp
    mkdir -p oracle/out
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/routing_matcher.php > "$tmp"
    local count
    count=$(python -c 'import json,sys; print(len(json.load(open(sys.argv[1]))))' "$tmp")
    if [[ "$count" != 90 ]]; then
        echo "expected 90 routing cases, got $count" >&2; rm -f "$tmp"; exit 1
    fi
    mv "$tmp" "$out"
    echo "$count Symfony Routing cases written to $out"
}

multipart() { python oracle/recorders/multipart.py; }

event_dispatcher() {
    local out=crates/event-dispatcher/tests/data/edges.txt tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/event_dispatcher_edges.php > "$tmp"
    mv "$tmp" "$out"
    echo "$(wc -l < "$out") EventDispatcher lines written to $out"
}

case "${1:-}" in
    routing) routing ;;
    multipart) multipart ;;
    event-dispatcher) event_dispatcher ;;
    all) routing; multipart; event_dispatcher ;;
    *) echo "usage: scripts/record.sh routing|multipart|event-dispatcher|all" >&2; exit 2 ;;
esac
