<?php
// HttpKernel lifecycle answers for crates/http-kernel/tests/data/lifecycle.json
// (scripts/record.sh http-kernel-lifecycle): 1,500 seeded scripts, each a
// controller (a response, another value, a throwable, or none), an argument
// resolver (arguments or a throwable), handle()'s $catch and
// $handleAllThrowables, and up to six prioritized listeners on the kernel
// events that log what they see and act (set or change a response, stop
// propagation, throw, replace the throwable, allow a custom status, replace
// the controller or its arguments). Recorded: every listener call in order
// with what its event exposes, handle()'s response or throwable, the request
// stack afterwards, then terminate() or terminateWithException() with their
// listener calls and output. The clock is fixed; header() and
// headers_sent() are replaced in the HttpFoundation namespace.

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
        return $GLOBALS['recorder_headers_sent'];
    }
}

namespace Symfony\Component\HttpKernel {
    function headers_sent(): bool
    {
        return $GLOBALS['recorder_headers_sent'];
    }
}

namespace {
    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';
    require __DIR__.'/lib/typed.php';

    use Symfony\Component\EventDispatcher\EventDispatcher;
    use Symfony\Component\HttpFoundation\Exception\SuspiciousOperationException;
    use Symfony\Component\HttpFoundation\Request;
    use Symfony\Component\HttpFoundation\RequestStack;
    use Symfony\Component\HttpFoundation\Response;
    use Symfony\Component\HttpKernel\Controller\ArgumentResolverInterface;
    use Symfony\Component\HttpKernel\Controller\ControllerResolverInterface;
    use Symfony\Component\HttpKernel\Event\ControllerArgumentsMetadata;
    use Symfony\Component\HttpKernel\Event\ControllerMetadata;
    use Symfony\Component\HttpKernel\Exception\HttpException;
    use Symfony\Component\HttpKernel\Exception\MethodNotAllowedHttpException;
    use Symfony\Component\HttpKernel\HttpKernel;
    use Symfony\Component\HttpKernel\KernelEvents;

    $GLOBALS['recorder_calls'] = [];
    $GLOBALS['recorder_headers_sent'] = false;

