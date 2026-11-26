<?php
// The router of http_foundation_native_session.php's built-in server: runs
// the scenario POSTed as JSON in a real web request (PHP's session
// extension only sends its cookie and cache headers there) and answers a
// JSON transcript on the last line.
//
// A scenario names its ini settings, the save handler (a store-backed
// AbstractSessionHandler subclass that logs every call as the upstream
// fixtures' TestSessionHandler does, a bare \SessionHandlerInterface with
// or without the timestamp and id methods, NativeFileSessionHandler,
// NullSessionHandler, or none: PHP's files module), the handler calls
// that answer false, the mode (session_* calls, NativeSessionStorage or
// PhpBridgeSessionStorage, with an AttributeBag and a FlashBag) and the
// operations. Then, as the fixtures' shutdown function does,
// session_write_close() and headers_list(). Recorded: each operation's
// result or exception, every handler call and what it answered, warnings,
// the store and the session files afterwards. Values are typed
// (["string", text], ["int", n], ["array", [[key, value], ...]], ...)
// with plain text, so the recorder can normalize generated ids in it.

namespace Symfony\Component\HttpFoundation\Session\Storage {
    function time(): int
    {
        return $GLOBALS['recorder_now'];
    }
}

namespace {
    use Symfony\Component\HttpFoundation\Session\Attribute\AttributeBag;
    use Symfony\Component\HttpFoundation\Session\Flash\FlashBag;
    use Symfony\Component\HttpFoundation\Session\Storage\Handler\AbstractSessionHandler;
    use Symfony\Component\HttpFoundation\Session\Storage\Handler\MigratingSessionHandler;
    use Symfony\Component\HttpFoundation\Session\Storage\Handler\NativeFileSessionHandler;
    use Symfony\Component\HttpFoundation\Session\Storage\Handler\NullSessionHandler;
    use Symfony\Component\HttpFoundation\Session\Storage\Handler\StrictSessionHandler;
    use Symfony\Component\HttpFoundation\Session\Storage\NativeSessionStorage;
    use Symfony\Component\HttpFoundation\Session\Storage\PhpBridgeSessionStorage;

    error_reporting(E_ALL);
    header_remove('X-Powered-By');
    header('Content-Type: text/plain; charset=utf-8');
    require '/demo/vendor/autoload.php';

    /** A value as typed JSON, strings as they are. */
    function t(mixed $v): array
    {
        return match (true) {
            null === $v => ['null'],
            \is_bool($v) => ['bool', $v],
            \is_int($v) => ['int', $v],
            \is_float($v) => ['float', var_export($v, true)],
            \is_string($v) => ['string', $v],
            \is_array($v) => ['array', array_map(static fn ($k) => [t($k), t($v[$k])], array_keys($v))],
            default => ['object', get_debug_type($v)],
        };
    }

    $GLOBALS['recorder_now'] = 1700000000;
    $GLOBALS['recorder_log'] = [];
    $log = static function (...$entry): void {
        $GLOBALS['recorder_log'][] = $entry;
    };
    set_error_handler(static function (int $type, string $message) use ($log): bool {
        $log('warning', $type, $message);

        return true;
    });

    $s = json_decode(file_get_contents('php://input'), true, 512, \JSON_THROW_ON_ERROR);
    $GLOBALS['store'] = $s['store'];
    $GLOBALS['fail'] = $s['fail'];
    $dir = sys_get_temp_dir().'/native-session-'.$s['n'];
    if (is_dir($dir)) {
        array_map('unlink', glob("$dir/*"));
    } else {
        mkdir($dir);
    }
    foreach ($s['files'] as $id => $data) {
        file_put_contents("$dir/sess_$id", $data);
    }
    foreach ($s['ini'] as [$k, $v]) {
        ini_set('session.'.$k, $v);
    }
    ini_set('session.save_path', $dir);

    /** Logs a call and its answer; a method in the scenario's "fail" list answers false. */
    trait Logs
    {
        protected function call(string $method, array $args, callable $f): mixed
        {
            $GLOBALS['recorder_log'][] = ['call', $this->tag, $method, ...array_map('t', $args)];
            $r = $f();
            if (\in_array($method, $GLOBALS['fail'], true)) {
                $r = false;
            }
            $GLOBALS['recorder_log'][] = ['return', $this->tag, $method, t($r)];

            return $r;
        }
    }

