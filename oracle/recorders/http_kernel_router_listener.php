<?php
// RouterListener answers for crates/http-kernel/tests/data/router_listener.json
// (scripts/record.sh http-kernel-router-listener): 2,000 seeded scripts. Each
// builds a RouterListener on a UrlMatcherInterface, a RequestMatcherInterface
// or a matcher that is both (with or without its own RequestContext), then
// runs a chain of one to three nested requests through onKernelRequest()
// (each pushed on the request stack first, as the kernel does) and unwinds
// it with onKernelFinishRequest() given the FinishRequestEvent or null.
// Matchers return generated parameters (with _route, _controller and
// _route_mapping of any shape) or throw routing exceptions. Recorded: the
// constructor's exception, every call's exception (class, message, code,
// headers, previous), the request attributes, the matcher calls, the logger
// calls, and the listener's context after each step. Then onKernelException()
// for a generated throwable, in debug mode or not: whether it sets a
// response, its status and whether its page is the welcome page recorded
// once at the top, with the hue and project directory taken out.

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Psr\Log\AbstractLogger;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\RequestStack;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Event\ExceptionEvent;
use Symfony\Component\HttpKernel\Event\FinishRequestEvent;
use Symfony\Component\HttpKernel\Event\RequestEvent;
use Symfony\Component\HttpKernel\EventListener\RouterListener;
use Symfony\Component\HttpKernel\Exception\BadRequestHttpException;
use Symfony\Component\HttpKernel\Exception\HttpExceptionInterface;
use Symfony\Component\HttpKernel\Exception\MethodNotAllowedHttpException;
use Symfony\Component\HttpKernel\Exception\NotFoundHttpException;
use Symfony\Component\HttpKernel\HttpKernelInterface;
use Symfony\Component\Routing\Exception\MethodNotAllowedException;
use Symfony\Component\Routing\Exception\NoConfigurationException;
use Symfony\Component\Routing\Exception\ResourceNotFoundException;
use Symfony\Component\Routing\Exception\RouteNotFoundException;
use Symfony\Component\Routing\Matcher\RequestMatcherInterface;
use Symfony\Component\Routing\Matcher\UrlMatcherInterface;
use Symfony\Component\Routing\RequestContext;
use Symfony\Component\Routing\RequestContextAwareInterface;

mt_srand(20261125);

function pick(array $a): mixed
{
    return $a[mt_rand(0, \count($a) - 1)];
}

function chance(int $percent): bool
{
    return mt_rand(1, 100) <= $percent;
}

// --- generators -----------------------------------------------------------

function gen_uri(): string
{
    $scheme = pick(['http', 'https', '']);
    $host = pick(['localhost', 'example.com', 'www.symfony.com', 'EXAMPLE.org', '127.0.0.1', '[::1]']);
    $port = pick(['', '', ':80', ':443', ':8080', ':90']);
    $path = pick(['/', '/path', '/foo/bar', '/a%20b', '/caf%C3%A9/', '', '/index.php/x']);
    $query = pick(['', '', '?a=1', '?b=2&a=1', '?x']);

    return '' === $scheme ? $path.$query : $scheme.'://'.$host.$port.$path.$query;
}

function gen_scalar(): mixed
{
    return pick(['vienna-2024', 'abc123', '', '0', 'x y', 5, 0, -1, 1.5, true, false, null]);
}

function gen_key(): int|string
{
    return pick(['slug', 'id', 'date', 'a', 'b', 'conference', 'article', 0, 1, 7, '_route', '_controller', '_format']);
}

function gen_value(int $depth = 0): mixed
{
    if ($depth < 1 && chance(15)) {
        $a = [];
        for ($i = mt_rand(0, 2); $i > 0; --$i) {
            $a[gen_key()] = gen_scalar();
        }

        return $a;
    }

    return gen_scalar();
}

