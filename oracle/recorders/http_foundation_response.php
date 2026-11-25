<?php
// Response answers for what ResponseTest leaves open, for
// crates/http-foundation/tests/data/response.json (scripts/record.sh
// http-foundation-response): 1,200 seeded scripts of constructor, status,
// content, header, cache, date, ETag, Vary, setCache(), prepare(),
// isNotModified() and send calls, each call's result and the process
// calls it made (header(), header_remove(), headers_send(), echo,
// ini_set(), ob_*()), then the response's state and every reader; plus
// 300 closeOutputBuffers() calls on scripted ob_get_status() answers.
// The clock is fixed as ClockMock fixes it; headers_sent() answers what
// the script says. Texts are base64 (lib/typed.php).

namespace Symfony\Component\HttpFoundation {
    const RECORDER_NOW = 1700000000;

    function time(): int
    {
        return RECORDER_NOW;
    }

    function gmdate(string $format, ?int $timestamp = null): string
    {
        return \gmdate($format, $timestamp ?? RECORDER_NOW);
    }

    function strtotime(string $datetime, ?int $baseTimestamp = null): int|false
    {
        return \strtotime($datetime, $baseTimestamp ?? RECORDER_NOW);
    }

    function header(string $header, bool $replace = true, int $response_code = 0): void
    {
        $GLOBALS['recorder_calls'][] = ['header', base64_encode($header), $replace, $response_code];
    }

    function header_remove(?string $name = null): void
    {
        $GLOBALS['recorder_calls'][] = ['header_remove', base64_encode($name)];
    }

    function headers_sent(): bool
    {
        return $GLOBALS['recorder_headers_sent'];
    }

    function ini_set(string $option, string $value): string|false
    {
        $GLOBALS['recorder_calls'][] = ['ini_set', $option, base64_encode($value)];

        return 'text/html';
    }

    function ob_get_status(bool $full_status = false): array
    {
        return $GLOBALS['recorder_ob_status'];
    }

    function ob_end_flush(): bool
    {
        $GLOBALS['recorder_calls'][] = ['ob_end_flush'];

        return true;
    }

    function ob_end_clean(): bool
    {
        $GLOBALS['recorder_calls'][] = ['ob_end_clean'];

        return true;
    }

    function flush(): void
    {
        $GLOBALS['recorder_calls'][] = ['flush'];
    }
}

namespace {
    // FrankenPHP's headers_send(): function_exists('headers_send') is true.
    function headers_send(int $status = 103): int
    {
        $GLOBALS['recorder_calls'][] = ['headers_send', $status];

        return 0;
    }

    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';
    require __DIR__.'/lib/typed.php';

    use Symfony\Component\HttpFoundation\Cookie;
    use Symfony\Component\HttpFoundation\Request;
    use Symfony\Component\HttpFoundation\Response;
    use Symfony\Component\HttpFoundation\ResponseHeaderBag;

    $GLOBALS['recorder_calls'] = [];
    $GLOBALS['recorder_headers_sent'] = false;
    $GLOBALS['recorder_ob_status'] = [];

