<?php
// ControllerResolver answers for crates/http-kernel/tests/data/controller_resolver.json
// (scripts/record.sh http-kernel-controller-resolver): 4,000 seeded scripts.
// Each declares a generated PHP program in its own namespace (classes,
// abstract classes, interfaces and traits with parents, interfaces,
// constructors of every visibility, arity and outcome, methods of every
// visibility, static or abstract, __invoke, __call and __callStatic,
// attributes; functions; an anonymous class), written to a file under
// /tmp and required, so anonymous names are PHP's own. It then sets a
// generated _controller (strings, class and function names in any case,
// arrays of any shape, objects, closures of each kind, falsy and scalar
// values), maybe _check_controller_is_allowed, maybe allowControllers()
// calls, and asks a ControllerResolver with a logger for the controller.
// Recorded: the declared program, the values given, the logs, and the
// callable returned (by identity with the given values), what calling it
// returns and its name as ArgumentMetadataFactory reports it, or the
// exception (class, message, code, previous).

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Psr\Log\AbstractLogger;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpKernel\Controller\ControllerResolver;

mt_srand(20261127);

const AS_CONTROLLER = 'Symfony\\Component\\HttpKernel\\Attribute\\AsController';

function pick(array $a): mixed
{
    return $a[mt_rand(0, \count($a) - 1)];
}

function chance(int $percent): bool
{
    return mt_rand(1, 100) <= $percent;
}

final class Logger extends AbstractLogger
{
    public array $calls = [];

    public function log($level, \Stringable|string $message, array $context = []): void
    {
        $this->calls[] = [$level, base64_encode((string) $message), typed_pairs($context)];
    }
}

const METHODS = ['index', 'show', 'indexAction', 'showAction', 'list', 'run', 'staticAction', 'Index', 'publicAction', '__invoke', '__call', '__callStatic', '__invoke', '__call'];
const INTERFACE_METHODS = ['make', 'handle'];
const THROWS = ['LogicException', 'InvalidArgumentException', 'RuntimeException', 'Error', 'BadMethodCallException', 'TypeError'];

function gen_attributes(string $ns, int $percent): array
{
    $attributes = [];
    if (chance($percent)) {
        foreach ([AS_CONTROLLER, $ns.'\\Dummy', $ns.'\\Other', 'Symfony\\Component\\HttpKernel\\Attribute\\ascontroller'] as $a) {
            if (chance(35)) {
                $attributes[] = $a;
            }
        }
    }

    return $attributes;
}

/** The non-private methods a class inherits (lower name => method). */
function inherited(array $classes, ?int $parent): array
{
    $methods = [];
    while (null !== $parent) {
        foreach ($classes[$parent]['methods'] as $m) {
            $l = strtolower($m['name']);
            if (!isset($methods[$l]) && 'private' !== $m['visibility'] && '__construct' !== $l) {
                $methods[$l] = $m;
            }
        }
        $parent = $classes[$parent]['parent'];
    }

    return $methods;
}

/** The interface methods a class must have (lower name => method). */
function interface_methods(array $classes, array $interfaces): array
{
    $methods = [];
    foreach ($interfaces as $i) {
        foreach ($classes[$i]['methods'] as $m) {
            $methods[strtolower($m['name'])] = $m;
        }
        $methods += interface_methods($classes, $classes[$i]['interfaces']);
    }

    return $methods;
}

function all_interfaces(array $classes, array $c): array
{
    $all = $c['interfaces'];
    if (null !== $c['parent']) {
        $all = array_merge($all, all_interfaces($classes, $classes[$c['parent']]));
    }

    return $all;
}

function gen_method(string $ns, string $name, string $kind): array
{
    $magic = str_starts_with($name, '__');
    $visibility = $magic ? 'public' : pick(['public', 'public', 'public', 'protected', 'private']);
    $static = '__callStatic' === $name || (!$magic && chance(25));
    $abstract = 'abstract' === $kind && !$magic && 'private' !== $visibility && chance(35);

    return ['name' => $name, 'visibility' => $visibility, 'static' => $static, 'abstract' => $abstract, 'attributes' => gen_attributes($ns, 20)];
}

