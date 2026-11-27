#!/usr/bin/env bash
# Record test vectors from the PHP oracle (oracle/, Docker) into the crates.
#
#   scripts/record.sh routing      crates/routing/tests/data/matcher.json and route.json
#   scripts/record.sh routing-route crates/routing/tests/data/route.json (Route, RouteCompiler, RouteCollection)
#   scripts/record.sh routing-request-context crates/routing/tests/data/request_context.json
#   scripts/record.sh routing-from-request crates/routing/tests/data/from_request.json (fromRequest(), matchRequest(); reads http-foundation's request.json)
#   scripts/record.sh routing-url-generator crates/routing/tests/data/url_generator.json (UrlGenerator)
#   scripts/record.sh routing-url-matcher crates/routing/tests/data/url_matcher.json (UrlMatcher and RedirectableUrlMatcher)
#   scripts/record.sh routing-compiled-url-generator crates/routing/tests/data/compiled_url_generator.json (CompiledUrlGenerator and its dumper)
#   scripts/record.sh multipart    crates/http-foundation/tests/data/multipart_vectors.json
#   scripts/record.sh request-vars crates/http-foundation/tests/data/{query,cookie,content_type}_vectors.json
#   scripts/record.sh second-app-w1 oracle/apps/easyadmin-demo/w1.json (second app, W1)
#   scripts/record.sh converter-fixture oracle/apps/converter-fixture/responses.json (M1-D fixture)
#   scripts/record.sh event-dispatcher  crates/event-dispatcher/tests/data/edges.txt
#   scripts/record.sh stopwatch    crates/stopwatch/tests/data/php.json
#   scripts/record.sh clock        crates/clock/tests/data/php.json (dates, zones, DatePoint, MockClock)
#   scripts/record.sh yaml         crates/yaml/tests/data/*.json (lint:yaml included)
#   scripts/record.sh yaml-unreadable crates/yaml/tests/data/unreadable_file.json (unprivileged oracle)
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
#   scripts/record.sh http-foundation crates/http-foundation/tests/data/{bags,header_utils,cookie,header_bags,server_bag,ip_utils,accept_header}.json
#   scripts/record.sh http-foundation-files crates/http-foundation/tests/data/files.json (UploadedFile, FileBag)
#   scripts/record.sh http-foundation-request crates/http-foundation/tests/data/request.json (Request)
#   scripts/record.sh http-foundation-response crates/http-foundation/tests/data/response.json (Response)
#   scripts/record.sh http-foundation-streamed-response crates/http-foundation/tests/data/streamed_response.json (StreamedResponse)
#   scripts/record.sh http-foundation-event-stream crates/http-foundation/tests/data/event_stream.json (EventStreamResponse, ServerEvent)
#   scripts/record.sh http-foundation-streamed-json crates/http-foundation/tests/data/streamed_json.json (StreamedJsonResponse)
#   scripts/record.sh http-foundation-binary-file crates/http-foundation/tests/data/binary_file.json (BinaryFileResponse)
#   scripts/record.sh http-foundation-request-matchers crates/http-foundation/tests/data/request_matchers.json (RequestMatcher, ChainRequestMatcher)
#   scripts/record.sh http-foundation-session crates/http-foundation/tests/data/session.json (Session, bags, mock storages)
#   scripts/record.sh http-foundation-session-handlers crates/http-foundation/tests/data/session_handlers.json (Strict, Marshalling, Migrating handlers)
#   scripts/record.sh http-foundation-native-session crates/http-foundation/tests/data/native_session.json (PHP's session extension, NativeSessionStorage, PhpBridgeSessionStorage)
#   scripts/record.sh http-foundation-request-session crates/http-foundation/tests/data/request_session.json (Request's and RequestStack's session methods)
#   scripts/record.sh http-foundation-json-redirect crates/http-foundation/tests/data/json_redirect.json (JsonResponse, RedirectResponse)
#   scripts/record.sh http-kernel-lifecycle crates/http-kernel/tests/data/lifecycle.json (HttpKernel)
#   scripts/record.sh http-kernel-router-listener crates/http-kernel/tests/data/router_listener.json (RouterListener)
#   scripts/record.sh http-kernel-argument-resolver crates/http-kernel/tests/data/argument_resolver.json (ArgumentResolver)
#   scripts/record.sh http-kernel-controller-resolver crates/http-kernel/tests/data/controller_resolver.json (ControllerResolver)
#   scripts/record.sh http-kernel-container-controller-resolver crates/http-kernel/tests/data/container_controller_resolver.json (ContainerControllerResolver)
#   scripts/record.sh http-kernel-response-listener crates/http-kernel/tests/data/response_listener.json (ResponseListener)
#   scripts/record.sh runtime-fpm  crates/http-foundation/tests/data/sapi.json (PHP-FPM: superglobals, header() and output; oracle/fpm.Dockerfile)
#   scripts/record.sh http-kernel-welcome crates/http-kernel/resources/welcome.html (RouterListener's welcome page)
#   scripts/record.sh mime-fileinfo crates/mime/tests/data/fileinfo.json (FileinfoMimeTypeGuesser, needs upstream/)
#   scripts/record.sh expression-language crates/expression-language/tests/data/php.json (needs upstream/)
#   scripts/record.sh password-hasher crates/password-hasher/tests/data/php.json (needs upstream/; verifies tests/data/rust_hashes.json)
#   scripts/record.sh http-foundation-releases oracle/out/{ip,header}_utils_<tag>.json (earlier releases, advisory checks)
#   scripts/record.sh di-parameter-bag crates/dependency-injection/tests/data/parameter_bag.json (ParameterBag)
#   scripts/record.sh di-env-var-processor crates/dependency-injection/tests/data/env_var_processor.json (EnvVarProcessor)
#   scripts/record.sh di-dumpers   crates/dependency-injection/tests/data/dumpers.json (XmlDumper, YamlDumper, GraphvizDumper; needs upstream/)
#   scripts/record.sh di-yaml-windows-file crates/dependency-injection/tests/data/yaml_windows_file.yml
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

