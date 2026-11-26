<?php
// Session handler answers for what their tests leave open, for
// crates/http-foundation/tests/data/session_handlers.json
// (scripts/record.sh http-foundation-session-handlers): 1,200 seeded
// scripts and the marshaller and constructor cases.
//
// A script builds a handler chain over scripted leaves: a RecorderBackend
// (an AbstractSessionHandler whose doRead/doWrite/doDestroy answer from
// queues) or a RecorderBareHandler (a plain \SessionHandlerInterface,
// given to StrictSessionHandler), optionally under a
// MarshallingSessionHandler (IdentityMarshaller or a scripted
// RecorderMarshaller) or a MigratingSessionHandler of two leaves. Each
// leaf's answer queues are part of the script; an empty queue answers ''
// (read), true or 1. Then 3 to 14 operations: open, close, validateId,
// read, write, destroy, updateTimestamp, gc. session.use_cookies is 0
// (the CLI shows no headers; the cookie is the PHP-server tests' part).
//
// Recorded: each operation's result or exception, and the calls each
// leaf and the marshaller saw. A TypeError loses its ", called in ... on
// line N" suffix. Texts are base64 (lib/typed.php).

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\Cache\Marshaller\MarshallerInterface;
use Symfony\Component\HttpFoundation\Session\Storage\Handler\AbstractSessionHandler;
use Symfony\Component\HttpFoundation\Session\Storage\Handler\IdentityMarshaller;
use Symfony\Component\HttpFoundation\Session\Storage\Handler\MarshallingSessionHandler;
use Symfony\Component\HttpFoundation\Session\Storage\Handler\MigratingSessionHandler;
use Symfony\Component\HttpFoundation\Session\Storage\Handler\StrictSessionHandler;

ini_set('session.use_cookies', '0');

/** Answers from queues; ['s', text] | ['false'] | ['throw', class, message] | ['bool', b] | ['int', n]. */
trait Scripted
{
    public array $calls = [];
    public array $answers;

    private function answer(string $queue, mixed $default): mixed
    {
        $a = array_shift($this->answers[$queue]);
        if (null === $a) {
            return $default;
        }

        return match ($a[0]) {
            's' => base64_decode($a[1]),
            'false' => false,
            'bool' => $a[1],
            'int' => $a[1],
            'throw' => throw new ($a[1])($a[2]),
        };
    }
}

final class RecorderBackend extends AbstractSessionHandler
{
    use Scripted;

    public function __construct(array $answers)
    {
        $this->answers = $answers;
    }

    protected function doRead(string $sessionId): string|false
    {
        $this->calls[] = 'doRead '.$sessionId;

        return $this->answer('read', '');
    }

    protected function doWrite(string $sessionId, string $data): bool
    {
        $this->calls[] = 'doWrite '.$sessionId.' '.$data;

        return $this->answer('write', true);
    }

    protected function doDestroy(string $sessionId): bool
    {
        $this->calls[] = 'doDestroy '.$sessionId;

        return $this->answer('destroy', true);
    }

    public function close(): bool
    {
        $this->calls[] = 'close';

        return $this->answer('close', true);
    }

    public function gc(int $maxlifetime): int|false
    {
        $this->calls[] = 'gc '.$maxlifetime;

        return $this->answer('gc', 1);
    }
}

final class RecorderBareHandler implements \SessionHandlerInterface
{
    use Scripted;

    public function __construct(array $answers)
    {
        $this->answers = $answers;
    }

    public function open(string $path, string $name): bool
    {
        $this->calls[] = 'open '.$path.' '.$name;

        return $this->answer('open', true);
    }

    public function close(): bool
    {
        $this->calls[] = 'close';

        return $this->answer('close', true);
    }

    public function read(string $id): string|false
    {
        $this->calls[] = 'read '.$id;

        return $this->answer('read', '');
    }

    public function write(string $id, string $data): bool
    {
        $this->calls[] = 'write '.$id.' '.$data;

        return $this->answer('write', true);
    }

    public function destroy(string $id): bool
    {
        $this->calls[] = 'destroy '.$id;

        return $this->answer('destroy', true);
    }

    public function gc(int $max_lifetime): int|false
    {
        $this->calls[] = 'gc '.$max_lifetime;

        return $this->answer('gc', 1);
    }
}