function gen_class(string $ns, string $name, string $kind, array $classes, bool $anonymous = false): array
{
    $c = ['name' => $ns.'\\'.$name, 'kind' => $kind, 'parent' => null, 'interfaces' => [], 'attributes' => gen_attributes($ns, 40), 'methods' => [], 'anonymous' => $anonymous];
    if ('trait' === $kind) {
        return $c;
    }
    $interfaces = array_keys(array_filter($classes, static fn ($x) => 'interface' === $x['kind']));
    $parents = array_keys(array_filter($classes, static fn ($x) => \in_array($x['kind'], ['class', 'abstract'], true) && !$x['anonymous']));
    if ('interface' === $kind) {
        foreach ($interfaces as $i) {
            if (chance(30)) {
                $c['interfaces'][] = $i;
            }
        }
        if (chance(40)) {
            $m = pick(INTERFACE_METHODS);
            $c['methods'][] = ['name' => $m, 'visibility' => 'public', 'static' => 'make' === $m, 'abstract' => true, 'attributes' => []];
        }

        return $c;
    }
    if ($anonymous) {
        $parents = array_values(array_filter($parents, static fn ($p) => null === ($k = constructor_of($classes, $classes[$p])) || ('public' === $k['visibility'] && null === $k['throws'] && !\in_array('required', $k['params'], true))));
    }
    if ($parents && chance($anonymous ? 60 : 45)) {
        $c['parent'] = pick($parents);
    }
    foreach ($interfaces as $i) {
        if (chance(30) && \count($c['interfaces']) < 2) {
            $c['interfaces'][] = $i;
        }
    }
    if (!$anonymous && chance(40)) {
        $params = [];
        for ($k = mt_rand(0, 2) - (chance(50) ? 1 : 0); $k > 0; --$k) {
            $params[] = 'required';
        }
        if (chance(25)) {
            $params[] = 'optional';
        }
        if (chance(10)) {
            $params[] = 'variadic';
        }
        $c['constructor'] = [
            'visibility' => pick(['public', 'public', 'public', 'protected', 'private']),
            'params' => $params,
            'throws' => chance(20) ? pick(THROWS) : null,
        ];
    }
    $inherited = inherited($classes, $c['parent']);
    $seen = [];
    for ($k = mt_rand(0, 4); $k > 0; --$k) {
        $name = pick(METHODS);
        if (isset($seen[strtolower($name)])) {
            continue;
        }
        $seen[strtolower($name)] = true;
        $m = gen_method(explode('\\', $c['name'])[0], $name, $kind);
        if (isset($inherited[strtolower($name)])) {
            $p = $inherited[strtolower($name)];
            $m['static'] = $p['static'];
            $m['abstract'] = $m['abstract'] && $p['abstract'];
            if ('public' === $p['visibility']) {
                $m['visibility'] = 'public';
            } elseif ('private' === $m['visibility']) {
                $m['visibility'] = 'protected';
            }
        }
        $c['methods'][] = $m;
    }
    if ('class' === $kind) {
        // implement what is abstract: inherited abstract methods, interface methods
        $own = [];
        foreach ($c['methods'] as $m) {
            $own[strtolower($m['name'])] = true;
        }
        $concrete = array_filter($inherited, static fn ($m) => !$m['abstract']);
        $required = array_filter($inherited, static fn ($m) => $m['abstract']) + interface_methods($classes, all_interfaces($classes, $c));
        foreach ($required as $l => $m) {
            if (!isset($own[$l]) && !isset($concrete[$l])) {
                $c['methods'][] = ['name' => $m['name'], 'visibility' => 'public', 'static' => $m['static'], 'abstract' => false, 'attributes' => []];
                $own[$l] = true;
            }
        }
    } else {
        // an abstract class implements its interfaces' methods, so instances never miss one
        $own = [];
        foreach ($c['methods'] as $m) {
            $own[strtolower($m['name'])] = true;
        }
        foreach (interface_methods($classes, $c['interfaces']) as $l => $m) {
            if (!isset($own[$l]) && !isset($inherited[$l])) {
                $c['methods'][] = ['name' => $m['name'], 'visibility' => 'public', 'static' => $m['static'], 'abstract' => false, 'attributes' => []];
            }
        }
    }
    // a method implementing an interface's is public, with its staticness
    $needed = interface_methods($classes, all_interfaces($classes, $c));
    foreach ($c['methods'] as &$m) {
        if (isset($needed[strtolower($m['name'])])) {
            $m['visibility'] = 'public';
            $m['static'] = $needed[strtolower($m['name'])]['static'];
            $m['abstract'] = false;
        }
    }
    unset($m);

    return $c;
}