    /** An AbstractSessionHandler subclass over the store. */
    final class RecorderHandler extends AbstractSessionHandler
    {
        use Logs;

        public function __construct(private string $tag)
        {
        }

        public function open(string $path, string $name): bool
        {
            return $this->call('open', [$name], fn () => parent::open($path, $name));
        }

        public function validateId(string $sessionId): bool
        {
            return $this->call('validateId', [$sessionId], fn () => parent::validateId($sessionId));
        }

        public function read(string $sessionId): string|false
        {
            return $this->call('read', [$sessionId], fn () => parent::read($sessionId));
        }

        public function updateTimestamp(string $sessionId, string $data): bool
        {
            return $this->call('updateTimestamp', [$sessionId, $data], fn () => parent::updateTimestamp($sessionId, $data));
        }

        public function write(string $sessionId, string $data): bool
        {
            return $this->call('write', [$sessionId, $data], fn () => parent::write($sessionId, $data));
        }

        public function destroy(string $sessionId): bool
        {
            return $this->call('destroy', [$sessionId], fn () => parent::destroy($sessionId));
        }

        public function close(): bool
        {
            return $this->call('close', [], fn () => true);
        }

        public function gc(int $maxLifetime): int|false
        {
            return $this->call('gc', [$maxLifetime], fn () => 0);
        }

        protected function doRead(string $sessionId): string
        {
            return $this->call('doRead', [$sessionId], fn () => $GLOBALS['store'][$this->tag][$sessionId] ?? '');
        }

        protected function doWrite(string $sessionId, string $data): bool
        {
            return $this->call('doWrite', [$sessionId, $data], function () use ($sessionId, $data) {
                $GLOBALS['store'][$this->tag][$sessionId] = $data;

                return true;
            });
        }

        protected function doDestroy(string $sessionId): bool
        {
            return $this->call('doDestroy', [$sessionId], function () use ($sessionId) {
                unset($GLOBALS['store'][$this->tag][$sessionId]);

                return true;
            });
        }
    }

    /** A plain \SessionHandlerInterface over the store; a missing id reads as $missing. */
    class BareHandler implements \SessionHandlerInterface
    {
        use Logs;

        public function __construct(protected string $tag, private string|false $missing)
        {
        }

        public function open(string $path, string $name): bool
        {
            return $this->call('open', [$name], fn () => true);
        }

        public function close(): bool
        {
            return $this->call('close', [], fn () => true);
        }

        public function read(string $id): string|false
        {
            return $this->call('read', [$id], fn () => $GLOBALS['store'][$this->tag][$id] ?? $this->missing);
        }

        public function write(string $id, string $data): bool
        {
            return $this->call('write', [$id, $data], function () use ($id, $data) {
                $GLOBALS['store'][$this->tag][$id] = $data;

                return true;
            });
        }

        public function destroy(string $id): bool
        {
            return $this->call('destroy', [$id], function () use ($id) {
                unset($GLOBALS['store'][$this->tag][$id]);

                return true;
            });
        }

        public function gc(int $max_lifetime): int|false
        {
            return $this->call('gc', [$max_lifetime], fn () => 0);
        }
    }

    final class BareTimestampHandler extends BareHandler implements \SessionUpdateTimestampHandlerInterface
    {
        public function validateId(string $id): bool
        {
            return $this->call('validateId', [$id], fn () => isset($GLOBALS['store'][$this->tag][$id]));
        }

        public function updateTimestamp(string $id, string $data): bool
        {
            return $this->call('updateTimestamp', [$id, $data], fn () => true);
        }
    }

    final class BareIdHandler extends BareHandler implements \SessionIdInterface
    {
        private int $next = 0;

        public function create_sid(): string
        {
            return $this->call('create_sid', [], fn () => sprintf('custom%026d', ++$this->next));
        }
    }

    $handler = match ($s['handler']) {
        'test' => new RecorderHandler('a'),
        'bare' => new BareHandler('a', ''),
        'bare_false' => new BareHandler('a', false),
        'bare_ts' => new BareTimestampHandler('a', ''),
        'bare_id' => new BareIdHandler('a', ''),
        'strict_bare' => new StrictSessionHandler(new BareHandler('a', false)),
        'migrating' => new MigratingSessionHandler(new RecorderHandler('a'), new RecorderHandler('b')),
        'files' => new NativeFileSessionHandler($dir),
        'null' => new NullSessionHandler(),
        'none' => null,
    };