function gen_mapping_entry(): mixed
{
    return match (mt_rand(1, 12)) {
        1, 2, 3, 4 => pick(['conference', 'article', 'a', 'b', 'slug', 'id', '7', '0']),
        5, 6, 7 => [pick(['conference', 'article', 'a', 'b']), pick(['slug', 'id', 'date', 'a', 'b', 7, '0'])],
        8 => [pick(['article', 'a'])],
        9 => [],
        10 => pick([null, true, false, 3, 1.5, '']),
        11 => [pick(['a', 'b']), [pick(['q'])]],
        default => [pick([null, 2, true]), pick(['x', null])],
    };
}

function gen_mapping(array $parameters): mixed
{
    if (chance(8)) {
        return pick(['ab', 'slug', 0, 5, '', '0', true, false, [], null]);
    }
    $mapping = [];
    $keys = array_keys($parameters);
    for ($i = mt_rand(1, 4); $i > 0; --$i) {
        $key = $keys && chance(80) ? pick($keys) : gen_key();
        $mapping[$key] = gen_mapping_entry();
    }

    return $mapping;
}

function gen_parameters(): array
{
    $parameters = [];
    if (chance(70)) {
        $parameters['_route'] = pick(['foo', 'blog_show', '', null, 3]);
    }
    if (chance(30)) {
        $parameters['_controller'] = pick(['App\\Controller::index', null, '']);
    }
    for ($i = mt_rand(0, 4); $i > 0; --$i) {
        $parameters[gen_key()] = gen_value();
    }
    if (chance(55)) {
        $parameters['_route_mapping'] = gen_mapping($parameters);
    }

    return $parameters;
}

/// What the matcher answers for one request: parameters or an exception.
function gen_result(): array
{
    return match (mt_rand(1, 20)) {
        1, 2 => ['throws', 'ResourceNotFound', pick(['', 'No routes found for "/path".'])],
        3 => ['throws', 'NoConfiguration', ''],
        4, 5 => ['throws', 'MethodNotAllowed', [pick(['POST', 'get', 'PUT']), ...(chance(50) ? ['delete'] : [])]],
        6 => ['throws', pick(['RouteNotFound', 'RuntimeException']), 'boom'],
        default => ['returns', gen_parameters()],
    };
}

function make_exception(array $result): \Throwable
{
    return match ($result[1]) {
        'ResourceNotFound' => new ResourceNotFoundException($result[2]),
        'NoConfiguration' => new NoConfigurationException(),
        'MethodNotAllowed' => new MethodNotAllowedException($result[2]),
        'RouteNotFound' => new RouteNotFoundException($result[2]),
        'RuntimeException' => new \RuntimeException($result[2]),
    };
}

// --- doubles ----------------------------------------------------------------

final class Queue
{
    public array $results = [];
    public array $calls = [];

    public function next(string $call): array
    {
        $this->calls[] = $call;
        $result = array_shift($this->results) ?? ['returns', []];
        if ('throws' === $result[0]) {
            throw make_exception($result);
        }

        return $result[1];
    }
}

final class UrlStub implements UrlMatcherInterface
{
    public function __construct(private RequestContext $context, private Queue $queue)
    {
    }

    public function setContext(RequestContext $context): void
    {
        $this->context = $context;
    }

    public function getContext(): RequestContext
    {
        return $this->context;
    }

    public function match(string $pathinfo): array
    {
        return $this->queue->next(base64_encode($pathinfo));
    }
}

final class RequestStub implements RequestMatcherInterface
{
    public function __construct(private Queue $queue)
    {
    }

    public function matchRequest(Request $request): array
    {
        return $this->queue->next('request');
    }
}

final class BothStub implements RequestMatcherInterface, RequestContextAwareInterface
{
    public function __construct(private RequestContext $context, private Queue $queue)
    {
    }

    public function setContext(RequestContext $context): void
    {
        $this->context = $context;
    }

    public function getContext(): RequestContext
    {
        return $this->context;
    }