function attributes_code(array $attributes): string
{
    return implode('', array_map(static fn ($a) => '#[\\'.$a.'] ', $attributes));
}

function short(string $name): string
{
    return substr($name, strrpos($name, '\\') + 1);
}

function class_code(array $classes, array $c): string
{
    $code = attributes_code($c['attributes']);
    if ($c['anonymous']) {
        $code = 'new '.$code.'class ';
    } else {
        $code .= match ($c['kind']) {
            'abstract' => 'abstract class ',
            'interface' => 'interface ',
            'trait' => 'trait ',
            default => 'class ',
        }.short($c['name']).' ';
    }
    if (null !== $c['parent']) {
        $code .= 'extends \\'.$classes[$c['parent']]['name'].' ';
    }
    if ($c['interfaces']) {
        $code .= ('interface' === $c['kind'] ? 'extends ' : 'implements ').implode(', ', array_map(static fn ($i) => '\\'.$classes[$i]['name'], $c['interfaces'])).' ';
    }
    $code .= "{\n";
    if (isset($c['constructor'])) {
        $k = $c['constructor'];
        $params = [];
        foreach ($k['params'] as $n => $p) {
            $params[] = match ($p) {
                'required' => '$r'.$n,
                'optional' => '$o'.$n.' = 1',
                'variadic' => '...$v',
            };
        }
        $body = null === $k['throws'] ? '' : 'throw new \\'.$k['throws'].'(\'Constructor of \'.__CLASS__.\' failed.\', 7);';
        $code .= '    '.$k['visibility'].' function __construct('.implode(', ', $params).') { '.$body." }\n";
    }
    foreach ($c['methods'] as $m) {
        $code .= '    '.attributes_code($m['attributes']).($m['abstract'] && 'interface' !== $c['kind'] ? 'abstract ' : '').$m['visibility'].' '.($m['static'] ? 'static ' : '').'function '.$m['name'];
        $params = \in_array($m['name'], ['__call', '__callStatic'], true) ? '$n, $a' : '';
        if ($m['abstract']) {
            $code .= '('.$params.");\n";
        } else {
            $body = \in_array($m['name'], ['__call', '__callStatic'], true) ? "return __METHOD__.'|'.\$n;" : 'return __METHOD__;';
            $code .= '('.$params.') { '.$body." }\n";
        }
    }

    return $code.'}';
}

function gen_world(int $n): array
{
    $ns = 'S'.$n;
    $classes = [];
    $names = ['Foo', 'Bar', 'Base', 'HomeController', 'Iface', 'Helper'];
    shuffle($names);
    for ($k = mt_rand(1, 4); $k > 0; --$k) {
        $kind = pick(['class', 'class', 'class', 'class', 'class', 'class', 'abstract', 'abstract', 'interface', 'interface', 'trait']);
        $classes[] = gen_class($ns, array_pop($names), $kind, $classes);
    }
    if (chance(20)) {
        $classes[] = gen_class($ns, 'anonymous', 'class', $classes, true);
    }
    $functions = [];
    $seen = [];
    for ($k = mt_rand(0, 2); $k > 0; --$k) {
        $name = pick(['handler', 'index_action', 'Handler', 'publicAction']);
        if (!isset($seen[strtolower($name)])) {
            $seen[strtolower($name)] = true;
            $functions[] = ['name' => $ns.'\\'.$name, 'attributes' => gen_attributes($ns, 40)];
        }
    }
    $closures = [];
    for ($k = mt_rand(0, 2); $k > 0; --$k) {
        $closures[] = ['attributes' => gen_attributes($ns, 40)];
    }

    return compact('ns', 'classes', 'functions', 'closures');
}

function program_code(array $world): string
{
    $code = "<?php\nnamespace {$world['ns']};\n\n";
    foreach ($world['classes'] as $c) {
        if (!$c['anonymous']) {
            $code .= class_code($world['classes'], $c)."\n\n";
        }
    }
    foreach ($world['functions'] as $f) {
        $code .= attributes_code($f['attributes']).'function '.short($f['name'])."() { return __FUNCTION__; }\n\n";
    }
    $code .= "return [\n    'anonymous' => [\n";
    foreach ($world['classes'] as $c) {
        if ($c['anonymous']) {
            $code .= '        '.class_code($world['classes'], $c).",\n";
        }
    }
    $code .= "    ],\n    'closures' => [\n";
    foreach ($world['closures'] as $c) {
        $code .= '        '.attributes_code($c['attributes'])."static function () { return 'closure'; },\n";
    }

    return $code."    ],\n];\n";
}

