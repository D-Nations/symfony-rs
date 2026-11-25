<?php
// JsonResponse and RedirectResponse answers for what their tests leave
// open, for crates/http-foundation/tests/data/json_redirect.json
// (scripts/record.sh http-foundation-json-redirect):
//
// "json": 1,500 seeded scripts: a constructor (new JsonResponse($data,
// $status, $headers, $json) or fromJsonString()) with values of every
// kind, then up to eight setData(), setJson(), setCallback(),
// setEncodingOptions(), Content-Type and setContent() calls; each call's
// result, then the status, content, encoding options and headers
// (allPreserveCase() without Date).
// "callbacks": setCallback() of hand-picked and generated names (Unicode
// categories, accessors, escapes, reserved words, dots, a final newline,
// invalid UTF-8): the content or the exception.
// "redirect": 1,000 seeded new RedirectResponse($url, $status, $headers)
// calls, some followed by setTargetUrl(): the exception, or the target,
// content, status and headers (without Date).
//
// A result is ["self"] or ["throws", class, message, code, previous class
// or null]. Texts are base64 (lib/typed.php).

error_reporting(E_ALL);

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpFoundation\Response;

enum UnitCase { case A; }
enum StringCase: string { case X = '<x>'; }

final class Text implements \Stringable
{
    public function __construct(private string $text)
    {
    }

    public function __toString(): string
    {
        return $this->text;
    }
}

/** A typed value, with a Text as ["stringable", base64]. */
function typed(mixed $v): array
{
    if ($v instanceof Text) {
        return ['stringable', base64_encode((string) $v)];
    }
    if (\is_array($v)) {
        $pairs = [];
        foreach ($v as $k => $item) {
            $pairs[] = [typed_value($k), typed($item)];
        }

        return ['array', $pairs];
    }

    return typed_value($v);
}

mt_srand(20261125);
$pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];

$texts = ['', 'foo', '<>\'&"', 'é', "\xff", "a\xb1\x31", '/', "\u{2028}", '😀', '1', '1.5', ' 2 ', '{"a":1}', '[1,2]', '{}', '[]', '{"0":{"0":1}}', 'null', 'true', '"x"', '{"a":', 'x', '{"\u0000a":1}', '{"a":{}}', '[{"b":[]}]', '1e400', '9223372036854775808', '{"1":1,"a":[1.0,-0.0]}'];
$scalars = [null, true, false, 0, 1, -7, PHP_INT_MAX, 0.1, 1.0, -0.0, 1e25, 1/3, INF, NAN, UnitCase::A, StringCase::X];

function data(int $depth): mixed
{
    global $texts, $scalars, $pick;
    if ($depth <= 0 || 0 === mt_rand(0, 2)) {
        return 0 === mt_rand(0, 1) ? $pick($texts) : $pick($scalars);
    }
    $items = [];
    $assoc = 0 === mt_rand(0, 1);
    for ($i = mt_rand(0, 3); $i > 0; --$i) {
        if ($assoc) {
            $items[$pick(['a', 'b', '0', '1', '', 'é', "\xff", '<k>'])] = data($depth - 1);
        } else {
            $items[] = data($depth - 1);
        }
    }

    return match (mt_rand(0, 5)) {
        0 => (object) $items,
        1 => new \ArrayObject($items),
        default => $items,
    };
}

$optionSets = [0, 15, JSON_PRETTY_PRINT, JSON_FORCE_OBJECT, JSON_THROW_ON_ERROR, 15 | JSON_THROW_ON_ERROR, JSON_PARTIAL_OUTPUT_ON_ERROR, JSON_PARTIAL_OUTPUT_ON_ERROR | JSON_THROW_ON_ERROR, JSON_INVALID_UTF8_SUBSTITUTE, JSON_INVALID_UTF8_IGNORE, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES, JSON_PRESERVE_ZERO_FRACTION, JSON_NUMERIC_CHECK, 15 | JSON_PRETTY_PRINT | JSON_FORCE_OBJECT];
$contentTypes = ['text/javascript', 'application/json', 'application/vnd.acme+json', 'TEXT/JAVASCRIPT', 'text/javascript; charset=utf-8', '', null];
$headerSets = [[], [], ['ETag' => 'foo'], ['Content-Type' => 'application/vnd.acme.blog-v1+json'], ['content-type' => 'text/javascript'], ['Content-Type' => ''], ['Cache-Control' => 'max-age=10'], ['Content-Type' => ['text/javascript', 'x/y']]];
$statuses = [200, 200, 200, 201, 202, 204, 400, 404, 500, 99, 600];
$callbacks = ['callback', 'foo.bar', 'ಠ_ಠ["foo"].bar[0]', '+invalid', 'a b', 'return', 'foo.return', 'jQuery123_456', '$', '_', '', 'a..b', "cb\n", "\xff"];