# The oracle mounts the moved packages from upstream/ (oracle/compose.yaml):
# refuse to record from a checkout that is not at its pinned commit.
check_moved_pins() {
    local moved package commit head
    moved=$(sed -n 's/^# moved: \(.*\) (.*$/\1/p' parity/packages.tsv)
    for package in $moved; do
        commit=$(awk -F'\t' -v p="$package" '$1 == p { print $3 }' parity/packages.tsv)
        head=$(git -C "upstream/$package" rev-parse HEAD 2>/dev/null || echo missing)
        if [[ "$head" != "$commit" ]]; then
            echo "upstream/$package is at $head, pinned at $commit: run scripts/fetch-upstream.sh $package" >&2
            exit 1
        fi
    done
}

oracle() { check_moved_pins; docker compose -f oracle/compose.yaml run --rm -T php "$@"; }

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

routing_from_request() {
    local out=crates/routing/tests/data/from_request.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/routing_from_request.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["cases"]), "recorded requests,", len(d["edges"]), "edge requests")' "$tmp"
    mv "$tmp" "$out"
    echo "fromRequest() and matchRequest() answers written to $out"
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

routing_url_matcher() {
    local out=crates/routing/tests/data/url_matcher.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/routing_url_matcher.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d), "collections,", sum(len(c[1]) for c in d), "requests")' "$tmp"
    mv "$tmp" "$out"
    echo "UrlMatcher answers written to $out"
}

routing_compiled_url_generator() {
    local out=crates/routing/tests/data/compiled_url_generator.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/routing_compiled_url_generator.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1]))), "dumped collections")' "$tmp"
    mv "$tmp" "$out"
    echo "CompiledUrlGenerator answers written to $out"
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
    # /demo is read-only; the php-kernel service puts a tmpfs on /demo/var
    # for the kernel's cache.
    check_moved_pins
    docker compose -f oracle/compose.yaml run --rm -T php-kernel \
        oracle/recorders/di_container.php > "$tmp"
    mv "$tmp" "$out"
    echo "$(grep -c '<service ' "$out") service definitions written to $out"
    out=spikes/di-trial/demo/tests/data/php_classes.json
    mkdir -p "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    check_moved_pins
    docker compose -f oracle/compose.yaml run --rm -T php-kernel \
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



http_foundation_request() {
    local out=crates/http-foundation/tests/data/request.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_request.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["cases"]), "Request cases")' "$tmp"
    mv "$tmp" "$out"
    echo "Request vectors written to $out"
}