/** Whether `new $class()` works without arguments and does not throw. */
function constructor_of(array $classes, array $c): ?array
{
    while (true) {
        if (isset($c['constructor'])) {
            return $c['constructor'];
        }
        if (null === $c['parent']) {
            return null;
        }
        $c = $classes[$c['parent']];
    }
}

function instantiable(array $classes, array $c): bool
{
    if ('class' !== $c['kind']) {
        return false;
    }
    $k = constructor_of($classes, $c);

    return null === $k || ('public' === $k['visibility'] && null === $k['throws'] && !\in_array('required', $k['params'], true));
}

function class_name_variant(string $name): string
{
    return match (mt_rand(1, 16)) {
        1 => strtolower($name),
        2 => strtoupper($name),
        3 => '\\'.$name,
        4 => '\\\\'.$name,
        5 => $name.'\\',
        default => $name,
    };
}

function typo(string $s): string
{
    if ('' === $s) {
        return 'x';
    }

    return match (mt_rand(1, 5)) {
        1 => substr($s, 0, -1),
        2 => $s.'s',
        3 => substr($s, 1),
        4 => strrev($s),
        default => str_replace('Action', 'Actoin', $s).'x',
    };
}

function gen_method_name(array $world, ?array $c): string
{
    $names = METHODS;
    if (null !== $c) {
        foreach ($c['methods'] as $m) {
            $names[] = $m['name'];
            $names[] = $m['name'];
        }
        $parent = $c['parent'];
        while (null !== $parent) {
            foreach ($world['classes'][$parent]['methods'] as $m) {
                $names[] = $m['name'];
            }
            $parent = $world['classes'][$parent]['parent'];
        }
    }
    $name = pick($names);

    return match (mt_rand(1, 12)) {
        1 => strtolower($name),
        2 => strtoupper($name),
        3, 4 => typo($name),
        5 => pick(['', 'make', 'handle', 'nope', '__construct', 'bindTo']),
        default => $name,
    };
}

/**
 * A value for _controller: [spec, value]. Specs: ['value', typed],
 * ['object', i], ['closure', i], ['array', [[typed key, spec], ...]],
 * ['unset'].
 */
function gen_controller(array $world, array $objects, array $closures): array
{
    $named = array_values(array_filter($world['classes'], static fn ($c) => !$c['anonymous']));
    $class = static function () use ($named): ?array {
        return $named && chance(90) ? pick($named) : null;
    };
    $class_string = static fn (?array $c) => null === $c ? pick(['Nope', '', 'S0\\Foo']) : class_name_variant($c['name']);
    $string = static fn (string $s) => [['value', typed_value($s)], $s];
    $object = static function () use ($objects): ?array {
        if (!$objects) {
            return null;
        }
        $i = mt_rand(0, \count($objects) - 1);

        return [['object', $i], $objects[$i]];
    };
    $closure = static function () use ($closures): ?array {
        if (!$closures) {
            return null;
        }
        $i = mt_rand(0, \count($closures) - 1);

        return [['closure', $i], $closures[$i]];
    };
    $array = static function (array $pairs): array {
        $spec = [];
        $value = [];
        foreach ($pairs as [$k, [$s, $v]]) {
            $spec[] = [typed_value($k), $s];
            $value[$k] = $v;
        }

        return [['array', $spec], $value];
    };
    $scalar = static fn ($v) => [['value', typed_value($v)], $v];

    switch (mt_rand(1, 20)) {
        case 1: case 2: case 3:
            $c = $class();

            return $string($class_string($c).'::'.gen_method_name($world, $c));
        case 4: case 5:
            return $string($class_string($class()));
        case 6: case 7:
            $f = $world['functions'] && chance(80) ? pick($world['functions'])['name'] : pick(['strlen_nope', 'S0\\handler', 'handler']);

            return $string(class_name_variant($f));
        case 8: case 9: case 10:
            $c = $class();

            return $array([[0, $scalar($class_string($c))], [1, $scalar(gen_method_name($world, $c))]]);
        case 11: case 12: case 13:
            $o = $object() ?? $closure();
            if (null === $o) {
                return $scalar('');
            }
            $c = null;
            foreach ($world['classes'] as $x) {
                if (\is_object($o[1]) && strtolower(get_class($o[1])) === strtolower($x['name'])) {
                    $c = $x;
                }
            }

            return $array([[0, $o], [1, $scalar($o[1] instanceof \Closure && chance(50) ? '__invoke' : gen_method_name($world, $c))]]);
        case 14: case 15:
            return $object() ?? $closure() ?? $scalar('');
        case 16: case 17:
            return $closure() ?? $object() ?? $scalar('');
        case 18:
            $o = $object() ?? $scalar(pick(['', 'x']));
            $m = $scalar(gen_method_name($world, null));

            return $array(pick([
                [['a', $o], ['b', $m]],
                [[0, $o]],
                [[0, $o], [1, $m], [2, $scalar('x')]],
                [[1, $m], [0, $o]],
                [[0, $o], [1, $scalar(null)]],
                [[0, $o], [1, $scalar(5)]],
                [[0, $o], [1, $scalar([])]],
                [[0, $scalar(123)], [1, $m]],
                [[0, $scalar(null)], [1, $m]],
                [[0, $scalar([])], [1, $m]],
                [[0, $scalar(1.5)], [1, $m]],
                [[0, $scalar(true)], [1, $m]],
                [[0, $scalar($class_string($class()))], [1, $m], [2, $scalar('x')]],
                [['0', $scalar($class_string($class()))], ['1', $m]],
            ]));
        case 19:
            return pick([[['unset'], null], $scalar(null), $scalar(''), $scalar('0'), $scalar(0), $scalar(false), $scalar([]), $scalar(0.0)]);
        default:
            return $scalar(pick([5, 1.5, true, -3, 'x y', "\xff", '0.0', 'S0\\Foo::bar::baz']));
    }
}

