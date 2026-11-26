<?php
// StreamedJsonResponse answers for what its tests leave open, for
// crates/http-foundation/tests/data/streamed_json.json
// (scripts/record.sh http-foundation-streamed-json): 1,500 seeded scripts.
// Each builds data from a seeded spec: arrays (int and string keys,
// placeholder keys and values; three fixed scripts put the placeholder
// as the key of an array), scalars (strings from a pool with
// slashes, HTML characters, numbers, invalid UTF-8, the placeholder),
// generators (keys auto, int, string, bool, float, null or an array;
// some echo "<y>" before each yield, some throw after a given item),
// ArrayObject, an IteratorAggregate, JsonSerializable objects (also
// Traversable, or throwing) and stdClass objects; the data is sometimes
// not iterable. Encoding options from a pool. Then send() or the
// callback is called once or twice. Recorded: the constructor's
// exception, each call's result, every non-empty echo as its own chunk
// (an output buffer of chunk size 1), then the headers. The TypeError
// text loses its ", called in ... on line N" suffix. Texts are base64
// (lib/typed.php).

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
    }

    function header_remove(?string $name = null): void
    {
    }

    function headers_sent(): bool
    {
        return false;
    }
}

namespace {
    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';
    require __DIR__.'/lib/typed.php';

    use Symfony\Component\HttpFoundation\StreamedJsonResponse;

