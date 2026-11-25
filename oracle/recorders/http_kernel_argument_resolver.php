<?php
// ArgumentResolver answers for crates/http-kernel/tests/data/argument_resolver.json
// (scripts/record.sh http-kernel-argument-resolver): 4,000 seeded scripts.
// Each evaluates a closure with a generated signature (0-4 parameters: no
// type, scalar types, nullable and union types, Request and look-alike
// classes, defaults, a variadic last, #[ValueResolver] attributes), sets
// generated request attributes, and asks an ArgumentResolver (the default
// resolvers, or a custom chain with string keys, with or without named
// resolvers) for the arguments. Recorded: the metadata
// ArgumentMetadataFactory builds (what reflection reports), and the
// arguments or the exception (class, message).

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\DependencyInjection\ServiceLocator;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpKernel\Controller\ArgumentResolver;
use Symfony\Component\HttpKernel\Controller\ArgumentResolver\DefaultValueResolver;
use Symfony\Component\HttpKernel\Controller\ArgumentResolver\RequestAttributeValueResolver;
use Symfony\Component\HttpKernel\Controller\ArgumentResolver\RequestValueResolver;
use Symfony\Component\HttpKernel\Controller\ArgumentResolver\SessionValueResolver;
use Symfony\Component\HttpKernel\Controller\ArgumentResolver\VariadicValueResolver;
use Symfony\Component\HttpKernel\Controller\ValueResolverInterface;
use Symfony\Component\HttpKernel\ControllerMetadata\ArgumentMetadata;
use Symfony\Component\HttpKernel\ControllerMetadata\ArgumentMetadataFactory;
use Symfony\Component\HttpKernel\Exception\NearMissValueResolverException;

mt_srand(20261126);

function pick(array $a): mixed
{
    return $a[mt_rand(0, \count($a) - 1)];
}

function chance(int $percent): bool
{
    return mt_rand(1, 100) <= $percent;
}

final class Yields2 implements ValueResolverInterface
{
    public function resolve(Request $request, ArgumentMetadata $argument): iterable
    {
        return 'two' === $argument->getName() ? ['a', 'b'] : [];
    }
}

final class NearMiss2 implements ValueResolverInterface
{
    public function resolve(Request $request, ArgumentMetadata $argument): iterable
    {
        if (\in_array($argument->getName(), ['miss', 'x'], true)) {
            throw new NearMissValueResolverException('Not for $'.$argument->getName().' either.');
        }

        return [];
    }
}

final class NearMiss implements ValueResolverInterface
{
    public function resolve(Request $request, ArgumentMetadata $argument): iterable
    {
        if ('miss' === $argument->getName() || chance(0)) {
            throw new NearMissValueResolverException('No luck for $'.$argument->getName().'.');
        }

        return [];
    }
}

const RESOLVERS = [
    'RequestAttribute' => RequestAttributeValueResolver::class,
    'Request' => RequestValueResolver::class,
    'Session' => SessionValueResolver::class,
    'Default' => DefaultValueResolver::class,
    'Variadic' => VariadicValueResolver::class,
    'Yields2' => Yields2::class,
    'NearMiss' => NearMiss::class,
    'NearMiss2' => NearMiss2::class,
];

function gen_type(): ?string
{
    return pick([
        null, null, 'int', 'int', 'float', 'string', 'string', 'bool', 'array', 'mixed', 'int|string', 'iterable',
        '\\'.Request::class, '\\Foo\\Request', '\\stdClass', '\\Symfony\\Component\\HttpFoundation\\Session\\SessionInterface',
    ]);
}

function gen_default(?string $type): ?string
{
    return pick(match ($type) {
        'int' => ['5', '0', '-1'],
        'float' => ['1.5', '0.0', '2'],
        'string' => ["'x'", "''", "'0'"],
        'bool' => ['true', 'false'],
        'array', 'iterable' => ['[]', "['a' => 1]"],
        'int|string' => ["'d'", '7'],
        null, 'mixed' => ["'d'", '7', '[]', 'false', '1.5'],
        default => ['null'],
    });
}

function gen_name(): string
{
    return pick(['foo', 'bar', 'id', 'price', 'active', 'request', 'two', 'miss', 'rest', 'x']);
}

function gen_signature(): array
{
    $parameters = [];
    $names = [];
    for ($i = mt_rand(0, 4); $i > 0; --$i) {
        $name = gen_name();
        if (isset($names[$name])) {
            continue;
        }
        $names[$name] = true;
        $type = gen_type();
        $nullable = null !== $type && !\in_array($type, ['mixed', 'int|string'], true) && chance(30);
        $variadic = 1 === $i && chance(15);
        $default = !$variadic && chance(35) ? gen_default($type) : null;
        if (null !== $default && 'null' === $default && null !== $type && !$nullable && !\in_array($type, ['mixed'], true)) {
            $nullable = true;
        }
        $attribute = null;
        if (chance(30)) {
            $attribute = [pick(array_keys(RESOLVERS) + [8 => 'Unknown']), chance(50)];
            if ($attribute[1] && chance(50)) {
                // disabled by the chain's string key
                $attribute[0] = 'key_'.$attribute[0];
            }
            if (chance(10)) {
                $attribute[] = pick(array_keys(RESOLVERS));
            }
        }
        $parameters[] = compact('name', 'type', 'nullable', 'variadic', 'default', 'attribute');
    }

    return $parameters;
}