/** marshall: ['string', text] | ['int', n] | ['missing'] | ['failed-key'] | ['failed-list']; unmarshall: a typed answer. */
final class RecorderMarshaller implements MarshallerInterface
{
    public array $calls = [];

    public function __construct(public array $marshalls, public array $unmarshalls)
    {
    }

    public function marshall(array $values, ?array &$failed): array
    {
        $this->calls[] = 'marshall '.$values['data'];
        $a = array_shift($this->marshalls) ?? ['string', base64_encode('m:'.$values['data'])];

        return match ($a[0]) {
            'string' => ['data' => base64_decode($a[1])],
            'int' => ['data' => $a[1]],
            'missing' => [],
            'failed-key' => (function () use (&$failed) { $failed = ['data' => true]; return []; })(),
            'failed-list' => (function () use (&$failed) { $failed = ['data']; return []; })(),
        };
    }

    public function unmarshall(string $value): mixed
    {
        $this->calls[] = 'unmarshall '.$value;
        $a = array_shift($this->unmarshalls) ?? ['string', base64_encode('u:'.$value)];

        return match ($a[0]) {
            'string' => base64_decode($a[1]),
            'false' => false,
            'true' => true,
            'int' => $a[1],
            'float' => unpack('E', hex2bin($a[1]))[1],
            'null' => null,
            'array' => [],
            'throw' => throw new ($a[1])($a[2]),
        };
    }
}

mt_srand(20261126);
$pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];
$b64 = static fn (string $s) => base64_encode($s);
$strip = static fn (string $m) => preg_replace('/, called in .* on line \d+$/', '', $m);
$call = static function (callable $f) use ($strip): array {
    $r = typed_call($f);
    if ('throws' === $r[0]) {
        $r[2] = base64_encode($strip($r[2]));
    }

    return $r;
};

$ids = ['a', 'b', 'c'];
$datas = ['', 'x', '0', 'payload'];
$errors = [['RuntimeException', 'storage down'], ['DomainException', 'bad data'], ['LogicException', 'nope']];

$leafAnswers = static function () use ($pick, $b64, $errors): array {
    $q = static function (callable $one) {
        $out = [];
        for ($i = mt_rand(0, 4); $i > 0; --$i) {
            $out[] = $one();
        }

        return $out;
    };
    $bool = static fn () => ['bool', 1 !== mt_rand(0, 3)];

    return [
        'read' => $q(static fn () => match (mt_rand(0, 5)) {
            0 => ['false'],
            1 => ['throw', ...$pick($errors)],
            2, 3 => ['s', $b64('')],
            default => ['s', $b64($pick(['data', 'x', '0']))],
        }),
        'write' => $q($bool),
        'destroy' => $q($bool),
        'open' => $q($bool),
        'close' => $q($bool),
        'gc' => $q(static fn () => 0 === mt_rand(0, 2) ? ['false'] : ['int', mt_rand(0, 9)]),
    ];
};

