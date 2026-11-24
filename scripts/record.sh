#!/usr/bin/env bash
# Record test vectors from the PHP oracle (oracle/, Docker) into the crates.
#
#   scripts/record.sh routing      crates/routing/tests/data/matcher.json and route.json
#   scripts/record.sh routing-route crates/routing/tests/data/route.json (Route, RouteCompiler, RouteCollection)
#   scripts/record.sh routing-request-context crates/routing/tests/data/request_context.json
#   scripts/record.sh routing-url-generator crates/routing/tests/data/url_generator.json (UrlGenerator)
#   scripts/record.sh multipart    crates/http-foundation/tests/data/multipart_vectors.json
#   scripts/record.sh request-vars crates/http-foundation/tests/data/{query,cookie,content_type}_vectors.json
#   scripts/record.sh second-app-w1 oracle/apps/easyadmin-demo/w1.json (second app, W1)
#   scripts/record.sh converter-fixture oracle/apps/converter-fixture/responses.json (M1-D fixture)
#   scripts/record.sh event-dispatcher  crates/event-dispatcher/tests/data/edges.txt
#   scripts/record.sh stopwatch    crates/stopwatch/tests/data/php.json
#   scripts/record.sh php-value    crates/php-value/tests/data/{php,filter,parse_str,parse_url,loose_equals,http_build_query}.json
#   scripts/record.sh yaml         crates/yaml/tests/data/*.json (lint:yaml included)
#   scripts/record.sh dotenv       crates/dotenv/tests/data/*.json (commands included)
#   scripts/record.sh config       crates/config/tests/data/definition.json, builder.json, dumpers.json, xml_utils.json, loaders.json
#   scripts/record.sh config-xml-utils crates/config/tests/data/xml_utils.json
#   scripts/record.sh config-loaders crates/config/tests/data/loaders.json
#   scripts/record.sh config-cache crates/config/tests/data/cache.json (Filesystem calls included)
#   scripts/record.sh translation-xliff crates/translation/tests/data/xliff.json
#   scripts/record.sh translation-catalogue crates/translation/tests/data/catalogue.json
#   scripts/record.sh translation-qt crates/translation/tests/data/qt.json
#   scripts/record.sh console      crates/console/tests/data/{strip_tags,builtins,table_test,table_vectors}.json
#   scripts/record.sh string       crates/string/tests/data/{unicode,wordwrap,builtins,case,inflector,lazy,*_test}.*
#   scripts/record.sh css-selector crates/css-selector/tests/data/php.json
#   scripts/record.sh options-resolver crates/options-resolver/tests/data/*.json
#   scripts/record.sh finder       crates/finder/tests/data/{providers,php}.json
#   scripts/record.sh php-dom      crates/php-dom/tests/data/parse.json
#   scripts/record.sh php-dom-schema crates/php-dom/tests/data/schema.json
#   scripts/record.sh http-foundation crates/http-foundation/tests/data/{bags,header_utils,cookie,header_bags,server_bag,ip_utils,accept_header}.json, crates/php-value/tests/data/dates.json
#   scripts/record.sh http-foundation-files crates/http-foundation/tests/data/files.json (UploadedFile, FileBag)
#   scripts/record.sh mime-fileinfo crates/mime/tests/data/fileinfo.json (FileinfoMimeTypeGuesser, needs upstream/)
#   scripts/record.sh http-foundation-releases oracle/out/{ip,header}_utils_<tag>.json (earlier releases, advisory checks)
#   scripts/record.sh twig-trial   spikes/twig-trial/demo/tests/data/php.json (Twig trial; not in "all")
#   scripts/record.sh di-trial     spikes/di-trial/demo/data/container.xml (the demo's compiled container), demo/tests/data/php_classes.json
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

routing_route() {
    local out=crates/routing/tests/data/route.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/routing_route.php > "$tmp"
    mv "$tmp" "$out"
    echo "Route, RouteCompiler and RouteCollection answers written to $out"
}

routing_request_context() {
    local out=crates/routing/tests/data/request_context.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/routing_request_context.php > "$tmp"
    mv "$tmp" "$out"
    echo "RequestContext answers written to $out"
}

routing_url_generator() {
    local out=crates/routing/tests/data/url_generator.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/routing_url_generator.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["generate"]), "generate() calls,", len(d["relative"]), "getRelativePath() calls")' "$tmp"
    mv "$tmp" "$out"
    echo "UrlGenerator answers written to $out"
}

