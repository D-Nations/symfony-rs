<?php
// ResponseListener answers for crates/http-kernel/tests/data/response_listener.json
// (scripts/record.sh http-kernel-response-listener): 1,500 seeded scripts.
// Each builds a ResponseListener (a charset, with or without the
// Content-Language header), a request (URI, method, SERVER_PROTOCOL, a
// locale, a request format, a `_vary_by_language` attribute of any type)
// and a response (content, status, headers including Content-Type,
// Content-Language, Vary, Transfer-Encoding and Content-Length, a charset
// of its own or none), dispatches a ResponseEvent for a main or sub
// request through an EventDispatcher, and records what came back: the
// exception, or the response's status, protocol version, charset, content
// and headers (Date taken out: it is the clock).

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\EventDispatcher\EventDispatcher;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Event\ResponseEvent;
use Symfony\Component\HttpKernel\EventListener\ResponseListener;
use Symfony\Component\HttpKernel\HttpKernelInterface;
use Symfony\Component\HttpKernel\KernelEvents;

mt_srand(20261125);

function pick(array $a): mixed
{
    return $a[mt_rand(0, \count($a) - 1)];
}

function chance(int $percent): bool
{
    return mt_rand(1, 100) <= $percent;
}

final class StubKernel implements HttpKernelInterface
{
    public function handle(Request $request, int $type = self::MAIN_REQUEST, bool $catch = true): Response
    {
        return new Response();
    }
}

function gen_script(): array
{
    $headers = [];
    if (chance(60)) {
        $headers[] = ['Content-Type', pick(['text/plain', 'text/html', 'TEXT/CSV', 'text/html; charset=ISO-8859-1', 'application/json', 'image/png', 'text/plain;Charset=utf-8'])];
    }
    if (chance(30)) {
        $headers[] = ['Content-Language', pick(['', 'mi, en', 'fr'])];
    }
    if (chance(25)) {
        $headers[] = ['Vary', pick(['Cookie', 'Accept-Language', 'accept-language, Cookie', ''])];
    }
    if (chance(15)) {
        $headers[] = ['Transfer-Encoding', 'chunked'];
    }
    if (chance(20)) {
        $headers[] = ['Content-Length', pick(['3', '0', '10'])];
    }
    if (chance(10)) {
        $headers[] = ['Cache-Control', pick(['no-cache', 'public, max-age=60'])];
    }

    return [
        'listener' => [
            'charset' => pick(['UTF-8', 'ISO-8859-15', 'utf-8', '']),
            'content_language' => chance(50),
        ],
        'main' => chance(85),
        'request' => [
            'uri' => pick(['/', '/a?x=1', 'http://example.com/p', 'https://example.com:8443/q']),
            'method' => pick(['GET', 'GET', 'HEAD', 'POST']),
            'protocol' => pick(['HTTP/1.0', 'HTTP/1.1', 'HTTP/1.1', 'HTTP/2.0']),
            'locale' => chance(50) ? pick(['fr', 'de_DE', '', 'en']) : null,
            'format' => chance(30) ? pick(['json', 'html', 'txt', 'application/json']) : null,
            'vary' => chance(60) ? [pick([true, false, 0, 1, '0', '', 'yes', null, [], [0], 0.0, 'false'])] : null,
        ],
        'response' => [
            'content' => pick(['', 'foo', '0', 'content']),
            'status' => pick([100, 103, 200, 200, 200, 201, 204, 206, 301, 304, 404, 500]),
            'headers' => $headers,
            'charset' => chance(25) ? pick(['ISO-8859-1', 'utf-8', '']) : null,
        ],
    ];
}

function run_script(array $s): array
{
    $dispatcher = new EventDispatcher();
    $listener = new ResponseListener($s['listener']['charset'], $s['listener']['content_language']);
    $dispatcher->addListener(KernelEvents::RESPONSE, $listener->onKernelResponse(...));

    $r = $s['request'];
    $request = Request::create($r['uri'], $r['method'], [], [], [], ['SERVER_PROTOCOL' => $r['protocol']]);
    if (null !== $r['locale']) {
        $request->setLocale($r['locale']);
    }
    if (null !== $r['format']) {
        $request->setRequestFormat($r['format']);
    }
    if (null !== $r['vary']) {
        $request->attributes->set('_vary_by_language', $r['vary'][0]);
    }

    $p = $s['response'];
    $response = new Response($p['content'], $p['status']);
    foreach ($p['headers'] as [$name, $value]) {
        $response->headers->set($name, $value);
    }
    if (null !== $p['charset']) {
        $response->setCharset($p['charset']);
    }

    $event = new ResponseEvent(new StubKernel(), $request, $s['main'] ? HttpKernelInterface::MAIN_REQUEST : HttpKernelInterface::SUB_REQUEST, $response);
    try {
        $dispatcher->dispatch($event, KernelEvents::RESPONSE);
    } catch (\Throwable $e) {
        return ['throws' => [$e::class, $e->getMessage()]];
    }

    $response = $event->getResponse();
    $headers = [];
    foreach ($response->headers->allPreserveCaseWithoutCookies() as $name => $values) {
        if ('Date' === $name) {
            continue;
        }
        $headers[] = [$name, $values];
    }

    return [
        'status' => $response->getStatusCode(),
        'version' => $response->getProtocolVersion(),
        'charset' => $response->getCharset(),
        'content' => base64_encode($response->getContent()),
        'headers' => $headers,
    ];
}

$scripts = [];
for ($i = 0; $i < 1500; ++$i) {
    $s = gen_script();
    $vary = $s['request']['vary'];
    $s['request']['vary'] = null === $vary ? null : typed_value($vary[0]);
    $outcome = run_script(['request' => ['vary' => $vary] + $s['request']] + $s);
    $scripts[] = ['script' => $s, 'outcome' => $outcome];
}

echo json_encode(['scripts' => $scripts], \JSON_UNESCAPED_SLASHES), "\n";