$scripts = [];
for ($n = 0; $n < 1200; ++$n) {
    $leaf = static function () use ($pick, $leafAnswers): array {
        return ['kind' => $pick(['abstract', 'bare']), 'answers' => $leafAnswers()];
    };
    $make = static function (array $spec): array {
        $h = 'abstract' === $spec['kind'] ? new RecorderBackend($spec['answers']) : new RecorderBareHandler($spec['answers']);

        return [$h, 'abstract' === $spec['kind'] ? $h : new StrictSessionHandler($h)];
    };

    $top = $pick(['leaf', 'marshalling', 'marshalling', 'migrating']);
    $leaves = 'migrating' === $top ? [$leaf(), $leaf()] : [$leaf()];
    $marshaller = null;
    if ('marshalling' === $top) {
        if (0 === mt_rand(0, 2)) {
            $marshaller = ['kind' => 'identity'];
        } else {
            $ms = [];
            for ($i = mt_rand(0, 3); $i > 0; --$i) {
                $ms[] = $pick([['string', $b64('m')], ['string', $b64('')], ['int', 7], ['missing'], ['failed-key'], ['failed-list']]);
            }
            $us = [];
            for ($i = mt_rand(0, 3); $i > 0; --$i) {
                $us[] = $pick([['string', $b64('v')], ['false'], ['true'], ['int', 12], ['float', bin2hex(pack('E', 1.5))], ['null'], ['array'], ['throw', 'DomainException', 'bad'], ['throw', 'RuntimeException', 'down']]);
            }
            $marshaller = ['kind' => 'scripted', 'marshall' => $ms, 'unmarshall' => $us];
        }
    }

    $ops = [];
    for ($i = mt_rand(3, 14); $i > 0; --$i) {
        $ops[] = match ($pick(['open', 'close', 'validateId', 'validateId', 'read', 'read', 'write', 'write', 'destroy', 'updateTimestamp', 'gc'])) {
            'open' => ['open', $b64('p'), $b64($pick(['PHPSESSID', 'sid']))],
            'close' => ['close'],
            'validateId' => ['validateId', $b64($pick($ids))],
            'read' => ['read', $b64($pick($ids))],
            'write' => ['write', $b64($pick($ids)), $b64($pick($datas))],
            'destroy' => ['destroy', $b64($pick($ids))],
            'updateTimestamp' => ['updateTimestamp', $b64($pick($ids)), $b64($pick($datas))],
            'gc' => ['gc', $pick([0, 30, 1440])],
        };
    }

    $made = array_map($make, $leaves);
    $raw = array_map(static fn ($m) => $m[0], $made);
    $m = null;
    $handler = $made[0][1];
    $build = ['ok'];
    try {
        if ('marshalling' === $top) {
            $m = 'identity' === $marshaller['kind'] ? new IdentityMarshaller() : new RecorderMarshaller($marshaller['marshall'], $marshaller['unmarshall']);
            $handler = new MarshallingSessionHandler($handler, $m);
        } elseif ('migrating' === $top) {
            // bare leaves: MigratingSessionHandler wraps them itself
            $handler = new MigratingSessionHandler($leaves[0]['kind'] === 'bare' ? $raw[0] : $made[0][1], $leaves[1]['kind'] === 'bare' ? $raw[1] : $made[1][1]);
        }
    } catch (\Throwable $e) {
        $build = ['throws', $e::class, $b64($strip($e->getMessage()))];
    }

    $results = [];
    if ('ok' === $build[0]) {
        foreach ($ops as $op) {
            $a = array_map(static fn ($v) => \is_string($v) ? base64_decode($v) : $v, \array_slice($op, 1));
            $result = $call(static fn () => $handler->{$op[0]}(...$a));
            $results[] = [
                'result' => $result,
                'calls' => array_map(static function ($h) {
                    $c = array_map('base64_encode', $h->calls);
                    $h->calls = [];

                    return $c;
                }, $raw),
                'marshaller' => $m instanceof RecorderMarshaller ? (static function () use ($m) {
                    $c = array_map('base64_encode', $m->calls);
                    $m->calls = [];

                    return $c;
                })() : [],
            ];
        }
    }
    $scripts[] = ['top' => $top, 'leaves' => $leaves, 'marshaller' => $marshaller, 'build' => $build, 'ops' => $ops, 'results' => $results];
}

// IdentityMarshaller::marshall() on arrays of mixed values
$valuePool = [['string', 'a'], ['string', ''], ['int', 1], ['null'], ['array'], ['bool', true], ['float', '1.5']];
$identity = [];
for ($n = 0; $n < 60; ++$n) {
    $values = [];
    for ($i = mt_rand(0, 3); $i > 0; --$i) {
        $values[] = $pick($valuePool);
    }
    $php = array_map(static fn ($v) => match ($v[0]) {
        'string' => $v[1], 'int' => $v[1], 'null' => null, 'array' => [], 'bool' => $v[1], 'float' => (float) $v[1],
    }, $values);
    $failed = null;
    $identity[] = ['values' => $values, 'result' => $call(static function () use ($php, &$failed) {
        return (new IdentityMarshaller())->marshall($php, $failed);
    })];
}

// constructors
$constructors = [
    'marshalling_bare' => $call(static fn () => new MarshallingSessionHandler(new RecorderBareHandler([]), new IdentityMarshaller())),
    'strict_abstract' => $call(static fn () => new StrictSessionHandler(new RecorderBackend([]))),
    'strict_strict' => $call(static fn () => new StrictSessionHandler(new StrictSessionHandler(new RecorderBareHandler([])))),
];
foreach ($constructors as $k => $r) {
    if ('object' === $r[0]) {
        $constructors[$k] = ['ok'];
    }
}

echo json_encode(['scripts' => $scripts, 'identity' => $identity, 'constructors' => $constructors], \JSON_UNESCAPED_SLASHES), "\n";