multipart() { python oracle/recorders/multipart.py; }

request_vars() { python oracle/recorders/request_vars.py; }

second_app_w1() {
    [[ -d oracle/out/apps/easyadmin-demo/vendor ]] || scripts/second-app.sh install
    # the recorder calls scripts/second-app.sh with this same bash
    SECOND_APP_BASH=$(cygpath -w "$BASH" 2>/dev/null || printf '%s' "$BASH") \
        python oracle/recorders/second_app_w1.py
}

converter_fixture() { python oracle/recorders/converter_fixture.py; }

di_trial() {
    local out=spikes/di-trial/demo/data/container.xml tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    # /demo is read-only; the kernel writes its cache to a tmpfs on /demo/var.
    docker compose -f oracle/compose.yaml run --rm -T --tmpfs /demo/var php \
        oracle/recorders/di_container.php > "$tmp"
    mv "$tmp" "$out"
    echo "$(grep -c '<service ' "$out") service definitions written to $out"
    out=spikes/di-trial/demo/tests/data/php_classes.json
    mkdir -p "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    docker compose -f oracle/compose.yaml run --rm -T --tmpfs /demo/var php \
        oracle/recorders/di_container.php classes > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "public service types written to $out"
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
    out=crates/php-value/tests/data/filter.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/filter_var.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["cases"]), "filter_var() cases")' "$tmp"
    mv "$tmp" "$out"
    echo "filter_var() vectors written to $out"
    out=crates/php-value/tests/data/parse_str.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/php_value_parse_str.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1]))), "parse_str() cases")' "$tmp"
    mv "$tmp" "$out"
    echo "parse_str() vectors written to $out"
    out=crates/php-value/tests/data/parse_url.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/php_value_parse_url.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1]))), "parse_url() cases")' "$tmp"
    mv "$tmp" "$out"
    echo "parse_url() vectors written to $out"
    out=crates/php-value/tests/data/loose_equals.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/php_value_loose_equals.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1]))), "== pairs")' "$tmp"
    mv "$tmp" "$out"
    echo "loose equality vectors written to $out"
    out=crates/php-value/tests/data/http_build_query.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/php_value_http_build_query.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1]))), "http_build_query() cases")' "$tmp"
    mv "$tmp" "$out"
    echo "http_build_query() vectors written to $out"
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
    out=crates/yaml/tests/data/lint_command.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/yaml_lint.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "Yaml lint:yaml outputs written to $out"
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
    out=crates/dotenv/tests/data/commands.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/dotenv_commands.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "Dotenv command outputs written to $out"
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
    out=crates/config/tests/data/dumpers.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/config_dumpers.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "Config dumper vectors written to $out"
    config_xml_utils
    config_loaders
}

config_loaders() {
    local out=crates/config/tests/data/loaders.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/config_loaders.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "Config loader and resource vectors written to $out"
}

config_xml_utils() {
    local out=crates/config/tests/data/xml_utils.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/config_xml_utils.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "Config XmlUtils vectors written to $out"
}

translation_xliff() {
    local out=crates/translation/tests/data/xliff.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/translation_xliff.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1], encoding="utf-8"))["cases"]), "load() cases")' "$tmp"
    mv "$tmp" "$out"
    echo "Translation XLIFF vectors written to $out"
}

config_cache() {
    local out=crates/config/tests/data/cache.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/config_cache.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(len(d["write"]), "writes,", len(d["fresh"]), "isFresh() calls,", len(d["filesystem"]), "Filesystem calls")' "$tmp"
    mv "$tmp" "$out"
    echo "Config cache vectors written to $out"
}

translation_catalogue() {
    local out=crates/translation/tests/data/catalogue.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/translation_catalogue.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1], encoding="utf-8"))["cases"]), "catalogue cases")' "$tmp"
    mv "$tmp" "$out"
    echo "Translation catalogue vectors written to $out"
    out=crates/translation/tests/data/catalogue_no_intl.json
    tmp=$(mktemp -p oracle/out)
    oracle -n oracle/recorders/translation_catalogue.php --without-intl > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); assert not d["intl"]; print(len(d["cases"]), "catalogue cases without intl")' "$tmp"
    mv "$tmp" "$out"
    echo "Translation catalogue vectors without intl written to $out"
}

translation_qt() {
    local out=crates/translation/tests/data/qt.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/translation_qt.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(len(d["cases"]), "load() cases,", len(d["evaluate"]), "evaluate() cases")' "$tmp"
    mv "$tmp" "$out"
    echo "Translation Qt vectors written to $out"
}