$headersOut = static function (Response $r): array {
    $all = $r->headers->allPreserveCase();
    unset($all['Date']);

    return typed_value($all);
};
$throws = static fn (\Throwable $e) => ['throws', $e::class, $e->getMessage(), $e->getCode(), null === $e->getPrevious() ? null : $e->getPrevious()::class];

$json = [];
for ($n = 0; $n < 1500; ++$n) {
    $kind = $pick(['new', 'new', 'new', 'json', 'from']);
    $data = match ($kind) {
        'new' => 0 === mt_rand(0, 4) ? null : data(mt_rand(0, 3)),
        'json' => $pick([$pick($texts), $pick($texts), 5, -1.5, 0.1, 1/3, true, null, [], new \stdClass(), new Text('{"t":1}'), new Text('')]),
        'from' => $pick($texts),
    };
    $status = 0 === mt_rand(0, 3) ? $pick($statuses) : 200;
    $headers = $pick($headerSets);
    $new = [$kind, typed($data), $status, typed_value($headers)];
    try {
        $response = 'from' === $kind ? JsonResponse::fromJsonString($data, $status, $headers) : new JsonResponse($data, $status, $headers, 'json' === $kind);
    } catch (\Throwable $e) {
        $json[] = ['new' => $new, 'throws' => $throws($e)];
        continue;
    }
    $ops = [];
    $results = [];
    for ($i = mt_rand(0, 8); $i > 0; --$i) {
        $op = match ($pick(['data', 'data', 'json', 'callback', 'callback', 'options', 'options', 'content_type', 'content'])) {
            'data' => ['data', typed(data(mt_rand(0, 3)))],
            'json' => ['json', base64_encode($pick($texts))],
            'callback' => ['callback', 0 === mt_rand(0, 3) ? null : base64_encode($pick($callbacks))],
            'options' => ['options', $pick($optionSets)],
            'content_type' => ['content_type', (static fn ($v) => null === $v ? null : base64_encode($v))($pick($contentTypes))],
            'content' => ['content', base64_encode($pick(['', 'raw', '{"x":2}']))],
        };
        $ops[] = $op;
        try {
            $result = match ($op[0]) {
                'data' => $response->setData(data_from($op[1])),
                'json' => $response->setJson(base64_decode($op[1])),
                'callback' => $response->setCallback(null === $op[1] ? null : base64_decode($op[1])),
                'options' => $response->setEncodingOptions($op[1]),
                'content_type' => $response->headers->set('Content-Type', null === $op[1] ? null : base64_decode($op[1])),
                'content' => $response->setContent(base64_decode($op[1])),
            };
            $results[] = $result === $response ? ['self'] : typed_value($result);
        } catch (\Throwable $e) {
            $results[] = $throws($e);
        }
    }
    $json[] = ['new' => $new, 'ops' => $ops, 'results' => $results, 'state' => [$response->getStatusCode(), base64_encode($response->getContent()), $response->getEncodingOptions(), $headersOut($response)]];
}

/** The PHP value of a typed() value. */
function data_from(array $t): mixed
{
    return match ($t[0]) {
        'null' => null,
        'bool', 'int' => $t[1],
        'float' => unpack('E', hex2bin($t[1]))[1],
        'string' => base64_decode($t[1]),
        'stringable' => new Text(base64_decode($t[1])),
        'array' => (static function () use ($t) {
            $a = [];
            foreach ($t[1] as [$k, $v]) {
                $a[data_from($k)] = data_from($v);
            }

            return $a;
        })(),
        'object' => (object) data_from(['array', $t[2]]),
        'arrayobject' => new \ArrayObject(data_from(['array', $t[1]])),
        'enum' => \constant($t[1].'::'.$t[2]),
    };
}