    $storage = null;
    $try = static function (callable $f): array {
        try {
            return ['ok', t($f())];
        } catch (\Throwable $e) {
            return ['throws', $e::class, $e->getMessage()];
        }
    };
    $build = $try(static function () use ($s, $handler, &$storage) {
        if (null !== $handler && ('raw' === $s['mode'] || 'bridge' === $s['mode'])) {
            session_set_save_handler($handler, false);
        }
        if ('native' === $s['mode']) {
            $storage = new NativeSessionStorage($s['options'], $handler);
        } elseif ('bridge' === $s['mode']) {
            if ($s['bridge_started']) {
                session_start();
            }
            $storage = new PhpBridgeSessionStorage($handler);
        }
        if (null !== $storage) {
            $storage->registerBag(new AttributeBag());
            $storage->registerBag(new FlashBag());
        }

        return null;
    });
    $log('build', $build);

    $run = static function (array $op) use (&$storage) {
        $a = \array_slice($op, 1);

        return match ($op[0]) {
            'start' => session_start(),
            'write_close' => session_write_close(),
            'abort' => session_abort(),
            'reset' => session_reset(),
            'destroy' => session_destroy(),
            'regenerate' => session_regenerate_id($a[0]),
            'id' => session_id(),
            'set_id' => session_id($a[0]),
            'name' => session_name(),
            'set_name' => session_name($a[0]),
            'status' => session_status(),
            'create_id' => session_create_id($a[0]),
            'encode' => session_encode(),
            'decode' => session_decode($a[0]),
            'gc' => session_gc(),
            'set' => $_SESSION[$a[0]] = $a[1],
            'unset' => (function () use ($a) { unset($_SESSION[$a[0]]); })(),
            'assign' => $_SESSION = $a[0],
            'session' => $_SESSION ?? null,
            'echo' => print("output\n"),
            'setcookie' => setcookie($a[0], $a[1]),
            'ini' => ini_set('session.'.$a[0], $a[1]),
            'ini_get' => ini_get('session.'.$a[0]),
            'cookie_params' => session_get_cookie_params(),
            'tick' => $GLOBALS['recorder_now'] += $a[0],
            's_start' => $storage->start(),
            's_save' => $storage->save(),
            's_regenerate' => $storage->regenerate($a[0], $a[1]),
            's_clear' => $storage->clear(),
            's_set_id' => $storage->setId($a[0]),
            's_get_id' => $storage->getId(),
            's_set_name' => $storage->setName($a[0]),
            's_get_name' => $storage->getName(),
            's_is_started' => $storage->isStarted(),
            's_attr_set' => $storage->getBag('attributes')->set($a[0], $a[1]),
            's_attr_get' => $storage->getBag('attributes')->get($a[0]),
            's_attr_all' => $storage->getBag('attributes')->all(),
            's_flash_add' => $storage->getBag('flashes')->add($a[0], $a[1]),
            's_flash_get' => $storage->getBag('flashes')->get($a[0]),
            's_meta' => [$storage->getMetadataBag()->getCreated(), $storage->getMetadataBag()->getLastUsed(), $storage->getMetadataBag()->getLifetime()],
            's_set_options' => $storage->setOptions($a[0]),
            's_register_bag' => $storage->registerBag(new AttributeBag('_late', 'late')),
            's_handler_name' => $storage->getSaveHandler()->getSaveHandlerName(),
        };
    };

    if ('ok' === $build[0]) {
        foreach ($s['ops'] as $op) {
            $log('op', $op, $try(static fn () => $run($op)));
        }
    }

    // the fixtures' shutdown function
    $log('end', $try(static fn () => \PHP_SESSION_ACTIVE === session_status() ? session_write_close() : null));
    $files = [];
    foreach (glob("$dir/sess_*") as $file) {
        $files[] = [substr(basename($file), 5), file_get_contents($file)];
    }
    sort($files);
    echo "\n", json_encode([
        'log' => $GLOBALS['recorder_log'],
        'headers' => headers_list(),
        'status' => session_status(),
        'id' => session_id(),
        'session' => isset($_SESSION) || \array_key_exists('_SESSION', $GLOBALS) ? t($_SESSION ?? null) : ['unset'],
        'store' => t($GLOBALS['store']),
        'files' => $files,
    ], \JSON_INVALID_UTF8_SUBSTITUTE | \JSON_UNESCAPED_SLASHES | \JSON_PARTIAL_OUTPUT_ON_ERROR), "\n";
}