console() {
    local out=crates/console/tests/data/strip_tags.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/console_strip_tags.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Console strip_tags vectors written to $out"
    out=crates/console/tests/data/builtins.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/console_builtins.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Console built-in and find() vectors written to $out"
    out=crates/console/tests/data/table_test.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/test_providers.php symfony/console Helper/TableTest > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Console TableTest providers written to $out"
    out=crates/console/tests/data/table_vectors.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/console_table.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Console Table vectors written to $out"
}

string() {
    local out tmp name
    mkdir -p oracle/out crates/string/tests/data
    for name in unicode wordwrap; do
        out=crates/string/tests/data/$name.json
        tmp=$(mktemp -p oracle/out)
        oracle "oracle/recorders/string_$name.php" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
        mv "$tmp" "$out"
        echo "String $name vectors written to $out"
    done
    out=crates/string/tests/data/builtins.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/string_builtins.php > "$tmp" 2>/dev/null
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "String built-in calls written to $out"
    out=crates/string/tests/data/case.tsv
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/string_case.php > "$tmp"
    grep -q '^# PHP ' "$tmp"
    mv "$tmp" "$out"
    echo "String case mappings written to $out"
    python scripts/gen-string-tables.py
    local class file
    for class in ByteStringTest CodePointStringTest UnicodeStringTest; do
        file=$(printf '%s' "$class" | sed 's/\([a-z]\)\([A-Z]\)/\1_\2/g' | tr 'A-Z' 'a-z')
        out=crates/string/tests/data/$file.json
        tmp=$(mktemp -p oracle/out)
        oracle oracle/recorders/test_providers.php symfony/string "$class" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
        mv "$tmp" "$out"
        echo "String $class providers written to $out"
    done
    for class in EnglishInflectorTest FrenchInflectorTest SpanishInflectorTest; do
        file=$(printf '%s' "$class" | sed 's/\([a-z]\)\([A-Z]\)/\1_\2/g' | tr 'A-Z' 'a-z')
        out=crates/string/tests/data/$file.json
        tmp=$(mktemp -p oracle/out)
        oracle oracle/recorders/test_providers.php symfony/string "Inflector/$class" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
        mv "$tmp" "$out"
        echo "String $class providers written to $out"
    done
    for name in inflector lazy; do
        out=crates/string/tests/data/$name.json
        tmp=$(mktemp -p oracle/out)
        oracle "oracle/recorders/string_$name.php" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
        mv "$tmp" "$out"
        echo "String $name vectors written to $out"
    done
}

css_selector() {
    local out=crates/css-selector/tests/data/php.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/css_selector.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "CssSelector providers and vectors written to $out"
}

options_resolver() {
    local out tmp
    mkdir -p oracle/out crates/options-resolver/tests/data
    out=crates/options-resolver/tests/data/options_resolver_test.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/test_providers.php symfony/options-resolver OptionsResolverTest > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "OptionsResolver OptionsResolverTest providers written to $out"
    out=crates/options-resolver/tests/data/edges.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/options_resolver_edges.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "OptionsResolver edge cases written to $out"
}

finder() {
    local name out tmp
    mkdir -p oracle/out crates/finder/tests/data
    for name in providers vectors; do
        out=crates/finder/tests/data/$([[ $name == vectors ]] && echo php || echo providers).json
        tmp=$(mktemp -p oracle/out)
        oracle "oracle/recorders/finder_$name.php" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
        mv "$tmp" "$out"
        echo "Finder $name written to $out"
    done
}

php_dom() {
    local out=crates/php-dom/tests/data/parse.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/php_dom_parse.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "DOMDocument::loadXML() answers written to $out"
}

php_dom_schema() {
    local out=crates/php-dom/tests/data/schema.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/php_dom_schema.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "DOMDocument::schemaValidateSource() answers written to $out"
}