// setCallback() names
$parts = ['a', 'Z', 'é', 'ಠ', '中', 'ß', 'ǅ', 'ʰ', 'ª', '$', '_', '0', '9', '١', "\u{0301}", "\u{0903}", '‿', "\u{200C}", "\u{200D}", '😀', '+', '-', ' ', '(', ')', "\n", '[0]', '[12]', '["x"]', "['x']", '["a\\"b"]', "['a\\'b']", '["\\\\"]', '[x]', '[]', '[-1]', '[١]', '["]', "[\"\xff\"]", '[0][1]', '["a"]["b"]', '.', '..', "\xff", "\xc3", "\u{2028}", '\\u0041', 'Ⅻ', '〇', 'ー'];
$names = ['callback', 'foo.bar', 'ಠ_ಠ["foo"].bar[0]', '+invalid', 'a b', '', '.', 'a.', '.a', 'a..b', "cb\n", "cb\n\n", "\ncb", "cb\r", "\xff", 'jQuery123_456', '$', '_', '$.fn', '__proto__', 'constructor', 'a["b"]', "a['b']", 'a[0]', 'a[0]extra', 'a[0]["x"].b', 'a["x\\"]', 'a["x"', 'a[0', 'a]', '0a', '١a', 'a١', "a\u{0301}", "\u{0301}a", 'ǅx', 'ʰx', 'Ⅻ', '〇', 'ー', 'a‿b', "a\u{200C}b", "\u{200C}a", 'a😀', 'a-b', 'a+b', 'a(b)', 'alert(1)', 'a;b', 'a/b', 'a\\u0041'];
foreach (['break', 'do', 'instanceof', 'typeof', 'case', 'else', 'new', 'var', 'catch', 'finally', 'return', 'void', 'continue', 'for', 'switch', 'while', 'debugger', 'function', 'this', 'with', 'default', 'if', 'throw', 'delete', 'in', 'try', 'class', 'enum', 'extends', 'super', 'const', 'export', 'import', 'implements', 'let', 'private', 'public', 'yield', 'interface', 'package', 'protected', 'static', 'null', 'true', 'false', 'await', 'async', 'undefined', 'NULL', 'True', 'Return'] as $word) {
    $names[] = $word;
    $names[] = 'a.'.$word;
    $names[] = $word.'.a';
    $names[] = $word.'x';
}
for ($n = 0; $n < 3000; ++$n) {
    $name = '';
    for ($i = mt_rand(1, 5); $i > 0; --$i) {
        $name .= $pick($parts);
    }
    $names[] = $name;
}
$callbackResults = [];
foreach ($names as $name) {
    $response = new JsonResponse(['a' => 1]);
    try {
        $response->setCallback($name);
        $result = ['ok', base64_encode($response->getContent())];
    } catch (\Throwable $e) {
        $result = $throws($e);
    }
    $callbackResults[] = [base64_encode($name), $result];
}

// RedirectResponse
$urls = ['foo.bar', 'foo.bar', '/path', '/path?a=1&b=2', 'http://example.com/a"b<c>\'d&amp;', 'https://x.test/é?q=😀', "\xff", "a\xc3", "a\nb", "a\r\nLocation: x", ' ', '0', '', '', "javascript:alert('x')", '%3Cscript%3E', '&lt;', "\u{2028}", "a\x00b", '//evil.test', "\xed\xa0\x80", "\xef\xbf\xbe"];
$redirectStatuses = [302, 302, 301, 301, 303, 307, 308, 201, 300, 304, 305, 200, 404, 500, 100, 99, 600, 0];
$redirectHeaders = [[], [], ['cache-control' => 'max-age=86400'], ['Cache-Control' => 'max-age=86400'], ['CACHE-CONTROL' => 'no-store'], ['cache_control' => 'max-age=5'], ['Cache-Control' => ''], ['Cache-Control' => null], ['Location' => '/other'], ['Content-Type' => 'text/plain'], ['ETag' => '"x"'], ['Expires' => 'Thu, 01 Jan 2099 00:00:00 GMT'], ['Last-Modified' => 'Thu, 01 Jan 2015 00:00:00 GMT'], ['Cache-Control' => 'public', 'ETag' => 'x'], ['X-Foo' => ['a', 'b']]];
$redirect = [];
for ($n = 0; $n < 1000; ++$n) {
    $url = $pick($urls);
    $status = $pick($redirectStatuses);
    $headers = $pick($redirectHeaders);
    $then = 0 === mt_rand(0, 3) ? $pick($urls) : null;
    $case = [base64_encode($url), $status, typed_value($headers), null === $then ? null : base64_encode($then)];
    try {
        $response = new RedirectResponse($url, $status, $headers);
        $after = null;
        if (null !== $then) {
            try {
                $after = $response->setTargetUrl($then) === $response ? ['self'] : ['other'];
            } catch (\Throwable $e) {
                $after = $throws($e);
            }
        }
        $case[] = ['ok', $after, base64_encode($response->getTargetUrl()), base64_encode($response->getContent()), $response->getStatusCode(), $headersOut($response)];
    } catch (\Throwable $e) {
        $case[] = $throws($e);
    }
    $redirect[] = $case;
}

echo json_encode(['json' => $json, 'callbacks' => $callbackResults, 'redirect' => $redirect], \JSON_THROW_ON_ERROR), "\n";
