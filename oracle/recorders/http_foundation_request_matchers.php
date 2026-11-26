<?php
// RequestMatcher answers for what their tests leave open, for
// crates/http-foundation/tests/data/request_matchers.json
// (scripts/record.sh http-foundation-request-matchers): 1,500 seeded
// scripts. Each builds a request with Request::create() from pools: a
// URI (paths with encoded characters, queries with keys in any order and
// case), a method, server entries (a host, an invalid host,
// SERVER_PORT as an int or a string, HTTPS, REMOTE_ADDR), headers, a body
// (JSON or not) and attributes (strings, ints, a closure). It then asks
// matchers built from pools whether they match (two fixed scripts
// first: SERVER_PORT as a string and as an int, without a host): every RequestMatcher
// class, with lists given as arrays or comma-separated text (spaces,
// tabs, empty items), patterns that do not compile, expressions over
// the request object's getters and bags, and a ChainRequestMatcher of
// several. Recorded: each answer, or the exception's class and message.
// Texts are base64 (lib/typed.php).

namespace {
    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';
    require __DIR__.'/lib/typed.php';

    use Symfony\Component\ExpressionLanguage\ExpressionLanguage;
    use Symfony\Component\HttpFoundation\ChainRequestMatcher;
    use Symfony\Component\HttpFoundation\Request;
    use Symfony\Component\HttpFoundation\RequestMatcher\AttributesRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\ExpressionRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\HeaderRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\HostRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\IpsRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\IsJsonRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\MethodRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\PathRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\PortRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\QueryParameterRequestMatcher;
    use Symfony\Component\HttpFoundation\RequestMatcher\SchemeRequestMatcher;