http_foundation_response() {
    local out=crates/http-foundation/tests/data/response.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_response.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "Response scripts,", len(d["close"]), "closeOutputBuffers() calls,", len(d["protocols"]), "protocol pairs")' "$tmp"
    mv "$tmp" "$out"
    echo "Response vectors written to $out"
}

http_foundation_streamed_response() {
    local out=crates/http-foundation/tests/data/streamed_response.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_streamed_response.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "StreamedResponse scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "StreamedResponse vectors written to $out"
}

http_foundation_event_stream() {
    local out=crates/http-foundation/tests/data/event_stream.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_event_stream.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "EventStreamResponse scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "EventStreamResponse vectors written to $out"
}

http_foundation_streamed_json() {
    local out=crates/http-foundation/tests/data/streamed_json.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_streamed_json.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "StreamedJsonResponse scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "StreamedJsonResponse vectors written to $out"
}

http_foundation_binary_file() {
    local out=crates/http-foundation/tests/data/binary_file.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_binary_file.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "BinaryFileResponse scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "BinaryFileResponse vectors written to $out"
}

http_foundation_request_matchers() {
    local out=crates/http-foundation/tests/data/request_matchers.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_request_matchers.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "RequestMatcher scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "RequestMatcher vectors written to $out"
}

http_foundation_native_session() {
    local out=crates/http-foundation/tests/data/native_session.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_native_session.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scenarios"]), "native session scenarios")' "$tmp"
    mv "$tmp" "$out"
    echo "Native session vectors written to $out"
}

http_foundation_session_handlers() {
    local out=crates/http-foundation/tests/data/session_handlers.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_session_handlers.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "session handler scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "Session handler vectors written to $out"
}

http_foundation_session() {
    local out=crates/http-foundation/tests/data/session.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_session.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "Session scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "Session vectors written to $out"
}

http_foundation_request_session() {
    local out=crates/http-foundation/tests/data/request_session.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_request_session.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "Request session scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "Request session vectors written to $out"
}

http_foundation_json_redirect() {
    local out=crates/http-foundation/tests/data/json_redirect.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_foundation_json_redirect.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["json"]), "JsonResponse scripts,", len(d["callbacks"]), "callback names,", len(d["redirect"]), "RedirectResponse cases")' "$tmp"
    mv "$tmp" "$out"
    echo "JsonResponse and RedirectResponse vectors written to $out"
}

http_kernel_lifecycle() {
    local out=crates/http-kernel/tests/data/lifecycle.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_kernel_lifecycle.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "HttpKernel scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "HttpKernel vectors written to $out"
}

http_kernel_router_listener() {
    local out=crates/http-kernel/tests/data/router_listener.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_kernel_router_listener.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "RouterListener scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "RouterListener vectors written to $out"
}

http_kernel_response_listener() {
    local out=crates/http-kernel/tests/data/response_listener.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_kernel_response_listener.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "ResponseListener scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "ResponseListener vectors written to $out"
}

runtime_fpm() {
    local out=crates/http-foundation/tests/data/sapi.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    docker compose -f oracle/compose.yaml run --rm -T php-fpm oracle/recorders/runtime_fpm.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["requests"]), "requests,", len(d["scripts"]), "output scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "PHP-FPM vectors written to $out"
}

http_kernel_argument_resolver() {
    local out=crates/http-kernel/tests/data/argument_resolver.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_kernel_argument_resolver.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "ArgumentResolver scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "ArgumentResolver vectors written to $out"
}

http_kernel_controller_resolver() {
    local out=crates/http-kernel/tests/data/controller_resolver.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_kernel_controller_resolver.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "ControllerResolver scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "ControllerResolver vectors written to $out"
}

http_kernel_container_controller_resolver() {
    local out=crates/http-kernel/tests/data/container_controller_resolver.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    check_moved_pins
    docker compose -f oracle/compose.yaml run --rm -T -e CR_MODE=container php         oracle/recorders/http_kernel_controller_resolver.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["scripts"]), "ContainerControllerResolver scripts")' "$tmp"
    mv "$tmp" "$out"
    echo "ContainerControllerResolver vectors written to $out"
}