/** A returned value: given objects and closures by index, strings as bytes. */
function encode(mixed $v, array $objects, array $closures): array
{
    if (\is_array($v)) {
        $pairs = [];
        foreach ($v as $k => $x) {
            $pairs[] = [typed_value($k), encode($x, $objects, $closures)];
        }

        return ['array', $pairs];
    }
    if (\is_object($v)) {
        foreach ($closures as $i => $c) {
            if ($v === $c) {
                return ['closure', $i];
            }
        }
        foreach ($objects as $i => $o) {
            if ($v === $o) {
                return ['object', $i];
            }
        }

        return ['new', base64_encode(get_class($v))];
    }

    return typed_value($v);
}

function pretty_name(callable $c): string
{
    $r = new \ReflectionFunction($c(...));
    if ($r->isAnonymous() || !$class = $r->getClosureCalledClass()) {
        return $r->name;
    }

    return $class->name.'::'.$r->name;
}

function throwable(\Throwable $e): array
{
    $previous = $e->getPrevious();

    return [$e::class, base64_encode($e->getMessage()), $e->getCode(), null === $previous ? null : [$previous::class, base64_encode($previous->getMessage())]];
}

/** What calling $c returns. Calling __call or __callStatic itself without
 * arguments fails in this recorder: where is left out of the message. */
function call(callable $c): array
{
    try {
        return ['returns', typed_value($c())];
    } catch (\ArgumentCountError $e) {
        return ['throws', [$e::class, base64_encode(preg_replace('/ passed in .* on line \d+/', ' passed', $e->getMessage())), 0, null]];
    } catch (\Throwable $e) {
        return ['throws', throwable($e)];
    }
}

function closure_info(\Closure $c): array
{
    $r = new \ReflectionFunction($c);

    return [
        'name' => base64_encode($r->name),
        'class' => null === $r->getClosureCalledClass() ? null : base64_encode($r->getClosureCalledClass()->name),
        'anonymous' => $r->isAnonymous(),
        'attributes' => array_map(static fn ($a) => $a->getName(), $r->getAttributes()),
        'pretty' => base64_encode(pretty_name($c)),
        'call' => call($c),
    ];
}