    mt_srand(20261125);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];
    $now = 1700000000;

    $httpDate = static fn (int $ts) => gmdate('D, d M Y H:i:s', $ts).' GMT';
    $dates = [$httpDate($now), $httpDate($now - 3600), $httpDate($now + 3600), $httpDate($now + 600), $httpDate($now - 86400 * 400), gmdate('D, d M Y H:i:s O', $now + 60), '-1', '0', '', 'foo', 'Sun, 25 Aug 2013 18:33:31 GMT'];
    $values = [
        'Cache-Control' => ['no-cache', 'private', 'public', 'max-age=100', 's-maxage=600, max-age=0', 'max-age=5, s-maxage=500', 'no-store', 'must-revalidate', 'proxy-revalidate', 'public, immutable', 'max-age=abc', 'max-age', 'private, max-age=10', '', null],
        'ETag' => ['"x"', 'W/"x"', 'x', '"12345"', '', null],
        'Age' => ['10', '1000', '0', 'abc', '-5', '1e3', '12abc', null],
        'Vary' => ['Accept-Language', 'Accept-Language, User-Agent', 'a b,,c', ' ,Prefer, ', '', null, ['Accept', 'X-Foo']],
        'Content-Type' => ['text/css', 'text/html; charset=ISO-8859-1', 'application/json', 'TEXT/plain', 'text/xml;Charset=x', 'image/png', '', null],
        'Content-Length' => ['12345', '0', '', null],
        'Transfer-Encoding' => ['chunked', null],
        'Content-Disposition' => ['attachment; filename="f.txt"', 'inline', 'ATTACHMENT', null],
        'Location' => ['/a', '/b', '', null],
        'Preference-Applied' => ['safe', 'other', null],
        'Pragma' => ['no-cache', null],
        'X-Foo' => ['bar', '', null, ['a', 'b']],
    ];
    $names = ['Cache-Control', 'cache-control', 'ETag', 'Etag', 'Last-Modified', 'Expires', 'Date', 'Age', 'Vary', 'Content-Type', 'content_type', 'Content-Length', 'Transfer-Encoding', 'Content-Disposition', 'Location', 'Preference-Applied', 'Pragma', 'X-Foo', 'x-foo'];
    $valueFor = static function (string $name) use ($values, $dates, $pick) {
        $canonical = ['cache-control' => 'Cache-Control', 'Etag' => 'ETag', 'content_type' => 'Content-Type', 'x-foo' => 'X-Foo'][$name] ?? $name;
        if (\in_array($canonical, ['Last-Modified', 'Expires', 'Date'], true)) {
            return 0 === mt_rand(0, 9) ? null : $pick($dates);
        }

        return $pick($values[$canonical]);
    };
    $statuses = [200, 200, 200, 201, 203, 204, 300, 301, 302, 303, 304, 307, 308, 100, 101, 103, 199, 299, 404, 410, 418, 500, 503, 599, 99, 600, 0];
    $texts = [null, null, null, '', 'Custom', '0'];
    $zones = [0, 3600, -18000, 19800, 50400];
    $agents = ['Mozilla/4.0 (compatible; MSIE 8.0; Windows NT 6.0)', 'Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1)', 'Mozilla/5.0 (compatible; MSIE 9.0;)', 'x msie 7.0; y', 'MSIE 6.0 MSIE 10.0;', 'MSIE ;', 'MSIE 8', 'Chrome/24', null];
    $protocols = ['HTTP/1.0', 'HTTP/1.1', 'HTTP/2.0', 'HTTP/1.00', 'http/1.0', '', null];
    $methods = ['GET', 'GET', 'HEAD', 'POST', 'QUERY', 'PUT'];
    $formats = [null, null, 'json', 'html', 'txt', 'xml', 'nope'];
    $ifNoneMatch = [null, '"x"', 'x', 'W/"x"', '"y", "x"', '*', '"12345"', ''];

    $requestSpec = static function () use ($pick, $agents, $protocols, $methods, $formats, $ifNoneMatch, $dates) {
        $server = ['REQUEST_METHOD' => $pick($methods)];
        foreach (['SERVER_PROTOCOL' => $pick($protocols), 'HTTPS' => $pick(['on', 'off', '1', '', null]), 'HTTP_USER_AGENT' => $pick($agents), 'HTTP_IF_NONE_MATCH' => $pick($ifNoneMatch), 'HTTP_IF_MODIFIED_SINCE' => 0 === mt_rand(0, 2) ? $pick($dates) : null] as $k => $v) {
            if (null !== $v) {
                $server[$k] = $v;
            }
        }

        return [typed_value($server), $pick($formats)];
    };
    $buildRequest = static function (array $spec): Request {
        $server = [];
        foreach ($spec[0][1] as [$k, $v]) {
            $server[$k[1] ? base64_decode($k[1]) : $k[1]] = base64_decode($v[1]);
        }
        $request = new Request([], [], [], [], [], $server);
        if (null !== $spec[1]) {
            $request->setRequestFormat($spec[1]);
        }

        return $request;
    };
    $date = static function (int $ts, int $offset): \DateTimeImmutable {
        $sign = $offset < 0 ? '-' : '+';
        $abs = abs($offset);

        return (new \DateTimeImmutable('@'.$ts))->setTimezone(new \DateTimeZone(sprintf('%s%02d:%02d', $sign, intdiv($abs, 3600), intdiv($abs % 3600, 60))));
    };
    $cacheValues = [true, false, 0, 1, 100, '', '12', '12abc', 'abc', ' 7', 1.5, -3, null, '"etag"', 'W/"w"', []];

    $state = static function (Response $r): array {
        $readers = [
            'string' => static fn () => (string) $r,
            'status' => static fn () => $r->getStatusCode(),
            'text' => static fn () => (new \ReflectionProperty($r, 'statusText'))->getValue($r),
            'version' => static fn () => $r->getProtocolVersion(),
            'charset' => static fn () => $r->getCharset(),
            'content' => static fn () => $r->getContent(),
            'headers' => static fn () => $r->headers->allPreserveCase(),
            'cacheable' => static fn () => $r->isCacheable(),
            'fresh' => static fn () => $r->isFresh(),
            'validateable' => static fn () => $r->isValidateable(),
            'revalidate' => static fn () => $r->mustRevalidate(),
            'immutable' => static fn () => $r->isImmutable(),
            'age' => static fn () => $r->getAge(),
            'max_age' => static fn () => $r->getMaxAge(),
            'ttl' => static fn () => $r->getTtl(),
            'date' => static fn () => $r->getDate()?->getTimestamp(),
            'expires' => static fn () => $r->getExpires()?->getTimestamp(),
            'last_modified' => static fn () => $r->getLastModified()?->getTimestamp(),
            'etag' => static fn () => $r->getEtag(),
            'vary' => static fn () => $r->getVary(),
            'has_vary' => static fn () => $r->hasVary(),
            'predicates' => static fn () => [$r->isInvalid(), $r->isInformational(), $r->isSuccessful(), $r->isRedirection(), $r->isClientError(), $r->isServerError(), $r->isOk(), $r->isForbidden(), $r->isNotFound(), $r->isEmpty(), $r->isRedirect(), $r->isRedirect('/a'), $r->isRedirect('')],
            'secure' => static fn () => array_map(static fn (Cookie $c) => $c->isSecure(), $r->headers->getCookies()),
        ];
        $out = [];
        foreach ($readers as $name => $read) {
            $out[] = [$name, typed_call($read)];
        }

        return $out;
    };
    $call = static function (Response $r, callable $f): array {
        $GLOBALS['recorder_calls'] = [];
        ob_start();
        try {
            $result = $f();
            $result = $result === $r ? ['self'] : typed_value($result);
        } catch (\Throwable $e) {
            $result = ['throws', $e::class, preg_replace('/, called in .* on line \d+$/', '', $e->getMessage())];
        }
        $echo = ob_get_clean();
        $calls = $GLOBALS['recorder_calls'];
        if ('' !== $echo) {
            $calls[] = ['echo', base64_encode($echo)];
        }

        return [$result, $calls];
    };

    $scripts = [];
    for ($n = 0; $n < 1200; ++$n) {
        $headers = [];
        for ($i = mt_rand(0, 3); $i > 0; --$i) {
            $name = $pick($names);
            $headers[$name] = $valueFor($name);
        }
        $new = [0 === mt_rand(0, 5) ? null : base64_encode($pick(['', 'foo', 'content'])), 0 === mt_rand(0, 3) ? $pick($statuses) : 200, typed_value($headers)];
        $ops = [];
        for ($i = mt_rand(1, 10); $i > 0; --$i) {
            $op = $pick(['status', 'content', 'version', 'charset', 'set', 'set', 'set', 'set', 'remove', 'private', 'public', 'immutable', 'date', 'expires', 'last_modified', 'expire', 'max_age', 's_maxage', 'sie', 'swr', 'ttl', 'client_ttl', 'etag', 'cache', 'cache', 'not_modified', 'vary', 'safe', 'cookie', 'prepare', 'prepare', 'is_not_modified', 'is_not_modified', 'send_headers', 'send_headers', 'send_content', 'send']);
            $ops[] = match ($op) {
                'status' => ['status', $pick($statuses), $pick($texts)],
                'content' => ['content', 0 === mt_rand(0, 3) ? null : base64_encode($pick(['', 'body', '0']))],
                'version' => ['version', $pick(['1.0', '1.1', '1', '1.00', '2.0', 'x'])],
                'charset' => ['charset', $pick(['utf-8', 'ISO-8859-1', '', '0'])],
                'set' => (static function () use ($pick, $names, $valueFor) {
                    $name = $pick($names);

                    return ['set', $name, typed_value($valueFor($name)), 0 !== mt_rand(0, 3)];
                })(),
                'remove' => ['remove', $pick($names)],
                'private', 'public', 'expire', 'not_modified', 'send_content' => [$op],
                'immutable', 'safe' => [$op, 0 !== mt_rand(0, 2)],
                'date' => ['date', $now + $pick([0, -3600, 3600, -100, 86400]), $pick($zones)],
                'expires', 'last_modified' => [$op, 0 === mt_rand(0, 4) ? null : $now + $pick([0, -3600, 3600, 600, -172800]), $pick($zones)],
                'max_age', 's_maxage', 'sie', 'swr', 'ttl', 'client_ttl' => [$op, $pick([0, 1, 10, 100, -5, 86400])],
                'etag' => ['etag', 0 === mt_rand(0, 4) ? null : $pick(['x', '"x"', 'W/"x"', '', '"']), 0 === mt_rand(0, 2)],
                'cache' => (static function () use ($pick, $cacheValues, $now) {
                    $keys = ['must_revalidate', 'no_cache', 'no_store', 'no_transform', 'public', 'private', 'proxy_revalidate', 'max_age', 's_maxage', 'stale_if_error', 'stale_while_revalidate', 'immutable', 'last_modified', 'etag'];
                    $options = [];
                    for ($i = mt_rand(1, 4); $i > 0; --$i) {
                        $key = 0 === mt_rand(0, 12) ? $pick(['wrong option', 'max-age', 0, '']) : $pick($keys);
                        $options[] = 'last_modified' === $key && 0 !== mt_rand(0, 3)
                            ? [typed_value($key), ['date', $now + $pick([0, -60, 3600])]]
                            : [typed_value($key), typed_value($pick($cacheValues))];
                    }

                    return ['cache', $options];
                })(),
                'vary' => ['vary', typed_value($pick(['Accept-Language', 'Prefer', 'a, b', ['X', 'Y'], ''])), 0 !== mt_rand(0, 2)],
                'cookie' => ['cookie', $pick(['foo', 'bar']), $pick(['1', 'two'])],
                'prepare', 'is_not_modified' => [$op, $requestSpec()],
                'send_headers' => ['send_headers', 0 === mt_rand(0, 2) ? $pick([103, 100, 199, 200, 404]) : null, 0 === mt_rand(0, 5)],
                'send' => ['send', 0 !== mt_rand(0, 2), 0 === mt_rand(0, 5)],
            };
        }

        $results = [];
        $GLOBALS['recorder_calls'] = [];
        try {
            $initial = [];
            foreach ($new[2][1] as [$k, $v]) {
                $initial[base64_decode($k[1])] = match ($v[0]) {
                    'null' => null,
                    'string' => base64_decode($v[1]),
                    'array' => array_map(static fn ($pair) => base64_decode($pair[1][1]), $v[1]),
                };
            }
            $response = new Response(null === $new[0] ? null : base64_decode($new[0]), $new[1], $initial);
        } catch (\Throwable $e) {
            $scripts[] = ['new' => $new, 'throws' => [$e::class, $e->getMessage()]];
            continue;
        }

        foreach ($ops as $op) {
            $GLOBALS['recorder_headers_sent'] = false;
            $f = match ($op[0]) {
                'status' => static fn () => $response->setStatusCode($op[1], $op[2]),
                'content' => static fn () => $response->setContent(null === $op[1] ? null : base64_decode($op[1])),
                'version' => static fn () => $response->setProtocolVersion($op[1]),
                'charset' => static fn () => $response->setCharset($op[1]),
                'set' => static fn () => $response->headers->set($op[1], match ($op[2][0]) {
                    'null' => null,
                    'string' => base64_decode($op[2][1]),
                    'array' => array_map(static fn ($pair) => base64_decode($pair[1][1]), $op[2][1]),
                }, $op[3]),
                'remove' => static fn () => $response->headers->remove($op[1]),
                'private' => static fn () => $response->setPrivate(),
                'public' => static fn () => $response->setPublic(),
                'immutable' => static fn () => $response->setImmutable($op[1]),
                'date' => static fn () => $response->setDate($date($op[1], $op[2])),
                'expires' => static fn () => $response->setExpires(null === $op[1] ? null : $date($op[1], $op[2])),
                'last_modified' => static fn () => $response->setLastModified(null === $op[1] ? null : $date($op[1], $op[2])),
                'expire' => static fn () => $response->expire(),
                'max_age' => static fn () => $response->setMaxAge($op[1]),
                's_maxage' => static fn () => $response->setSharedMaxAge($op[1]),
                'sie' => static fn () => $response->setStaleIfError($op[1]),
                'swr' => static fn () => $response->setStaleWhileRevalidate($op[1]),
                'ttl' => static fn () => $response->setTtl($op[1]),
                'client_ttl' => static fn () => $response->setClientTtl($op[1]),
                'etag' => static fn () => $response->setEtag($op[1], $op[2]),
                'cache' => static function () use ($response, $op, $date) {
                    $options = [];
                    foreach ($op[1] as [$k, $v]) {
                        $key = match ($k[0]) {
                            'int' => $k[1],
                            'string' => base64_decode($k[1]),
                        };
                        $options[$key] = 'date' === $v[0] ? $date($v[1], 0) : match ($v[0]) {
                            'null' => null,
                            'bool', 'int' => $v[1],
                            'float' => unpack('E', hex2bin($v[1]))[1],
                            'string' => base64_decode($v[1]),
                            'array' => [],
                        };
                    }

                    return $response->setCache($options);
                },
                'not_modified' => static fn () => $response->setNotModified(),
                'vary' => static fn () => $response->setVary('array' === $op[1][0] ? array_map(static fn ($pair) => base64_decode($pair[1][1]), $op[1][1]) : base64_decode($op[1][1]), $op[2]),
                'safe' => static fn () => $response->setContentSafe($op[1]),
                'cookie' => static fn () => $response->headers->setCookie(Cookie::create($op[1], $op[2])),
                'prepare' => static fn () => $response->prepare($buildRequest($op[1])),
                'is_not_modified' => static fn () => $response->isNotModified($buildRequest($op[1])),
                'send_headers' => static function () use ($response, $op) {
                    $GLOBALS['recorder_headers_sent'] = $op[2];

                    return $response->sendHeaders($op[1]);
                },
                'send_content' => static fn () => $response->sendContent(),
                'send' => static function () use ($response, $op) {
                    $GLOBALS['recorder_headers_sent'] = $op[2];

                    return $response->send($op[1]);
                },
            };
            $results[] = $call($response, $f);
        }

        $scripts[] = ['new' => $new, 'ops' => $ops, 'results' => $results, 'state' => $state($response)];
    }

    $close = [];
    for ($n = 0; $n < 300; ++$n) {
        $status = [];
        for ($i = mt_rand(0, 4); $i > 0; --$i) {
            $entry = ['name' => 'default output handler', 'level' => \count($status)];
            $kind = mt_rand(0, 9);
            if ($kind < 7) {
                $entry['flags'] = $pick([112, 0x70 | 0x0f, 16, 32, 64, 96, 80, 48, 0, 0x2070]);
            } elseif (8 === $kind) {
                $entry['del'] = 0 === mt_rand(0, 1);
            }
            $status[] = $entry;
        }
        $target = $pick([0, 0, 1, 2, -1, 5]);
        $flush = 0 === mt_rand(0, 1);
        $GLOBALS['recorder_ob_status'] = $status;
        $GLOBALS['recorder_calls'] = [];
        Response::closeOutputBuffers($target, $flush);
        $close[] = [array_map(static fn ($s) => [$s['flags'] ?? null, $s['del'] ?? null], $status), $target, $flush, $GLOBALS['recorder_calls']];
    }

    // prepare()'s two loose comparisons: SERVER_PROTOCOL != 'HTTP/1.0'
    // (typed values) and '1.0' == the protocol version.
    $protocols = [];
    foreach (['1.0', '1', '1.00', '01.0', ' 1.0', '1.0 ', '1e0', '1.1', '2', '', 'x'] as $version) {
        foreach (['HTTP/1.0', 'HTTP/1.00', 'http/1.0', 'HTTP/1.1', '', null, 1, 1.0, true, false, '1.0'] as $protocol) {
            $GLOBALS['recorder_calls'] = [];
            $response = new Response('foo');
            $response->setProtocolVersion($version);
            $request = new Request([], [], [], [], [], ['SERVER_PROTOCOL' => $protocol]);
            $response->prepare($request);
            $protocols[] = [$version, typed_value($protocol), $response->getProtocolVersion(), typed_value($response->headers->get('pragma')), typed_value($response->headers->get('expires'))];
        }
    }

    echo json_encode(['scripts' => $scripts, 'close' => $close, 'protocols' => $protocols], \JSON_THROW_ON_ERROR), "\n";
}