http_kernel_welcome() {
    local out=crates/http-kernel/resources/welcome.html tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/http_kernel_welcome.php > "$tmp"
    mv "$tmp" "$out"
    echo "Welcome page written to $out"
}

yaml_unreadable() {
    local out=crates/yaml/tests/data/unreadable_file.json tmp
    mkdir -p oracle/out
    tmp=$(mktemp -p oracle/out)
    # PHP's root process can read chmod(0200) files. This case must run
    # without that privilege; repository mounts remain read-only.
    docker compose -f oracle/compose.yaml run --rm -T --user 1000:1000 \
        --env USER=check php oracle/recorders/yaml_unreadable.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); assert d["unreadable"][0] == "throws"; assert d["readable"] == d["restored"]' "$tmp"
    mv "$tmp" "$out"
    echo "Unprivileged Parser::parseFile() answers written to $out"
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
    yaml_unreadable
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

# PasswordHasher loads only upstream/symfony/password-hasher, so it runs on
# the oracle image's base (same PHP, sodium 1.0.18 and libargon2) without
# the symfony/demo mount.
password_hasher() {
    local out=crates/password-hasher/tests/data/php.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    docker run --rm -v "$root:/work:ro" -w /work php:8.4.27-cli-bookworm \
        php -d display_errors=stderr oracle/recorders/password_hasher.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["digest"]) + len(d["pbkdf2"]) + len(d["crypt"]), "digest, PBKDF2 and crypt() vectors,", len(d.get("rust_hashes", [])), "Rust hashes verified")' "$tmp"
    mv "$tmp" "$out"
    echo "PasswordHasher answers written to $out"
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


di_parameter_bag() {
    local out=crates/dependency-injection/tests/data/parameter_bag.json tmp
    mkdir -p oracle/out crates/dependency-injection/tests/data
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/di_parameter_bag.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1], encoding="utf-8"))["cases"]), "cases")' "$tmp"
    mv "$tmp" "$out"
    echo "ParameterBag vectors written to $out"
}

di_env_var_processor() {
    local out=crates/dependency-injection/tests/data/env_var_processor.json tmp
    mkdir -p oracle/out crates/dependency-injection/tests/data
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/di_env_var_processor.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1], encoding="utf-8"))), "cases")' "$tmp"
    mv "$tmp" "$out"
    echo "EnvVarProcessor vectors written to $out"
}

di_dumpers() {
    local out=crates/dependency-injection/tests/data/dumpers.json tmp
    mkdir -p oracle/out crates/dependency-injection/tests/data
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/di_dumpers.php > "$tmp"
    python -c 'import json,sys; print(len(json.load(open(sys.argv[1], encoding="utf-8"))), "dumps")' "$tmp"
    mv "$tmp" "$out"
    echo "Dumper vectors written to $out"
}

di_yaml_windows_file() {
    local out=crates/dependency-injection/tests/data/yaml_windows_file.yml tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/di_yaml_windows_file.php > "$tmp"
    mv "$tmp" "$out"
    echo "Windows-path YAML fixture written to $out"
}




clock() {
    local out=crates/clock/tests/data/php.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/clock.php > "$tmp"
    python -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(len(d["modify"]), "modify cases,", len(d["datepoint"]), "DatePoint cases")' "$tmp"
    mv "$tmp" "$out"
    echo "Clock vectors written to $out"
}

expression_language() {
    local out=crates/expression-language/tests/data/php.json tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle oracle/recorders/expression_language.php > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1]))' "$tmp"
    mv "$tmp" "$out"
    echo "ExpressionLanguage vectors written to $out"
}