function signature_code(array $parameters): string
{
    $code = [];
    foreach ($parameters as $p) {
        $s = '';
        if ($p['attribute']) {
            foreach (\array_slice($p['attribute'], 0, 1) as $resolver) {
                $class = str_starts_with($resolver, 'key_') ? $resolver : (RESOLVERS[$resolver] ?? 'unknown_resolver');
                $s .= '#[\\Symfony\\Component\\HttpKernel\\Attribute\\ValueResolver('.var_export($class, true).', disabled: '.($p['attribute'][1] ? 'true' : 'false').')] ';
            }
            if (isset($p['attribute'][2])) {
                $s .= '#[\\Symfony\\Component\\HttpKernel\\Attribute\\ValueResolver('.var_export(RESOLVERS[$p['attribute'][2]], true).')] ';
            }
        }
        if (null !== $p['type']) {
            $s .= ($p['nullable'] ? '?' : '').$p['type'].' ';
        }
        $s .= ($p['variadic'] ? '...' : '').'$'.$p['name'];
        if (null !== $p['default']) {
            $s .= ' = '.$p['default'];
        }
        $code[] = $s;
    }

    return 'return static function ('.implode(', ', $code).') {};';
}

function gen_attribute_value(): mixed
{
    return match (mt_rand(1, 14)) {
        1 => pick(['123', '06', '-06', '007', '00', '9223372036854775807', '9223372036854775808', '09223372036854775808', '-9223372036854775809']),
        2 => pick(['1.5', '1e3', '.5', '5.', '-0', ' 12', '12 ', ' 12 ', "12\n", '1_000', '0x1A', '1x', 'abc', '']),
        3 => pick(['true', 'false', 'on', 'off', 'yes', 'no', '1', '0', 'TRUE', ' true ', 'banana']),
        4 => pick([0, 1, -1, 42, PHP_INT_MAX, PHP_INT_MIN]),
        5 => pick([0.0, 1.5, -2.5, 1e20, -1e20, 9.2233720368547758E18, \INF, -\INF, \NAN, 3.0]),
        6 => pick([true, false]),
        7 => null,
        8 => pick([[], ['a', 'b'], ['k' => 'v', 3 => 4]]),
        9 => new \stdClass(),
        default => pick(['foo', 'bar', 'x y', 'café']),
    };
}

/// The value as a result: the request is ['request'].
function encode_argument(mixed $v, Request $request): array
{
    return $v === $request ? ['request'] : typed_value($v);
}

function build_resolvers(string $kind): array
{
    if ('default' === $kind) {
        return [[], []];
    }
    $chain = [];
    $spec = [];
    foreach (array_keys(RESOLVERS) as $name) {
        if (chance(55)) {
            $key = chance(50) ? 'key_'.$name : null;
            $spec[] = [$key, $name];
        }
    }
    shuffle($spec);
    foreach ($spec as [$key, $name]) {
        $class = RESOLVERS[$name];
        if (null === $key) {
            $chain[] = new $class();
        } else {
            $chain[$key] = new $class();
        }
    }

    return [$chain, $spec];
}

$scripts = [];
for ($n = 0; $n < 4000; ++$n) {
    $signature = gen_signature();
    $code = signature_code($signature);
    $controller = eval($code);

    $attributes = [];
    foreach ($signature as $p) {
        if (chance(60)) {
            $attributes[$p['name']] = gen_attribute_value();
        }
    }
    if (chance(15)) {
        $attributes[gen_name()] = gen_attribute_value();
    }

    $kind = pick(['default', 'default', 'custom']);
    [$chain, $spec] = build_resolvers($kind);
    $named = null;
    $namedSpec = null;
    if (chance(40)) {
        $namedSpec = [];
        foreach (array_keys(RESOLVERS) as $name) {
            if (chance(60)) {
                $namedSpec[] = $name;
            }
        }
        $services = [];
        foreach ($namedSpec as $name) {
            $class = RESOLVERS[$name];
            $services[$class] = static fn () => new $class();
        }
        $named = new ServiceLocator($services);
    }

    $request = Request::create('/');
    foreach ($attributes as $k => $v) {
        $request->attributes->set($k, $v);
    }

    $metadata = [];
    foreach ((new ArgumentMetadataFactory())->createArgumentMetadata($controller) as $m) {
        $metadata[] = [
            $m->getName(),
            $m->getType(),
            $m->isVariadic(),
            $m->hasDefaultValue(),
            $m->hasDefaultValue() ? typed_value($m->getDefaultValue()) : null,
            $m->isNullable(),
            array_map(static fn ($a) => [$a::class, $a->resolver, $a->disabled], $m->getAttributes()),
            $m->getControllerName(),
        ];
    }

    try {
        $resolver = new ArgumentResolver(new ArgumentMetadataFactory(), $chain, $named);
        $arguments = $resolver->getArguments($request, $controller);
        $outcome = ['returns', array_map(static fn ($a) => encode_argument($a, $request), $arguments)];
    } catch (\Throwable $e) {
        $outcome = ['throws', $e::class, base64_encode($e->getMessage())];
    }

    $scripts[] = [
        'code' => base64_encode($code),
        'metadata' => $metadata,
        'attributes' => typed_pairs($attributes),
        'chain' => $spec,
        'named' => $namedSpec,
        'outcome' => $outcome,
    ];
}

echo json_encode(['scripts' => $scripts], \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR);