@mkdir('/tmp/cr');
$scripts = [];
for ($n = 0; $n < 4000; ++$n) {
    $world = gen_world($n + 1);
    $file = '/tmp/cr/s'.($n + 1).'.php';
    file_put_contents($file, program_code($world));
    $made = require $file;

    // the anonymous class's name, as PHP gives it
    $a = 0;
    foreach ($world['classes'] as &$c) {
        if ($c['anonymous']) {
            $c['name'] = base64_encode(get_class($made['anonymous'][$a++]));
        } else {
            $c['name'] = base64_encode($c['name']);
        }
    }
    unset($c);
    $decoded = array_map(static fn ($c) => ['name' => base64_decode($c['name'])] + $c, $world['classes']);

    // objects: one of each instantiable class, the anonymous object, maybe a stdClass
    $objects = [];
    $objectSpecs = [];
    foreach ($decoded as $i => $c) {
        if ($c['anonymous']) {
            $objects[] = $made['anonymous'][0];
            $objectSpecs[] = ['class', $i];
        } elseif (instantiable($decoded, $c) && chance(80)) {
            $objects[] = new $c['name']();
            $objectSpecs[] = ['class', $i];
        }
    }
    if (chance(15)) {
        $objects[] = new \stdClass();
        $objectSpecs[] = ['stdclass'];
    }

    // closures: anonymous ones, from functions, from callable methods
    $closures = $made['closures'];
    $closureSpecs = array_map(static fn ($c) => ['anonymous'], $made['closures']);
    foreach ($world['functions'] as $i => $f) {
        if (chance(50)) {
            $closures[] = \Closure::fromCallable($f['name']);
            $closureSpecs[] = ['function', $i];
        }
    }
    for ($k = mt_rand(0, 2); $k > 0 && $objects; --$k) {
        $o = mt_rand(0, \count($objects) - 1);
        $m = gen_method_name($world, null);
        foreach ($decoded as $c) {
            if (strtolower($c['name']) === strtolower(get_class($objects[$o])) && $c['methods'] && chance(70)) {
                $m = pick($c['methods'])['name'];
            }
        }
        if (is_callable([$objects[$o], $m])) {
            $closures[] = \Closure::fromCallable([$objects[$o], $m]);
            $closureSpecs[] = ['method', $o, $m];
        }
    }
    foreach ($decoded as $i => $c) {
        foreach ($c['methods'] as $m) {
            if ($m['static'] && chance(15) && is_callable([$c['name'], $m['name']])) {
                $closures[] = \Closure::fromCallable([$c['name'], $m['name']]);
                $closureSpecs[] = ['static', $i, $m['name']];
            }
        }
    }
    foreach ($closures as $i => $c) {
        $closureSpecs[$i] = ['spec' => $closureSpecs[$i]] + closure_info($c);
    }

    [$spec, $controller] = gen_controller(['classes' => $decoded] + $world, $objects, $closures);

    $request = Request::create(pick(['/', '/', '/foo', '/a/b', '/caf%C3%A9']));
    if ('unset' !== $spec[0]) {
        $request->attributes->set('_controller', $controller);
    }
    $check = null;
    if (chance(55)) {
        $check = pick([true, true, true, false, 1, 'yes', '0', null]);
        $request->attributes->set('_check_controller_is_allowed', $check);
        $check = typed_value($check);
    }

    $logger = new Logger();
    $resolver = new ControllerResolver($logger);
    $allow = [];
    if (chance(45)) {
        for ($k = mt_rand(1, 2); $k > 0; --$k) {
            $types = [];
            for ($t = mt_rand(0, 3); $t > 0; --$t) {
                $types[] = chance(80) ? class_name_variant(pick($decoded)['name']) : pick(['Closure', 'closure', 'Nope', 'stdClass']);
            }
            $attributes = [];
            for ($t = mt_rand(0, 2); $t > 0; --$t) {
                $attributes[] = pick([AS_CONTROLLER, $world['ns'].'\\Dummy', $world['ns'].'\\Other', 'Other', strtolower($world['ns'].'\\Dummy')]);
            }
            $resolver->allowControllers($types, $attributes);
            $allow[] = [array_map('base64_encode', $types), $attributes];
        }
    }

    try {
        $result = $resolver->getController($request);
        if (false === $result) {
            $outcome = ['false'];
        } else {
            $outcome = ['returns', encode($result, $objects, $closures), call($result), base64_encode(pretty_name($result))];
        }
    } catch (\Throwable $e) {
        $outcome = ['throws', throwable($e)];
    }

    $scripts[] = [
        'classes' => $world['classes'],
        'functions' => $world['functions'],
        'objects' => $objectSpecs,
        'closures' => $closureSpecs,
        'controller' => $spec,
        'path' => $request->getPathInfo(),
        'check' => $check,
        'allow' => $allow,
        'outcome' => $outcome,
        'logs' => $logger->calls,
    ];
}

echo json_encode(['file' => (new \ReflectionClass(ControllerResolver::class))->getFileName(), 'scripts' => $scripts], \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR);