case "${1:-}" in
    routing) routing; routing_route; routing_request_context ;;
    routing-request-context) routing_request_context ;;
    routing-from-request) routing_from_request ;;
    routing-url-generator) routing_url_generator ;;
    routing-url-matcher) routing_url_matcher ;;
    routing-compiled-url-generator) routing_compiled_url_generator ;;
    routing-route) routing_route ;;
    multipart) multipart ;;
    request-vars) request_vars ;;
    second-app-w1) second_app_w1 ;;
    converter-fixture) converter_fixture ;;
    di-trial) di_trial ;;
    event-dispatcher) event_dispatcher ;;
    stopwatch) stopwatch ;;
    clock) clock ;;
    yaml) yaml ;;
    yaml-unreadable) yaml_unreadable ;;
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
    http-foundation) http_foundation ;;
    http-foundation-files) http_foundation_files ;;
    http-foundation-request) http_foundation_request ;;
    http-foundation-response) http_foundation_response ;;
    http-foundation-streamed-response) http_foundation_streamed_response ;;
    http-foundation-event-stream) http_foundation_event_stream ;;
    http-foundation-streamed-json) http_foundation_streamed_json ;;
    http-foundation-binary-file) http_foundation_binary_file ;;
    http-foundation-request-matchers) http_foundation_request_matchers ;;
    http-foundation-session) http_foundation_session ;;
    http-foundation-session-handlers) http_foundation_session_handlers ;;
    http-foundation-native-session) http_foundation_native_session ;;
    http-foundation-request-session) http_foundation_request_session ;;
    http-foundation-json-redirect) http_foundation_json_redirect ;;
    http-kernel-lifecycle) http_kernel_lifecycle ;;
    http-kernel-router-listener) http_kernel_router_listener ;;
    http-kernel-welcome) http_kernel_welcome ;;
    http-kernel-response-listener) http_kernel_response_listener ;;
    runtime-fpm) runtime_fpm ;;
    http-kernel-argument-resolver) http_kernel_argument_resolver ;;
    http-kernel-controller-resolver) http_kernel_controller_resolver ;;
    http-kernel-container-controller-resolver) http_kernel_container_controller_resolver ;;
    mime-fileinfo) mime_fileinfo ;;
    expression-language) expression_language ;;
    password-hasher) password_hasher ;;
    http-foundation-releases) http_foundation_releases ;;
    di-parameter-bag) di_parameter_bag ;;
    di-env-var-processor) di_env_var_processor ;;
    di-dumpers) di_dumpers ;;
    di-yaml-windows-file) di_yaml_windows_file ;;
    twig-trial) twig_trial ;;
    all) routing; routing_route; routing_request_context; routing_url_generator; routing_url_matcher; routing_compiled_url_generator; multipart; request_vars; event_dispatcher; stopwatch; clock; yaml; dotenv; config; string; console; css_selector; options_resolver; finder; http_foundation; http_foundation_files; http_foundation_request; http_foundation_response; http_foundation_streamed_response; http_foundation_event_stream; http_foundation_streamed_json; http_foundation_binary_file; http_foundation_request_matchers; http_foundation_session; http_foundation_session_handlers; http_foundation_native_session; http_foundation_request_session; http_foundation_json_redirect; http_kernel_lifecycle; http_kernel_router_listener; http_kernel_welcome; http_kernel_argument_resolver; http_kernel_controller_resolver; http_kernel_container_controller_resolver; http_kernel_response_listener; runtime_fpm; routing_from_request; mime_fileinfo; password_hasher; expression_language; translation_xliff; translation_qt; config_cache; di_parameter_bag; di_env_var_processor; di_dumpers; di_yaml_windows_file ;;
    *) echo "usage: scripts/record.sh routing|routing-route|routing-request-context|routing-from-request|routing-url-generator|routing-url-matcher|routing-compiled-url-generator|multipart|request-vars|second-app-w1|converter-fixture|di-trial|event-dispatcher|stopwatch|clock|yaml|yaml-unreadable|dotenv|config|config-xml-utils|config-loaders|string|console|css-selector|options-resolver|finder|http-foundation|http-foundation-files|http-foundation-request|http-foundation-response|http-foundation-streamed-response|http-foundation-event-stream|http-foundation-streamed-json|http-foundation-binary-file|http-foundation-request-matchers|http-foundation-session|http-foundation-session-handlers|http-foundation-native-session|http-foundation-request-session|http-foundation-json-redirect|http-kernel-lifecycle|http-kernel-router-listener|http-kernel-welcome|http-kernel-argument-resolver|http-kernel-controller-resolver|http-kernel-container-controller-resolver|http-kernel-response-listener|mime-fileinfo|password-hasher|expression-language|http-foundation-releases|translation-xliff|translation-qt|config-cache|twig-trial|di-parameter-bag|di-env-var-processor|di-dumpers|di-yaml-windows-file|all" >&2; exit 2 ;;
esac
