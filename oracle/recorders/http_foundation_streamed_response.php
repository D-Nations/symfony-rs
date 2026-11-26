<?php
// StreamedResponse answers for what StreamedResponseTest leaves open, for
// crates/http-foundation/tests/data/streamed_response.json
// (scripts/record.sh http-foundation-streamed-response): 1,500 seeded
// scripts, each a StreamedResponse built from a callback (echoing parts,
// flushing between them, counting its calls, or throwing), a list of
// chunks or null, then up to ten calls: prepare() against GET and HEAD
// requests, setNotModified(), isNotModified(), sendHeaders() (with
// informational codes), sendContent(), send(), setContent(), getContent(),
// (string), getCallback() called, setCallback(), setChunks(), status and
// header changes. Each call's result and the process calls it made in
// order (header(), header_remove(), headers_send(), ini_set(), echo,
// ob_flush(), flush(), ob_end_*()), then the response's state. The clock
// is fixed as ClockMock fixes it. Texts are base64 (lib/typed.php).

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

    /** What was echoed since the last recorded call, as one call. */
    function recorder_drain(): void
    {
        $echo = ob_get_contents();
        if (false !== $echo && '' !== $echo) {
            $GLOBALS['recorder_calls'][] = ['echo', base64_encode($echo)];
            ob_clean();
        }
    }

    function header(string $header, bool $replace = true, int $response_code = 0): void
    {
        recorder_drain();
        $GLOBALS['recorder_calls'][] = ['header', base64_encode($header), $replace, $response_code];
    }

    function header_remove(?string $name = null): void
    {
        recorder_drain();
        $GLOBALS['recorder_calls'][] = ['header_remove', base64_encode($name)];
    }

    function headers_sent(): bool
    {
        return $GLOBALS['recorder_headers_sent'];
    }

    function ini_set(string $option, string $value): string|false
    {
        recorder_drain();
        $GLOBALS['recorder_calls'][] = ['ini_set', $option, base64_encode($value)];

        return 'text/html';
    }

    function ob_get_status(bool $full_status = false): array
    {
        return [];
    }

    function ob_end_flush(): bool
    {
        recorder_drain();
        $GLOBALS['recorder_calls'][] = ['ob_end_flush'];

        return true;
    }

    function ob_end_clean(): bool
    {
        recorder_drain();
        $GLOBALS['recorder_calls'][] = ['ob_end_clean'];

        return true;
    }

    function ob_flush(): bool
    {
        recorder_drain();
        $GLOBALS['recorder_calls'][] = ['ob_flush'];

        return true;
    }

    function flush(): void
    {
        recorder_drain();
        $GLOBALS['recorder_calls'][] = ['flush'];
    }
}

namespace {
    // FrankenPHP's headers_send(): function_exists('headers_send') is true.
    function headers_send(int $status = 103): int
    {
        Symfony\Component\HttpFoundation\recorder_drain();
        $GLOBALS['recorder_calls'][] = ['headers_send', $status];

        return 0;
    }

    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';
    require __DIR__.'/lib/typed.php';

    use Symfony\Component\HttpFoundation\Request;
    use Symfony\Component\HttpFoundation\Response;
    use Symfony\Component\HttpFoundation\StreamedResponse;

    $GLOBALS['recorder_calls'] = [];
    $GLOBALS['recorder_headers_sent'] = false;