    public function matchRequest(Request $request): array
    {
        return $this->queue->next('request');
    }
}

final class Logger extends AbstractLogger
{
    public array $calls = [];

    public function log($level, \Stringable|string $message, array $context = []): void
    {
        $this->calls[] = [$level, base64_encode((string) $message), typed_pairs($context)];
    }
}

final class Kernel implements HttpKernelInterface
{
    public function handle(Request $request, int $type = self::MAIN_REQUEST, bool $catch = true): Response
    {
        return new Response();
    }
}

// --- recording ----------------------------------------------------------------

function context_state(RequestContext $c): array
{
    return [
        base64_encode($c->getBaseUrl()),
        base64_encode($c->getPathInfo()),
        $c->getMethod(),
        base64_encode($c->getHost()),
        $c->getScheme(),
        $c->getHttpPort(),
        $c->getHttpsPort(),
        base64_encode($c->getQueryString()),
    ];
}

function throwable(\Throwable $e): array
{
    $previous = $e->getPrevious();

    return [
        $e::class,
        base64_encode($e->getMessage()),
        $e->getCode(),
        $e instanceof HttpExceptionInterface ? $e->getStatusCode() : null,
        $e instanceof HttpExceptionInterface ? typed_pairs($e->getHeaders()) : null,
        $previous ? [$previous::class, base64_encode($previous->getMessage())] : null,
    ];
}

/// The welcome page with the per-response parts taken out.
function normalize_welcome(string $content): string
{
    $content = preg_replace('/--hue: \d+;/', '--hue: @@HUE@@;', $content, 1);

    return preg_replace('#(class="notranslate project_dir_path">)[^<]*(</code>)#', '$1@@PROJECT_DIR@@$2', $content, 1);
}

function gen_request(): array
{
    $headers = [];
    if (chance(15)) {
        $headers[] = ['host', base64_encode(pick(['bad host %22', 'exa mple.com', 'foo.com:8080', 'Example.COM', '###']))];
    }
    if (chance(30)) {
        $headers[] = ['referer', base64_encode(pick(['https://www.google.com', '0', '', 'http://x/?a="b"']))];
    }

    return [
        'uri' => base64_encode(gen_uri()),
        'method' => pick(['GET', 'GET', 'post', 'HEAD', 'PUT']),
        'headers' => $headers,
        'preset_controller' => chance(10),
        'result' => gen_result(),
    ];
}

function build_request(array $r): Request
{
    $request = Request::create(base64_decode($r['uri']), $r['method']);
    foreach ($r['headers'] as [$name, $value]) {
        $request->headers->set($name, base64_decode($value));
    }
    if ($r['preset_controller']) {
        $request->attributes->set('_controller', null);
    }

    return $request;
}

function encode_result(array $result): array
{
    return 'returns' === $result[0]
        ? ['returns', typed_value($result[1])]
        : ['throws', $result[1], 'MethodNotAllowed' === $result[1] ? $result[2] : base64_encode($result[2])];
}

function gen_exception(): array
{
    return match (mt_rand(1, 6)) {
        1, 2 => ['NotFoundHttp', 'NoConfiguration'],
        3 => ['NotFoundHttp', 'ResourceNotFound'],
        4 => ['NotFoundHttp', null],
        5 => ['MethodNotAllowedHttp', 'NoConfiguration'],
        default => ['BadRequestHttp', 'NoConfiguration'],
    };
}

function build_exception(array $spec): \Throwable
{
    $previous = match ($spec[1]) {
        'NoConfiguration' => new NoConfigurationException(),
        'ResourceNotFound' => new ResourceNotFoundException(),
        null => null,
    };

    return match ($spec[0]) {
        'NotFoundHttp' => new NotFoundHttpException('', $previous),
        'MethodNotAllowedHttp' => new MethodNotAllowedHttpException(['GET'], '', $previous),
        'BadRequestHttp' => new BadRequestHttpException('', $previous),
    };
}