    mt_srand(20261130);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];
    $b64 = static fn (?string $s) => null === $s ? null : base64_encode($s);

    $uris = ['/', '/admin/foo', '/admin/fo%20o', '/admin/fo%0Ao', '/en/login', '/blog/2024/post', '/caf%C3%A9', '/a%2Fb', 'https://example.com/x', 'http://example.com:8080/y?foo=1&bar=2', '/?foo=&bar=', '/?bar=1&foo=2', '/?FOO=1', '/?foo[]=1&bar=', '/?0=a&1=b', 'sftp://example.com/', '//weird'];
    $methods = ['GET', 'get', 'POST', 'HEAD', 'PUT', 'DELETE', 'OPTIONS'];
    $hosts = [null, 'foo.example.com', 'EXAMPLE.com', 'localhost:8000', 'sensio.com', 'bad host!'];
    $ports = [null, 80, 8000, '8000', 443];
    $addrs = [null, '127.0.0.1', '::1', '192.168.1.10', '10.0.0.1'];
    $headerNames = ['X-Foo', 'x-foo', 'Bar', 'X_FOO', 'Accept', 'Content-Type', 'X-Requested-With'];
    $headerValues = ['1', 'foo', 'XMLHttpRequest', 'application/json', ''];
    $bodies = [null, '', 'not json', '"s"', '11', '[1, 2]', '{"a": 1}', '{', ' {"a":1} ', 'null', '1e400'];
    $attributes = [['foo', 'foo_bar'], ['foo', 'FOO'], ['id', 42], ['_route', 'home'], ['_controller', 'closure']];

    $lists = ['', 'get', 'post,get', 'post, get', 'POST ,  GET', "head\t,\tpost", ',get', 'get,', 'x-foo, bar', 'X-FOO', 'bar', 'x_foo', '127.0.0.1', '127.0.0.1, ::1', '192.168.0.0/16', '10.0.0.0/8, ::1', 'not-an-ip', 'foo', 'foo, bar', 'bar,foo', 'FOO', '0, 1', 'http', 'HTTPS', 'ftp, http', 'https,http', ' https '];
    $regexps = ['.*', '^/admin', '/admin/.*', '^/admin/fo o*$', '\.example\.com$', '^foo', 'EXAMPLE', '^.*$', '(', '[a-', 'caf.', 'é', '^/a/b$', '^/{_locale}/login$', 'foo_.*', '^\d+$', 'home'];
    $expressions = [
        'request.getMethod() == method',
        'request.getPathInfo() == path',
        'path matches "{^/admin}"',
        'host == "foo.example.com"',
        'ip == "127.0.0.1"',
        'request.getClientIp() in ["127.0.0.1", "::1"]',
        'request.headers.get("x-foo") == "1"',
        'request.headers.has("X_FOO")',
        'request.headers.get("missing", "d") == "d"',
        'request.query.get("foo")',
        'request.query.keys() == ["foo", "bar"]',
        'request.query.count() > 1',
        'request.attributes.all() == attributes',
        'request.attributes.get("foo") == "foo_bar"',
        'request.attributes.has("id")',
        'request.server.get("REMOTE_ADDR") == ip',
        'request.cookies.all() == []',
        'request.isSecure()',
        'request.isXmlHttpRequest()',
        'request.isMethod("post")',
        'request.getScheme() == "https"',
        'request.getPort()',
        'request.getHost()',
        'request.getHttpHost()',
        'request.getUri()',
        'request.getRequestUri()',
        'request.getQueryString()',
        'request.getLocale() == "en"',
        'request.getSchemeAndHttpHost()',
        'request.getUnknown()',
        '1',
        '0',
        '"x"',
        '""',
        '1.5',
        'null',
        '[1]',
        'method',
        'attributes',
        'not request.isSecure() and method in ["GET", "HEAD"]',
    ];

    $spec = static function (bool $inChain = false) use (&$spec, $pick, $lists, $regexps, $expressions, $headerNames) {
        $list = static fn () => 0 === mt_rand(0, 1) ? [$pick($lists)] : array_map(static fn () => $pick($lists), range(1, mt_rand(0, 3)));
        $kinds = ['attributes', 'header', 'host', 'ips', 'is_json', 'method', 'path', 'port', 'query', 'scheme', 'expression', 'expression'];
        if (!$inChain) {
            $kinds[] = 'chain';
        }
        $kind = $pick($kinds);

        return match ($kind) {
            'attributes' => ['attributes', array_map(static fn () => [$pick(['foo', 'id', '_route', '_controller', 'none']), base64_encode($pick($regexps))], range(1, mt_rand(0, 2)))],
            'header' => ['header', array_map('base64_encode', 0 === mt_rand(0, 2) ? array_map(static fn () => $pick($headerNames), range(1, mt_rand(1, 2))) : $list())],
            'host', 'path' => [$kind, base64_encode($pick($regexps))],
            'ips', 'method', 'query', 'scheme' => [$kind, array_map('base64_encode', $list())],
            'is_json' => ['is_json'],
            'port' => ['port', $pick([80, 443, 8000, 8080, 0])],
            'expression' => ['expression', base64_encode($pick($expressions))],
            'chain' => ['chain', array_map(static fn () => $spec(true), range(0, mt_rand(0, 3)))],
        };
    };

    $build = static function (array $spec) use (&$build): object {
        $d = static fn (array $l) => array_map('base64_decode', $l);

        return match ($spec[0]) {
            'attributes' => new AttributesRequestMatcher(array_combine(array_column($spec[1], 0), array_map('base64_decode', array_column($spec[1], 1))) ?: []),
            'header' => new HeaderRequestMatcher($d($spec[1])),
            'host' => new HostRequestMatcher(base64_decode($spec[1])),
            'ips' => new IpsRequestMatcher($d($spec[1])),
            'is_json' => new IsJsonRequestMatcher(),
            'method' => new MethodRequestMatcher($d($spec[1])),
            'path' => new PathRequestMatcher(base64_decode($spec[1])),
            'port' => new PortRequestMatcher($spec[1]),
            'query' => new QueryParameterRequestMatcher($d($spec[1])),
            'scheme' => new SchemeRequestMatcher($d($spec[1])),
            'expression' => new ExpressionRequestMatcher(new ExpressionLanguage(), base64_decode($spec[1])),
            'chain' => new ChainRequestMatcher(array_map($build, $spec[1])),
        };
    };

    // With HTTP_HOST null (Request::create() sets 'localhost'), getPort()
    // is SERVER_PORT as given: the
    // string '8000' is not === 8000.
    $fixed = [
        [[['HTTP_HOST', null], ['SERVER_PORT', '8000']], [['port', 8000], ['port', 80]]],
        [[['HTTP_HOST', null], ['SERVER_PORT', 8000]], [['port', 8000], ['port', 80]]],
    ];

    $scripts = [];
    for ($n = 0; $n < 1500; ++$n) {
        $server = [];
        if (null !== $h = $pick($hosts)) {
            $server[] = ['HTTP_HOST', $h];
        }
        if (null !== $p = $pick($ports)) {
            $server[] = ['SERVER_PORT', $p];
        }
        if (0 === mt_rand(0, 3)) {
            $server[] = ['HTTPS', $pick(['on', 'off', '1'])];
        }
        if (null !== $a = $pick($addrs)) {
            $server[] = ['REMOTE_ADDR', $a];
        }
        for ($i = mt_rand(0, 2); $i > 0; --$i) {
            $server[] = ['HTTP_'.strtoupper(str_replace('-', '_', $pick($headerNames))), $pick($headerValues)];
        }
        $requestSpec = [
            'uri' => base64_encode($pick($uris)),
            'method' => $pick($methods),
            'server' => $server,
            'content' => $b64($pick($bodies)),
            'attributes' => array_map(static fn () => $pick($attributes), range(1, mt_rand(0, 2))),
            'locale' => 0 === mt_rand(0, 4) ? 'en' : null,
        ];
        $matchers = array_map(static fn () => $spec(), range(1, 6));
        if (isset($fixed[$n])) {
            [$server, $ports] = $fixed[$n];
            $matchers = [...$ports, ...\array_slice($matchers, 2)];
            $requestSpec['server'] = $server;
            // A URI with a host would set HTTP_HOST again.
            $requestSpec['uri'] = base64_encode('/');
        }

        $serverArray = [];
        foreach ($server as [$k, $v]) {
            $serverArray[$k] = $v;
        }
        $results = [];
        foreach ($matchers as $m) {
            // A fresh request for each matcher, as getters cache.
            $request = Request::create(base64_decode($requestSpec['uri']), $requestSpec['method'], [], [], [], $serverArray, null === $requestSpec['content'] ? null : base64_decode($requestSpec['content']));
            foreach ($requestSpec['attributes'] as [$k, $v]) {
                $request->attributes->set($k, 'closure' === $v ? static fn () => null : $v);
            }
            if (null !== $requestSpec['locale']) {
                $request->setLocale($requestSpec['locale']);
            }
            try {
                $results[] = typed_value($build($m)->matches($request));
            } catch (\Throwable $e) {
                $results[] = ['throws', $e::class, base64_encode($e->getMessage())];
            }
        }
        $scripts[] = ['request' => $requestSpec, 'matchers' => $matchers, 'results' => $results];
    }

    echo json_encode(['scripts' => $scripts], \JSON_THROW_ON_ERROR), "\n";
}