    mt_srand(20261125);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];

    $exceptions = static function () use ($pick): array {
        return match (mt_rand(0, 8)) {
            0, 1 => ['runtime', $pick(['', 'foo', 'boom'])],
            2 => ['type_error', 'bad type'],
            3 => ['error', 'fatal'],
            4 => ['logic', 'logic'],
            5, 6 => ['http', $pick([400, 403, 404, 405, 418, 429, 503, 200, 302]), $pick(['', 'nope']), $pick([[], ['X-E' => 'e'], ['Retry-After' => 5], ['X-L' => ['a', 'b']]])],
            7 => ['method_not_allowed', $pick([['POST'], ['get', 'put']])],
            8 => ['suspicious', 'untrusted host'],
        };
    };
    $build = static function (array $spec): \Throwable {
        return match ($spec[0]) {
            'runtime' => new \RuntimeException($spec[1]),
            'type_error' => new \TypeError($spec[1]),
            'error' => new \Error($spec[1]),
            'logic' => new \LogicException($spec[1]),
            'http' => HttpException::fromStatusCode($spec[1], $spec[2], null, $spec[3]),
            'method_not_allowed' => new MethodNotAllowedHttpException($spec[1]),
            'suspicious' => new SuspiciousOperationException($spec[1]),
        };
    };
    $values = [null, false, true, 0, 3, 1.5, -0.0, '', 'foo', str_repeat('é', 300), ['a' => 1, 2 => 3], [], new \stdClass()];
    $events = [KernelEvents::REQUEST, KernelEvents::CONTROLLER, KernelEvents::CONTROLLER_ARGUMENTS, KernelEvents::VIEW, KernelEvents::RESPONSE, KernelEvents::FINISH_REQUEST, KernelEvents::EXCEPTION, KernelEvents::TERMINATE];
    $statuses = [200, 201, 204, 301, 302, 304, 403, 404, 500, 503];

    $describe = static function (\Throwable $e): array {
        return [$e::class, $e->getMessage(), $e->getPrevious() ? $e->getPrevious()::class : null];
    };
    $kind = static fn ($m) => match (true) {
        null === $m => null,
        $m instanceof ControllerArgumentsMetadata => 'arguments',
        $m instanceof ControllerMetadata => 'controller',
    };
    $state = static function (Response $r): array {
        $headers = $r->headers->allPreserveCase();
        unset($headers['Date']);
        ksort($headers);

        return [$r->getStatusCode(), base64_encode((new \ReflectionProperty($r, 'statusText'))->getValue($r)), base64_encode($r->getContent()), typed_value($headers)];
    };

    $scripts = [];
    for ($n = 0; $n < 1500; ++$n) {
        $controller = match (mt_rand(0, 9)) {
            0, 1, 2, 3 => ['response', $pick(['ok', '', 'body']), $pick($statuses)],
            4, 5 => ['value', $pick(range(0, \count($values) - 1))],
            6, 7, 8 => ['throw', $exceptions()],
            9 => ['false'],
        };
        $arguments = 0 === mt_rand(0, 9) ? ['throw', $exceptions()] : ['list', array_slice(['x', 'y', 'z'], 0, mt_rand(0, 3))];
        $listeners = [];
        for ($i = mt_rand(0, 6); $i > 0; --$i) {
            $event = $pick($events);
            $action = match ($event) {
                KernelEvents::REQUEST => $pick([['log'], ['log'], ['response', 'early', $pick($statuses)], ['stop'], ['throw', $exceptions()]]),
                KernelEvents::CONTROLLER => $pick([['log'], ['log'], ['controller', 'replaced'], ['stop'], ['throw', $exceptions()]]),
                KernelEvents::CONTROLLER_ARGUMENTS => $pick([['log'], ['arguments', array_slice(['p', 'q'], 0, mt_rand(0, 2))], ['controller', 'swapped'], ['throw', $exceptions()]]),
                KernelEvents::VIEW => $pick([['log'], ['response', 'view', $pick($statuses)], ['response', 'view', $pick($statuses)], ['stop']]),
                KernelEvents::RESPONSE => $pick([['log'], ['status', $pick($statuses)], ['header', 'X-R', 'r'], ['response', 'filtered', $pick($statuses)], ['throw', $exceptions()]]),
                KernelEvents::FINISH_REQUEST => $pick([['log'], ['log'], ['throw', $exceptions()]]),
                KernelEvents::EXCEPTION => $pick([['log'], ['response', 'handled', $pick($statuses)], ['response', 'handled', $pick($statuses)], ['allow'], ['replace', $exceptions()], ['throw', $exceptions()], ['stop']]),
                KernelEvents::TERMINATE => $pick([['log'], ['log'], ['throw', $exceptions()]]),
            };
            $listeners[] = [$event, $pick([0, 0, 10, -10]), $action];
        }
        $script = [
            'controller' => $controller,
            'arguments' => $arguments,
            'listeners' => $listeners,
            'catch' => 0 !== mt_rand(0, 3),
            'all' => 0 === mt_rand(0, 2),
            'headers_sent' => 0 === mt_rand(0, 3),
        ];

        // Run it.
        $log = [];
        $dispatcher = new EventDispatcher();
        $stack = new RequestStack();
        foreach ($listeners as $index => [$eventName, $priority, $action]) {
            $dispatcher->addListener($eventName, static function ($event) use ($index, $eventName, $action, &$log, $stack, $build, $kind, $describe, $values) {
                $seen = match ($eventName) {
                    KernelEvents::REQUEST => [\count(array_filter([$stack->getCurrentRequest(), $stack->getParentRequest()])), $event->hasResponse(), $event->isMainRequest()],
                    KernelEvents::CONTROLLER => [\count($event->getControllerReflector()->getParameters())],
                    KernelEvents::CONTROLLER_ARGUMENTS => [typed_value($event->getArguments()), typed_value($event->getNamedArguments())],
                    KernelEvents::VIEW => [typed_value($event->getControllerResult()), typed_value($event->controllerMetadata?->getArguments())],
                    KernelEvents::RESPONSE => [$event->getResponse()->getStatusCode(), null !== $event->controllerMetadata],
                    KernelEvents::FINISH_REQUEST => [$kind($event->controllerMetadata)],
                    KernelEvents::EXCEPTION => [$describe($event->getThrowable()), $event->isKernelTerminating(), $kind($event->controllerMetadata), $event->hasResponse()],
                    KernelEvents::TERMINATE => [$event->getResponse()->getStatusCode()],
                };
                $log[] = [$eventName, $index, $seen];
                switch ($action[0]) {
                    case 'response':
                        $event->setResponse(new Response($action[1], $action[2], ['X-From' => $eventName]));
                        break;
                    case 'stop':
                        $event->stopPropagation();
                        break;
                    case 'throw':
                        throw $build($action[1]);
                    case 'replace':
                        $event->setThrowable($build($action[1]));
                        break;
                    case 'allow':
                        $event->allowCustomResponseCode();
                        break;
                    case 'arguments':
                        $event->setArguments($action[1]);
                        break;
                    case 'controller':
                        $label = $action[1];
                        $event->setController(static fn (...$args) => new Response($label.\count($args)));
                        break;
                    case 'status':
                        $event->getResponse()->setStatusCode($action[1]);
                        break;
                    case 'header':
                        $event->getResponse()->headers->set($action[1], $action[2]);
                        break;
                }
            }, $priority);
        }

        $controllerCallable = match ($controller[0]) {
            'response' => static fn ($first = null, ...$rest) => new Response($controller[1], $controller[2]),
            'value' => static fn ($first = null, ...$rest) => $values[$controller[1]],
            'throw' => static fn ($first = null, ...$rest) => throw $build($controller[1]),
            'false' => false,
        };
        $resolver = new class($controllerCallable) implements ControllerResolverInterface {
            public function __construct(private $controller)
            {
            }

            public function getController(Request $request): callable|false
            {
                return $this->controller;
            }
        };
        $argumentResolver = new class($arguments, $build) implements ArgumentResolverInterface {
            public function __construct(private array $arguments, private \Closure $build)
            {
            }

            public function getArguments(Request $request, callable $controller, ?\ReflectionFunctionAbstract $reflector = null): array
            {
                if ('throw' === $this->arguments[0]) {
                    throw ($this->build)($this->arguments[1]);
                }

                return $this->arguments[1];
            }
        };
        $kernel = new HttpKernel($dispatcher, $resolver, $stack, $argumentResolver, $script['all']);
        $request = Request::create('/path');

        $handled = null;
        try {
            $response = $kernel->handle($request, HttpKernel::MAIN_REQUEST, $script['catch']);
            $handled = ['response', $state($response)];
        } catch (\Throwable $e) {
            $handled = ['throws', $describe($e)];
        }
        $handleLog = $log;
        $after = [null !== $stack->getCurrentRequest(), base64_encode($request->headers->get('X-Php-Ob-Level') ?? '')];

        // Then terminate() the response, or terminateWithException() the throwable.
        $log = [];
        $GLOBALS['recorder_calls'] = [];
        $GLOBALS['recorder_headers_sent'] = $script['headers_sent'];
        ob_start();
        try {
            if ('response' === $handled[0]) {
                $kernel->terminate($request, $response);
            } else {
                $kernel->terminateWithException($e, $request);
            }
            $terminated = ['ok'];
        } catch (\Throwable $t) {
            $terminated = ['throws', $describe($t)];
        }
        $echo = ob_get_clean();
        $calls = $GLOBALS['recorder_calls'];
        if ('' !== $echo) {
            $calls[] = ['echo', base64_encode($echo)];
        }

        // A throwable from terminate() given back to terminateWithException():
        // the kernel knows it as its own terminate exception.
        $terminateLog = $log;
        $again = null;
        if ('response' === $handled[0] && 'throws' === $terminated[0]) {
            $log = [];
            $GLOBALS['recorder_calls'] = [];
            ob_start();
            try {
                $kernel->terminateWithException($t, $request);
                $again = ['ok', $log];
            } catch (\Throwable $u) {
                $again = ['throws', $describe($u), $log];
            }
            ob_end_clean();
        }

        $scripts[] = [
            'again' => $again,
            'script' => $script,
            'log' => $handleLog,
            'handled' => $handled,
            'after' => $after,
            'terminate_log' => $terminateLog,
            'terminated' => $terminated,
            'calls' => $calls,
        ];
    }

    echo json_encode(['values' => array_map('typed_value', $values), 'scripts' => $scripts], \JSON_THROW_ON_ERROR), "\n";
}
