<?php
// EventStreamResponse and ServerEvent answers for what their tests leave
// open, for crates/http-foundation/tests/data/event_stream.json
// (scripts/record.sh http-foundation-event-stream): 1,200 seeded scripts.
// Each builds an EventStreamResponse from headers (keys in any case), a
// retry, and a callback that yields events, sends them itself with
// sendEvent() (setting the response's retry between some), or none; then
// calls send() (once or twice), getRetry(), setRetry(), getCallback()
// and (string). Events take every field from pools of PHP-truthy and
// falsy values, line breaks and retries <= 0; connection_aborted() is
// scripted to turn true after a given event. Recorded: each call's
// result, the echoed output and header() lines, then the headers and
// the retry. PHP_SAPI is the CLI's (a constant), so sendEvent() does not
// flush here. Texts are base64 (lib/typed.php).

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
        return false;
    }

    function ob_get_status(bool $full_status = false): array
    {
        return [];
    }

    function connection_aborted(): int
    {
        ++$GLOBALS['recorder_events'];

        return null !== $GLOBALS['recorder_abort_after'] && $GLOBALS['recorder_events'] >= $GLOBALS['recorder_abort_after'] ? 1 : 0;
    }
}

namespace {
    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';
    require __DIR__.'/lib/typed.php';

    use Symfony\Component\HttpFoundation\EventStreamResponse;
    use Symfony\Component\HttpFoundation\ServerEvent;

    mt_srand(20261127);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];

    $texts = ['', '0', 'foo', 'bar baz', "a\nb", "a\r\nb\rc", "x\n", "\n", '00'];
    $fields = [null, null, '', '0', 'x', "a\nb", "c\rd", '1'];
    $retries = [null, null, 0, -1, 100, 1500, 3000];
    $eventSpec = static function () use ($pick, $texts, $fields, $retries) {
        $data = 0 === mt_rand(0, 3)
            ? ['lines', array_map('base64_encode', array_map(static fn () => $pick($texts), range(1, mt_rand(0, 3))))]
            : ['text', base64_encode($pick($texts))];
        $b64 = static fn ($v) => null === $v ? null : base64_encode($v);

        return [$data, $b64($pick($fields)), $pick($retries), $b64($pick($fields)), $b64($pick($fields))];
    };
    $event = static function (array $spec): ServerEvent {
        $data = 'lines' === $spec[0][0] ? array_map('base64_decode', $spec[0][1]) : base64_decode($spec[0][1]);
        $d = static fn ($v) => null === $v ? null : base64_decode($v);

        return new ServerEvent($data, $d($spec[1]), $spec[2], $d($spec[3]), $d($spec[4]));
    };

    $headerKeys = ['Content-Type', 'content-type', 'Cache-Control', 'cache-control', 'X-Accel-Buffering', 'Expires', 'Connection', 'X-Foo'];
    $headerValues = ['text/plain', 'no-store', 'yes', '1', 'close', 'bar'];

    $scripts = [];
    for ($n = 0; $n < 1200; ++$n) {
        $headers = [];
        for ($i = mt_rand(0, 2); $i > 0; --$i) {
            $headers[] = [$pick($headerKeys), $pick($headerValues)];
        }
        $kind = $pick(['yield', 'yield', 'yield', 'send', 'null']);
        $events = array_map(static fn () => $eventSpec(), range(1, mt_rand(1, 4)));
        // For 'send': the response's retry set before event $setAt.
        $setAt = 'send' === $kind && 0 === mt_rand(0, 1) ? [mt_rand(0, \count($events) - 1), $pick([0, 500, 1500, 3000])] : null;
        $new = [$headers, $pick($retries), $kind, 'null' === $kind ? [] : $events, $setAt];
        $abortAfter = 0 === mt_rand(0, 3) ? mt_rand(1, 3) : null;
        $ops = [];
        for ($i = mt_rand(1, 4); $i > 0; --$i) {
            $ops[] = match ($pick(['send', 'send', 'get_retry', 'set_retry', 'callback', 'string'])) {
                'send' => ['send'],
                'get_retry' => ['get_retry'],
                'set_retry' => ['set_retry', $pick([0, -1, 100, 1500, 3000])],
                'callback' => ['callback'],
                'string' => ['string'],
            };
        }

        $array = [];
        foreach ($headers as [$k, $v]) {
            $array[$k] = $v;
        }
        $callback = match ($kind) {
            'null' => null,
            'yield' => static function () use ($events, $event) {
                foreach ($events as $spec) {
                    yield $event($spec);
                }
            },
            'send' => static function (EventStreamResponse $response) use ($events, $event, $setAt) {
                foreach ($events as $i => $spec) {
                    if (null !== $setAt && $setAt[0] === $i) {
                        $response->setRetry($setAt[1]);
                    }
                    $response->sendEvent($event($spec));
                }
            },
        };
        $GLOBALS['recorder_events'] = 0;
        $GLOBALS['recorder_abort_after'] = $abortAfter;
        $response = new EventStreamResponse($callback, 200, $array, $new[1]);

        $results = [];
        foreach ($ops as $op) {
            $GLOBALS['recorder_calls'] = [];
            ob_start();
            try {
                $result = match ($op[0]) {
                    'send' => $response->send() === $response ? ['self'] : ['other'],
                    'get_retry' => typed_value($response->getRetry()),
                    'set_retry' => typed_value($response->setRetry($op[1])),
                    'callback' => typed_value(null !== $response->getCallback()),
                    'string' => typed_value((string) $response),
                };
            } catch (\Throwable $e) {
                $result = ['throws', $e::class, $e->getMessage()];
            }
            $echo = ob_get_clean();
            $results[] = [$result, base64_encode($echo), $GLOBALS['recorder_calls']];
        }

        $scripts[] = [
            'new' => $new,
            'abort_after' => $abortAfter,
            'ops' => $ops,
            'results' => $results,
            'headers' => typed_value($response->headers->allPreserveCase()),
            'retry' => typed_value($response->getRetry()),
        ];
    }

    echo json_encode(['scripts' => $scripts], \JSON_THROW_ON_ERROR), "\n";
}
