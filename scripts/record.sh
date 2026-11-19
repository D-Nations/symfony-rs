#!/usr/bin/env bash
# Record test vectors from the PHP oracle (oracle/, Docker) into the crates.
#
#   scripts/record.sh routing      crates/routing/tests/data/matcher.json
#   scripts/record.sh multipart    crates/http-foundation/tests/data/multipart_vectors.json
#   scripts/record.sh request-vars crates/http-foundation/tests/data/{query,cookie,content_type}_vectors.json
#   scripts/record.sh second-app-w1 oracle/apps/easyadmin-demo/w1.json (second app, W1)
#   scripts/record.sh event-dispatcher  crates/event-dispatcher/tests/data/edges.txt
#   scripts/record.sh stopwatch    crates/stopwatch/tests/data/php.json
#   scripts/record.sh php-value    crates/php-value/tests/data/php.json
#   scripts/record.sh yaml         crates/yaml/tests/data/*.json
#   scripts/record.sh dotenv       crates/dotenv/tests/data/*.json
#   scripts/record.sh config       crates/config/tests/data/definition.json, builder.json
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

request_vars() { python oracle/recorders/request_vars.py; }

second_app_w1() {
    [[ -d oracle/out/apps/easyadmin-demo/vendor ]] || scripts/second-app.sh install
    # the recorder calls scripts/second-app.sh with this same bash
    SECOND_APP_BASH=$(cygpath -w "$BASH" 2>/dev/null || printf '%s' "$BASH") \
        python oracle/recorders/second_app_w1.py
}

event_dispatcher() {
    local out=crates/event-dispatcher/tests/data/edges.txt tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/event_dispatcher_edges.php > "$tmp"
    mv "$tmp" "$out"
    echo "$(wc -l < "$out") EventDispatcher lines written to $out"
}

stopwatch() {
    local out=crates/stopwatch/tests/data/php.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/stopwatch.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Stopwatch vectors written to $out"
}

php_value() {
    local out=crates/php-value/tests/data/php.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/php_value.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "PHP value vectors written to $out"
}

yaml() {
    local name out tmp
    mkdir -p oracle/out crates/yaml/tests/data
    for name in escaping timestamps; do
        out=crates/yaml/tests/data/$name.json
        tmp=$(mktemp -p oracle/out)
        oracle "oracle/recorders/yaml_$name.php" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
        mv "$tmp" "$out"
        echo "Yaml $name vectors written to $out"
    done
    for class in InlineTest; do
        name=$(echo "$class" | sed 's/Test$//' | tr '[:upper:]' '[:lower:]')
        out=crates/yaml/tests/data/${name}_test.json
        tmp=$(mktemp -p oracle/out)
        oracle oracle/recorders/yaml_test_providers.php "$class" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
        mv "$tmp" "$out"
        echo "Yaml $class providers written to $out"
    done
    # YamlTestSuiteTest needs the suite: scripts/fetch-upstream.sh
    for class in ParserTest DumperTest YamlTest YamlTestSuiteTest; do
        name=$(echo "$class" | sed -E 's/([a-z])([A-Z])/\1_\2/g' | tr '[:upper:]' '[:lower:]')
        out=crates/yaml/tests/data/${name}.json
        tmp=$(mktemp -p oracle/out)
        oracle oracle/recorders/yaml_test_calls.php "$class" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
        mv "$tmp" "$out"
        echo "Yaml $class calls written to $out"
    done
}

dotenv() {
    local out tmp
    mkdir -p oracle/out crates/dotenv/tests/data
    out=crates/dotenv/tests/data/dotenv_test.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/test_providers.php symfony/dotenv DotenvTest > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Dotenv DotenvTest providers written to $out"
    out=crates/dotenv/tests/data/parse.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/dotenv_parse.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Dotenv parse vectors written to $out"
}

config() {
    local out=crates/config/tests/data/definition.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/config_definition.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Config Definition vectors written to $out"
    out=crates/config/tests/data/builder.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/config_builder.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Config Builder vectors written to $out"
}

case "${1:-}" in
    routing) routing ;;
    multipart) multipart ;;
    request-vars) request_vars ;;
    second-app-w1) second_app_w1 ;;
    event-dispatcher) event_dispatcher ;;
    stopwatch) stopwatch ;;
    php-value) php_value ;;
    yaml) yaml ;;
    dotenv) dotenv ;;
    config) config ;;
    all) routing; multipart; request_vars; event_dispatcher; stopwatch; php_value; yaml; dotenv; config ;;
    *) echo "usage: scripts/record.sh routing|multipart|request-vars|second-app-w1|event-dispatcher|stopwatch|php-value|yaml|dotenv|config|all" >&2; exit 2 ;;
esac