# IpUtils (isPrivateIp(), checkIp()) and HeaderUtils (makeDisposition())
# answers of earlier releases: before GHSA-38cx-cq6f-5755's fix (v8.0.8,
# v8.0.13) and around the v8.1.11 move. Written to oracle/out/ for checking
# advisory verdicts and the move's differences; nothing is committed.
http_foundation_releases() {
    local tag dir
    mkdir -p oracle/out
    for tag in v8.0.8 v8.0.13 v8.1.10 v8.1.11; do
        dir=$(mktemp -d -p oracle/out)
        git -C "$dir" init -q
        git -C "$dir" fetch -q --depth 1 https://github.com/symfony/http-foundation.git "refs/tags/$tag"
        git -C "$dir" show FETCH_HEAD:IpUtils.php > "oracle/out/IpUtils-$tag.php"
        git -C "$dir" show FETCH_HEAD:HeaderUtils.php > "oracle/out/HeaderUtils-$tag.php"
        rm -rf "$dir"
        oracle oracle/recorders/http_foundation_ip_utils.php "/work/oracle/out/IpUtils-$tag.php" > "oracle/out/ip_utils_$tag.json"
        oracle oracle/recorders/http_foundation_header_utils.php "/work/oracle/out/HeaderUtils-$tag.php" > "oracle/out/header_utils_$tag.json"
        echo "IpUtils and HeaderUtils $tag answers written to oracle/out/{ip,header}_utils_$tag.json"
    done
}

http_foundation() {
    local name out tmp
    mkdir -p oracle/out crates/http-foundation/tests/data
    for name in bags header_utils cookie header_bags server_bag ip_utils accept_header; do
        out=crates/http-foundation/tests/data/$name.json
        tmp=$(mktemp -p oracle/out)
        oracle "oracle/recorders/http_foundation_$name.php" > "$tmp"
        python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
        mv "$tmp" "$out"
        echo "HttpFoundation $name vectors written to $out"
    done
    out=crates/php-value/tests/data/dates.json
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_dates.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "Date vectors written to $out"
}

http_foundation_files() {
    local out=crates/http-foundation/tests/data/files.json tmp
    mkdir -p oracle/out
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_files.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "HttpFoundation file vectors written to $out"
}

mime_fileinfo() {
    local out=crates/mime/tests/data/fileinfo.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/mime_fileinfo.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "FileinfoMimeTypeGuesser answers written to $out"
}

twig_trial() {
    local out=spikes/twig-trial/demo/tests/data/php.json tmp
    mkdir -p oracle/out
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/twig_trial.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(len(d["cases"]), "post cases,", len(d["snippets"]), "snippets")' "$tmp"
    mv "$tmp" "$out"
    echo "Twig trial output written to $out"
}

case "${1:-}" in
    routing) routing; routing_route; routing_request_context ;;
    routing-request-context) routing_request_context ;;
    routing-url-generator) routing_url_generator ;;
    routing-route) routing_route ;;
    multipart) multipart ;;
    request-vars) request_vars ;;
    second-app-w1) second_app_w1 ;;
    converter-fixture) converter_fixture ;;
    di-trial) di_trial ;;
    event-dispatcher) event_dispatcher ;;
    stopwatch) stopwatch ;;
    php-value) php_value ;;
    yaml) yaml ;;
    dotenv) dotenv ;;
    config) config ;;
    config-xml-utils) config_xml_utils ;;
    config-loaders) config_loaders ;;
    translation-xliff) translation_xliff ;;
    translation-catalogue) translation_catalogue ;;
    translation-qt) translation_qt ;;
    config-cache) config_cache ;;
    string) string ;;
    console) console ;;
    css-selector) css_selector ;;
    options-resolver) options_resolver ;;
    finder) finder ;;
    php-dom) php_dom ;;
    php-dom-schema) php_dom_schema ;;
    http-foundation) http_foundation ;;
    http-foundation-files) http_foundation_files ;;
    mime-fileinfo) mime_fileinfo ;;
    http-foundation-releases) http_foundation_releases ;;
    twig-trial) twig_trial ;;
    all) routing; routing_route; routing_request_context; routing_url_generator; multipart; request_vars; event_dispatcher; stopwatch; php_value; yaml; dotenv; config; string; console; css_selector; options_resolver; finder; php_dom; php_dom_schema; http_foundation; http_foundation_files; mime_fileinfo; translation_xliff; translation_qt; config_cache ;;
    *) echo "usage: scripts/record.sh routing|routing-route|routing-request-context|routing-url-generator|multipart|request-vars|second-app-w1|converter-fixture|di-trial|event-dispatcher|stopwatch|php-value|yaml|dotenv|config|config-xml-utils|config-loaders|string|console|css-selector|options-resolver|finder|php-dom|php-dom-schema|http-foundation|http-foundation-files|mime-fileinfo|http-foundation-releases|translation-xliff|translation-qt|config-cache|twig-trial|all" >&2; exit 2 ;;
esac