$scripts = [];
$welcome = null;
for ($n = 0; $n < 2000; ++$n) {
    $script = [
        'matcher' => pick(['url', 'request', 'both']),
        'pass_context' => chance(60),
        'ports' => [pick([80, 8080]), pick([443, 8443])],
        'logger' => chance(50),
        'debug' => chance(70),
        'requests' => [],
        'finish' => [],
        'exception' => gen_exception(),
    ];
    for ($i = mt_rand(1, 3); $i > 0; --$i) {
        $script['requests'][] = gen_request();
        $script['finish'][] = pick(['event', 'event', 'null']);
    }

    $queue = new Queue();
    foreach ($script['requests'] as $r) {
        $queue->results[] = $r['result'];
    }
    $matcherContext = new RequestContext();
    $matcherContext->setHttpPort($script['ports'][0])->setHttpsPort($script['ports'][1]);
    $passed = $script['pass_context'] ? (new RequestContext())->setHttpPort($script['ports'][0])->setHttpsPort($script['ports'][1]) : null;
    $matcher = match ($script['matcher']) {
        'url' => new UrlStub($matcherContext, $queue),
        'request' => new RequestStub($queue),
        'both' => new BothStub($matcherContext, $queue),
    };
    $logger = $script['logger'] ? new Logger() : null;
    $stack = new RequestStack();
    $out = ['construct' => null, 'steps' => [], 'exception' => null];

    try {
        $listener = new RouterListener($matcher, $stack, $passed, $logger, '/work', $script['debug']);
    } catch (\Throwable $e) {
        $out['construct'] = throwable($e);
        $listener = null;
    }

    if ($listener) {
        $context = $passed ?? $matcherContext;
        $kernel = new Kernel();
        $requests = [];
        foreach ($script['requests'] as $i => $r) {
            $request = build_request($r);
            $requests[] = $request;
            $stack->push($request);
            $calls = \count($queue->calls);
            $logs = $logger ? \count($logger->calls) : 0;
            try {
                $listener->onKernelRequest(new RequestEvent($kernel, $request, 0 === $i ? HttpKernelInterface::MAIN_REQUEST : HttpKernelInterface::SUB_REQUEST));
                $thrown = null;
            } catch (\Throwable $e) {
                $thrown = throwable($e);
            }
            $out['steps'][] = [
                'thrown' => $thrown,
                'attributes' => typed_value($request->attributes->all()),
                'calls' => \array_slice($queue->calls, $calls),
                'logs' => $logger ? \array_slice($logger->calls, $logs) : [],
                'context' => context_state($context),
            ];
        }
        for ($i = \count($requests) - 1; $i >= 0; --$i) {
            try {
                if ('event' === $script['finish'][$i]) {
                    $listener->onKernelFinishRequest(new FinishRequestEvent($kernel, $requests[$i], 0 === $i ? HttpKernelInterface::MAIN_REQUEST : HttpKernelInterface::SUB_REQUEST));
                } else {
                    $listener->onKernelFinishRequest();
                }
                $thrown = null;
            } catch (\Throwable $e) {
                $thrown = throwable($e);
            }
            $stack->pop();
            $out['steps'][] = ['thrown' => $thrown, 'context' => context_state($context)];
        }

        $event = new ExceptionEvent($kernel, Request::create('/'), HttpKernelInterface::MAIN_REQUEST, build_exception($script['exception']));
        $listener->onKernelException($event);
        $response = $event->getResponse();
        if ($response) {
            $page = normalize_welcome($response->getContent());
            $welcome ??= $page;
            $out['exception'] = [$response->getStatusCode(), $page === $welcome];
        }
    }

    foreach ($script['requests'] as &$r) {
        $r['result'] = encode_result($r['result']);
    }
    unset($r);
    $script['out'] = $out;
    $scripts[] = $script;
}

echo json_encode(['welcome' => base64_encode($welcome), 'scripts' => $scripts], \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR);