    mt_srand(20261126);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];

    $texts = ['', 'foo', 'bar', '0', "line\n", 'chunk'];
    $callbackSpec = static function () use ($pick, $texts) {
        return match (mt_rand(0, 4)) {
            0 => ['null'],
            1 => ['chunks', array_map('base64_encode', array_map(static fn () => $pick($texts), range(1, mt_rand(0, 3))))],
            2 => ['count'],
            3 => ['throw', base64_encode($pick($texts))],
            default => ['parts', array_map(static fn () => [base64_encode($pick($texts)), 0 === mt_rand(0, 1)], range(1, mt_rand(1, 3)))],
        };
    };
    // A callback for a spec; 'count' shares its counter with the script.
    $callback = static function (array $spec, array &$counter) {
        return match ($spec[0]) {
            'count' => static function () use (&$counter) {
                ++$counter[0];
                echo 'call '.$counter[0];
            },
            'throw' => static function () use ($spec) {
                echo base64_decode($spec[1]);
                throw new \RuntimeException('boom');
            },
            'parts' => static function () use ($spec) {
                foreach ($spec[1] as [$text, $flush]) {
                    echo base64_decode($text);
                    if ($flush) {
                        Symfony\Component\HttpFoundation\flush();
                    }
                }
            },
        };
    };
    $argument = static function (array $spec, array &$counter) use ($callback) {
        return match ($spec[0]) {
            'null' => null,
            'chunks' => array_map('base64_decode', $spec[1]),
            default => $callback($spec, $counter),
        };
    };

    $statuses = [200, 200, 200, 201, 204, 304, 404, 500, 100, 103, 101];
    $headerNames = ['Content-Type', 'Content-Length', 'Transfer-Encoding', 'Cache-Control', 'ETag', 'X-Foo'];
    $headerValues = [
        'Content-Type' => ['text/plain', 'application/json', 'text/event-stream', null],
        'Content-Length' => ['123', '0', null],
        'Transfer-Encoding' => ['chunked', null],
        'Cache-Control' => ['max-age=600, public', 'no-cache', null],
        'ETag' => ['"x"', null],
        'X-Foo' => ['bar', null],
    ];
    $requestSpec = static fn () => [$pick(['GET', 'GET', 'HEAD', 'POST']), $pick(['HTTP/1.0', 'HTTP/1.1', null]), $pick([null, '"x"', '"y"', '*'])];
    $buildRequest = static function (array $spec): Request {
        $server = ['REQUEST_METHOD' => $spec[0]];
        if (null !== $spec[1]) {
            $server['SERVER_PROTOCOL'] = $spec[1];
        }
        if (null !== $spec[2]) {
            $server['HTTP_IF_NONE_MATCH'] = $spec[2];
        }

        return new Request([], [], [], [], [], $server);
    };

    $state = static function (Response $r): array {
        $readers = [
            'string' => static fn () => (string) $r,
            'status' => static fn () => $r->getStatusCode(),
            'version' => static fn () => $r->getProtocolVersion(),
            'content' => static fn () => $r->getContent(),
            'headers' => static fn () => $r->headers->allPreserveCase(),
            'callback' => static fn () => null !== $r->getCallback(),
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
            $result = ['throws', $e::class, $e->getMessage()];
        }
        Symfony\Component\HttpFoundation\recorder_drain();
        ob_end_clean();

        return [$result, $GLOBALS['recorder_calls']];
    };

    $scripts = [];
    for ($n = 0; $n < 1500; ++$n) {
        $new = [$callbackSpec(), $pick($statuses), []];
        for ($i = mt_rand(0, 2); $i > 0; --$i) {
            $name = $pick($headerNames);
            $value = $pick($headerValues[$name]);
            if (null !== $value) {
                $new[2][] = [$name, $value];
            }
        }
        $ops = [];
        for ($i = mt_rand(1, 10); $i > 0; --$i) {
            $op = $pick(['prepare', 'prepare', 'not_modified', 'is_not_modified', 'send_headers', 'send_headers', 'send_content', 'send_content', 'send', 'send', 'set_content', 'get_content', 'string', 'invoke', 'invoke', 'set_callback', 'set_chunks', 'status', 'header']);
            $ops[] = match ($op) {
                'prepare', 'is_not_modified' => [$op, $requestSpec()],
                'not_modified', 'send_content', 'get_content', 'string', 'invoke' => [$op],
                'send_headers' => ['send_headers', 0 === mt_rand(0, 2) ? $pick([103, 100, 199, 200, 404]) : null, 0 === mt_rand(0, 5)],
                'send' => ['send', 0 !== mt_rand(0, 2), 0 === mt_rand(0, 5)],
                'set_content' => ['set_content', 0 === mt_rand(0, 1) ? null : base64_encode($pick($texts))],
                'set_callback' => ['set_callback', $pick([['count'], ['throw', base64_encode('x')], ['parts', [[base64_encode('cb'), true]]]])],
                'set_chunks' => ['set_chunks', array_map('base64_encode', array_map(static fn () => $pick($texts), range(1, mt_rand(1, 3))))],
                'status' => ['status', $pick($statuses)],
                'header' => (static function () use ($pick, $headerNames, $headerValues) {
                    $name = $pick($headerNames);

                    return ['header', $name, $pick($headerValues[$name])];
                })(),
            };
        }

        $counter = [0];
        $headers = [];
        foreach ($new[2] as [$name, $value]) {
            $headers[$name] = $value;
        }
        $GLOBALS['recorder_calls'] = [];
        try {
            $response = new StreamedResponse($argument($new[0], $counter), $new[1], $headers);
        } catch (\Throwable $e) {
            $scripts[] = ['new' => $new, 'throws' => [$e::class, $e->getMessage()]];
            continue;
        }

        $results = [];
        foreach ($ops as $op) {
            $GLOBALS['recorder_headers_sent'] = false;
            $f = match ($op[0]) {
                'prepare' => static fn () => $response->prepare($buildRequest($op[1])),
                'is_not_modified' => static fn () => $response->isNotModified($buildRequest($op[1])),
                'not_modified' => static fn () => $response->setNotModified(),
                'send_headers' => static function () use ($response, $op) {
                    $GLOBALS['recorder_headers_sent'] = $op[2];

                    return $response->sendHeaders($op[1]);
                },
                'send_content' => static fn () => $response->sendContent(),
                'send' => static function () use ($response, $op) {
                    $GLOBALS['recorder_headers_sent'] = $op[2];

                    return $response->send($op[1]);
                },
                'set_content' => static fn () => $response->setContent(null === $op[1] ? null : base64_decode($op[1])),
                'get_content' => static fn () => $response->getContent(),
                'string' => static fn () => (string) $response,
                'invoke' => static function () use ($response) {
                    $callback = $response->getCallback();
                    if (null === $callback) {
                        return null;
                    }
                    $callback();

                    return true;
                },
                'set_callback' => static function () use ($response, $op, $callback, &$counter) {
                    return $response->setCallback($callback($op[1], $counter));
                },
                'set_chunks' => static fn () => $response->setChunks(array_map('base64_decode', $op[1])),
                'status' => static fn () => $response->setStatusCode($op[1]),
                'header' => static fn () => $response->headers->set($op[1], $op[2]),
            };
            $results[] = $call($response, $f);
        }

        $scripts[] = ['new' => $new, 'ops' => $ops, 'results' => $results, 'state' => $state($response)];
    }

    echo json_encode(['scripts' => $scripts], \JSON_THROW_ON_ERROR), "\n";
}