    mt_srand(20261128);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];

    $strings = ['', '0', '12', '1.5', '1e3', 'a/b', "<a href='x'>&\"", 'é', "\xff", '__symfony_json__', '"__symfony_json__"', "\u{2028}", 'Article'];
    $stringKeys = ['a', 'b', '_embedded', '__symfony_json__', 'with/slash', 'é'];
    $floats = ['1.5', '0', '-2.25', '10'];

    // A value spec of at most $depth more levels.
    $spec = static function (int $depth, bool $iterable = false) use (&$spec, $pick, $strings, $stringKeys, $floats) {
        $scalar = static fn () => match ($pick(['null', 'bool', 'int', 'float', 'str', 'str', 'str'])) {
            'null' => ['null'],
            'bool' => ['bool', 0 === mt_rand(0, 1)],
            'int' => ['int', $pick([0, 1, -3, 42])],
            'float' => ['float', $pick($floats)],
            'str' => ['str', base64_encode($pick($strings))],
        };
        $kinds = $iterable ? ['arr', 'arr', 'gen', 'gen', 'ao', 'agg', 'sertrav'] : ['scalar', 'scalar', 'scalar', 'arr', 'gen', 'gen', 'ao', 'agg', 'ser', 'sertrav', 'serthrow', 'obj'];
        $kind = $depth <= 0 ? 'scalar' : $pick($kinds);
        // A throwing jsonSerialize() ends the script: keep it rare.
        if ('serthrow' === $kind && 0 !== mt_rand(0, 4)) {
            $kind = 'ser';
        }
        $items = static fn () => array_map(static fn () => $spec($depth - 1), range(1, mt_rand(0, 3)));
        $arrayKey = static fn () => 0 === mt_rand(0, 1) ? ['i', $pick([0, 1, 2, 7])] : ['s', base64_encode($pick($stringKeys))];
        $pairs = static function () use ($items, $arrayKey) {
            $pairs = [];
            foreach ($items() as $i => $v) {
                // Mostly a list, sometimes keyed.
                $pairs[] = [0 === mt_rand(0, 2) ? $arrayKey() : ['i', $i], $v];
            }

            return $pairs;
        };

        return match ($kind) {
            'scalar' => $scalar(),
            'arr' => ['arr', $pairs()],
            'gen' => (static function () use ($items, $pick, $scalar, $stringKeys) {
                $yields = [];
                foreach ($items() as $v) {
                    $key = match ($pick(['auto', 'auto', 'auto', 'int', 'str', 'other'])) {
                        'auto' => null,
                        'int' => ['int', $pick([0, 1, 3])],
                        'str' => ['str', base64_encode($pick([...$stringKeys, '0', 'k']))],
                        'other' => $pick([['bool', true], ['bool', false], ['float', '1.5'], ['null'], ['arr', []]]),
                    };
                    $yields[] = [$key, $v];
                }
                $throwAt = 0 === mt_rand(0, 5) ? mt_rand(0, \count($yields)) : null;

                return ['gen', $yields, 0 === mt_rand(0, 1), $throwAt];
            })(),
            'ao' => ['ao', $pairs()],
            'agg' => ['agg', $items()],
            'ser' => ['ser', $spec($depth - 1)],
            'sertrav' => ['sertrav', $spec($depth - 1)],
            'serthrow' => ['serthrow'],
            'obj' => ['obj', array_map(static fn ($v) => [base64_encode($pick(['p', 'q', '__symfony_json__'])), $v], $items())],
        };
    };

    $build = static function (array $spec) use (&$build) {
        $array = static function (array $pairs) use ($build) {
            $a = [];
            foreach ($pairs as [$k, $v]) {
                $a['i' === $k[0] ? $k[1] : base64_decode($k[1])] = $build($v);
            }

            return $a;
        };

        return match ($spec[0]) {
            'null' => null,
            'bool', 'int' => $spec[1],
            'float' => (float) $spec[1],
            'str' => base64_decode($spec[1]),
            'arr' => $array($spec[1]),
            'gen' => (static function () use ($spec, $build) {
                foreach ($spec[1] as $i => [$k, $v]) {
                    if ($spec[3] === $i) {
                        throw new \RuntimeException("generator item $i");
                    }
                    if ($spec[2]) {
                        echo '<y>';
                    }
                    if (null === $k) {
                        yield $build($v);
                    } else {
                        yield $build($k) => $build($v);
                    }
                }
                if ($spec[3] === \count($spec[1])) {
                    throw new \RuntimeException('generator end');
                }
            })(),
            'ao' => new \ArrayObject($array($spec[1])),
            'agg' => new class(array_map($build, $spec[1])) implements \IteratorAggregate {
                public function __construct(private array $items)
                {
                }

                public function getIterator(): \Traversable
                {
                    return new \ArrayIterator($this->items);
                }
            },
            'ser' => new class($build($spec[1])) implements \JsonSerializable {
                public function __construct(private mixed $value)
                {
                }

                public function jsonSerialize(): mixed
                {
                    return $this->value;
                }
            },
            'sertrav' => new class($build($spec[1])) implements \JsonSerializable, \IteratorAggregate {
                public function __construct(private mixed $value)
                {
                }

                public function jsonSerialize(): mixed
                {
                    return $this->value;
                }

                public function getIterator(): \Traversable
                {
                    return new \ArrayIterator(['ignored']);
                }
            },
            'serthrow' => new class implements \JsonSerializable {
                public function jsonSerialize(): mixed
                {
                    throw new \DomainException('jsonSerialize');
                }
            },
            'obj' => (static function () use ($spec, $build) {
                $o = new \stdClass();
                foreach ($spec[1] as [$name, $v]) {
                    $o->{base64_decode($name)} = $build($v);
                }

                return $o;
            })(),
        };
    };

    $options = [15, 15, 0, \JSON_UNESCAPED_SLASHES | \JSON_NUMERIC_CHECK, \JSON_PRETTY_PRINT, \JSON_FORCE_OBJECT, \JSON_PARTIAL_OUTPUT_ON_ERROR, \JSON_INVALID_UTF8_SUBSTITUTE | \JSON_UNESCAPED_UNICODE, \JSON_PRESERVE_ZERO_FRACTION, \JSON_NUMERIC_CHECK];
    $headerSets = [[], [], [['X-Test', 'Test']], [['Content-Type', 'application/json+stream']], [['content-type', 'text/plain']], [['Content-Type', '0']], [['Content-Type', '']]];

    $thrown = static fn (\Throwable $e) => ['throws', $e::class, base64_encode(preg_replace('/, called in .* on line \d+$/', '', $e->getMessage()))];

    // The placeholder as the key of an array: not counted, so the parts no
    // longer line up with what is streamed in them.
    $p = ['s', base64_encode('__symfony_json__')];
    $gen = ['gen', [[null, ['str', base64_encode('g')]], [null, ['int', 1]]], false, null];
    $fixed = [
        ['arr', [[$p, ['arr', [[['i', 0], $gen]]]], [['s', base64_encode('b')], $gen]]],
        ['arr', [[$p, ['arr', [[['i', 0], ['str', base64_encode('x')]]]]], [['s', base64_encode('b')], $gen]]],
        ['arr', [[['s', base64_encode('a')], $gen], [$p, ['arr', [[$p, $gen]]]]]],
    ];

    $scripts = [];
    for ($n = 0; $n < 1500; ++$n) {
        $data = $fixed[$n] ?? (0 === mt_rand(0, 14) ? $spec(2) : $spec(mt_rand(1, 3), true));
        $headers = $pick($headerSets);
        $option = $pick($options);
        $ops = array_map(static fn () => $pick(['send', 'send', 'callback']), range(1, mt_rand(1, 2)));

        $array = [];
        foreach ($headers as [$k, $v]) {
            $array[$k] = $v;
        }
        $script = ['data' => $data, 'headers_in' => $headers, 'options' => $option, 'ops' => $ops];
        try {
            $response = new StreamedJsonResponse($build($data), 200, $array, $option);
        } catch (\Throwable $e) {
            $script['new'] = $thrown($e);
            $scripts[] = $script;
            continue;
        }
        $script['new'] = null;

        $results = [];
        foreach ($ops as $op) {
            $chunks = [];
            ob_start(static function (string $buffer) use (&$chunks) {
                if ('' !== $buffer) {
                    $chunks[] = base64_encode($buffer);
                }

                return '';
            }, 1);
            try {
                $result = 'send' === $op
                    ? ($response->send() === $response ? ['self'] : ['other'])
                    : typed_value(($response->getCallback())());
            } catch (\Throwable $e) {
                $result = $thrown($e);
            }
            ob_end_flush();
            $results[] = [$result, $chunks];
        }
        $script['results'] = $results;
        $script['headers'] = typed_value($response->headers->allPreserveCase());
        $scripts[] = $script;
    }

    echo json_encode(['scripts' => $scripts], \JSON_THROW_ON_ERROR), "\n";
}
