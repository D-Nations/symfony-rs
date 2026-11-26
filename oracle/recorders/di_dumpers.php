<?php
// Dumper vectors (scripts/record.sh di-dumpers): what the real XmlDumper,
// YamlDumper and GraphvizDumper of symfony/dependency-injection write for
// the containers of their upstream tests (Tests/Fixtures/containers/*.php
// and the ones the tests build), the compiled ones of
// XmlDumperTest::testCompiledContainerCanBeDumped (not container9: its
// Expression needs symfony/cache to compile), and an "edge"
// container the tests leave open: entities, CR, invalid UTF-8, numeric
// look-alikes, floats, int keys, tags with scalar and nested attributes,
// every argument type, env placeholders, decorations, factories,
// deprecated and chained aliases. Replayed by
// crates/dependency-injection/tests/{xml,yaml,graphviz}_dumper_test.rs,
// whose support/containers.rs builds the same containers in Rust.
//
// Output: {"<dumper>:<case>": ["ok", base64(output)] |
// ["throws", class, message]}. The fixtures path in container9's file is
// written as "%path%", as the upstream fixtures write it.
//
// Output: crates/dependency-injection/tests/data/dumpers.json

ini_set('display_errors', 'stderr');
error_reporting(E_ALL & ~E_DEPRECATED & ~E_USER_DEPRECATED);
require __DIR__.'/lib/di_autoload.php';

spl_autoload_register(static function (string $class): void {
    foreach ([
        'Symfony\\Component\\Yaml\\' => '/work/upstream/symfony/yaml/',
        'Symfony\\Component\\ExpressionLanguage\\' => '/work/upstream/symfony/expression-language/',
        'Symfony\\Component\\VarExporter\\' => '/work/upstream/symfony/var-exporter/',
    ] as $prefix => $dir) {
        if (str_starts_with($class, $prefix)) {
            $file = $dir.str_replace('\\', '/', substr($class, \strlen($prefix))).'.php';
            if (is_file($file)) {
                require $file;
            }

            return;
        }
    }
});

// Without symfony/deprecation-contracts in upstream/, deprecations are
// silenced (the dumps do not depend on them).
if (!function_exists('trigger_deprecation')) {
    function trigger_deprecation(string $package, string $version, string $message, mixed ...$args): void
    {
    }
}

use Symfony\Component\DependencyInjection\Argument\AbstractArgument;
use Symfony\Component\DependencyInjection\Argument\EnvClosureArgument;
use Symfony\Component\DependencyInjection\Argument\IteratorArgument;
use Symfony\Component\DependencyInjection\Argument\ServiceClosureArgument;
use Symfony\Component\DependencyInjection\Argument\ServiceLocatorArgument;
use Symfony\Component\DependencyInjection\Argument\TaggedIteratorArgument;
use Symfony\Component\DependencyInjection\Compiler\AutowirePass;
use Symfony\Component\DependencyInjection\ContainerBuilder;
use Symfony\Component\DependencyInjection\ContainerInterface;
use Symfony\Component\DependencyInjection\Definition;
use Symfony\Component\DependencyInjection\Dumper\GraphvizDumper;
use Symfony\Component\DependencyInjection\Dumper\XmlDumper;
use Symfony\Component\DependencyInjection\Dumper\YamlDumper;
use Symfony\Component\DependencyInjection\Parameter;
use Symfony\Component\DependencyInjection\Reference;
use Symfony\Component\DependencyInjection\Tests\Fixtures\FooUnitEnum;
use Symfony\Component\ExpressionLanguage\Expression;

$fixtures = realpath('/work/upstream/symfony/dependency-injection/Tests/Fixtures');

function fixture(string $name): ContainerBuilder
{
    global $fixtures;

    return include $fixtures.'/containers/'.$name.'.php';
}

function tagged_arguments(bool $legacy): ContainerBuilder
{
    if ($legacy) {
        $taggedIterator = new TaggedIteratorArgument('foo_tag', 'barfoo', 'foobar', false, 'getPriority');
        $taggedIterator2 = new TaggedIteratorArgument('foo_tag', null, null, false, null, ['baz']);
        $taggedIterator3 = new TaggedIteratorArgument('foo_tag', null, null, false, null, ['baz', 'qux'], false);
    } else {
        $taggedIterator = new TaggedIteratorArgument('foo_tag', 'barfoo');
        $taggedIterator2 = new TaggedIteratorArgument('foo_tag', null, false, ['baz']);
        $taggedIterator3 = new TaggedIteratorArgument('foo_tag', null, false, ['baz', 'qux'], false);
    }
    $container = new ContainerBuilder();
    $container->register('foo', 'Foo')->addTag('foo_tag');
    $container->register('baz', 'Baz')->addTag('foo_tag');
    $container->register('qux', 'Qux')->addTag('foo_tag');
    foreach ([
        'foo_tagged_iterator' => $taggedIterator,
        'foo2_tagged_iterator' => $taggedIterator2,
        'foo3_tagged_iterator' => $taggedIterator3,
        'foo_tagged_locator' => new ServiceLocatorArgument($taggedIterator),
        'foo2_tagged_locator' => new ServiceLocatorArgument($taggedIterator2),
        'foo3_tagged_locator' => new ServiceLocatorArgument($taggedIterator3),
    ] as $id => $argument) {
        $container->register($id, 'Bar')->setPublic(true)->addArgument($argument);
    }

    return $container;
}

function yaml_tagged_arguments(bool $legacy): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo_service', 'Foo')->addTag('foo');
    $container->register('baz_service', 'Baz')->addTag('foo');
    $container->register('qux_service', 'Qux')->addTag('foo');
    if ($legacy) {
        $taggedIterator = new TaggedIteratorArgument('foo', 'barfoo', 'foobar', false, 'getPriority');
        $container->register('foo_service_tagged_iterator', 'Bar')->addArgument($taggedIterator);
        $container->register('foo_service_tagged_locator', 'Bar')->addArgument(new ServiceLocatorArgument($taggedIterator));

        return $container;
    }
    $taggedIterator = new TaggedIteratorArgument('foo', null, false, ['baz']);
    $taggedIterator2 = new TaggedIteratorArgument('foo', null, false, ['baz', 'qux'], false);
    $container->register('foo_service_tagged_iterator', 'Bar')->addArgument($taggedIterator);
    $container->register('foo2_service_tagged_iterator', 'Bar')->addArgument($taggedIterator2);
    $container->register('foo_service_tagged_locator', 'Bar')->addArgument(new ServiceLocatorArgument($taggedIterator));
    $container->register('foo2_service_tagged_locator', 'Bar')->addArgument(new ServiceLocatorArgument($taggedIterator2));
    $container->register('bar_service_tagged_locator', 'Bar')->addArgument(new ServiceLocatorArgument(new TaggedIteratorArgument('foo')));

    return $container;
}

function service_closure(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo', 'Foo')
        ->addArgument(new ServiceClosureArgument(new Reference('bar', ContainerInterface::IGNORE_ON_INVALID_REFERENCE)));

    return $container;
}

function env_closure(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo', 'Foo')
        ->addArgument(new EnvClosureArgument('%env(FOO)%'))
        ->addArgument(new EnvClosureArgument('%env(FOO)%', null, true))
        ->addArgument(new EnvClosureArgument('%env(BAR)%', 'def', true))
        ->addArgument(new EnvClosureArgument('%env(FOO)%', 42));

    return $container;
}

function enumeration(): ContainerBuilder
{
    $class = 'Symfony\Component\DependencyInjection\Tests\Fixtures\FooClassWithEnumAttribute';
    $container = new ContainerBuilder();
    $container->register($class, $class)->setPublic(true)->addArgument(FooUnitEnum::BAR);
    $container->setParameter('unit_enum', FooUnitEnum::BAR);
    $container->setParameter('enum_array', [FooUnitEnum::BAR, FooUnitEnum::FOO]);
    $container->compile();

    return $container;
}

function default_attribute(string $class): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo', 'Symfony\Component\DependencyInjection\Tests\Fixtures\\'.$class)
        ->setPublic(true)
        ->setAutowired(true)
        ->setArguments([2 => true]);
    (new AutowirePass())->process($container);

    return $container;
}

function abstract_argument(): ContainerBuilder
{
    $class = 'Symfony\Component\DependencyInjection\Tests\Fixtures\FooWithAbstractArgument';
    $container = new ContainerBuilder();
    $container->register($class, $class)
        ->setArgument('$baz', new AbstractArgument('should be defined by Pass'))
        ->setArgument('$bar', 'test');

    return $container;
}

function object_argument(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo', 'FooClass')->addArgument(new \stdClass())->setPublic(true);

    return $container;
}

function inline_services(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo', 'Class1')
        ->setPublic(true)
        ->addArgument((new Definition('Class2'))->addArgument(new Definition('Class2')));

    return $container;
}

function tagged_with_index(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo_service', 'Foo')->addTag('foo');
    $container->register('with_index', 'Bar')->addArgument(new TaggedIteratorArgument('foo', 'barfoo'));

    return $container;
}

function env_placeholders(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->setParameter('%env(PARAMETER_NAME)%', '%env(PARAMETER_VALUE)%');
    $container
        ->register('service', '%env(SERVICE_CLASS)%')
        ->setFile('%env(SERVICE_FILE)%')
        ->addArgument('%env(SERVICE_ARGUMENT)%')
        ->setProperty('%env(SERVICE_PROPERTY_NAME)%', '%env(SERVICE_PROPERTY_VALUE)%')
        ->addMethodCall('%env(SERVICE_METHOD_NAME)%', ['%env(SERVICE_METHOD_ARGUMENT)%'])
        ->setFactory('%env(SERVICE_FACTORY)%')
        ->setConfigurator('%env(SERVICE_CONFIGURATOR)%')
        ->setPublic(true);
    $container->compile();

    return $container;
}

function inline_definition(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo', 'stdClass')->addArgument(
        (new Definition('stdClass'))->addArgument(new Reference('bar'))
    );
    $container->register('bar', 'stdClass');

    return $container;
}

function exclude_self_on_tag_name(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $container->register('foo', 'Foo')->addArgument(new TaggedIteratorArgument('foo', null, false, [], false));

    return $container;
}

function compiled(ContainerBuilder $container): ContainerBuilder
{
    $container->compile();

    return $container;
}

function edge(): ContainerBuilder
{
    $container = new ContainerBuilder();
    $port = $container->getParameterBag()->get('env(int:PORT)');
    $container->getParameterBag()->add([
        'cr' => "a\rb",
        'quotes' => 'quote"apos\'',
        'entities' => '<&>',
        'one' => 1.0,
        'sum' => 0.1 + 0.2,
        'negzero' => -0.0,
        'big' => 1e100,
        'max' => \PHP_INT_MAX,
        'holes' => [1 => 'a', 3 => 'b'],
        'empty_array' => [],
        'zero' => '0',
        'empty' => '',
        'lead_space' => ' 12',
        'trail_space' => '12 ',
        'upper_bin' => '0B01',
        'bare_hex' => '0x',
        'bad_hex' => '0xg',
        'bin_nl' => "0b01\n",
        'null_nl' => "null\n",
        'invalid_utf8' => "a\xffb",
        'tab_nl' => "a\tb\nc",
        'at' => '@foo',
        'percent' => '100%',
        'ref_param' => 'edge',
        'nested' => ['k"ey' => ['<k>' => '%x%']],
        'port' => 'http://host:'.$port,
    ]);

    $edge = $container->register('edge', '\Leading\Slash')
        ->setLazy(true)
        ->setAutoconfigured(true)
        ->setShared(false)
        ->setFile('/path/to/file.php')
        ->addTag('int_keys', ['x', 'y'])
        ->addTag('scalars', ['t' => true, 'f' => false, 'i' => 7, 'fl' => 1.5, 'n' => null, 'q' => 'a"b<c>'])
        ->addTag('named', ['name' => 'the name', 'nested' => ['n' => null, 'e' => '', 'v' => 'v', 'deep' => ['z' => 0]]])
        ->addTag('0')
        ->addTag('cr', ['v' => "x\ry"])
        ->addError('First "error".')
        ->addError('Second <error>.')
        ->setArguments([
            new ServiceLocatorArgument(['a' => new Reference('missing_a'), 'b' => new Reference('missing_b', ContainerInterface::NULL_ON_INVALID_REFERENCE)]),
            new ServiceClosureArgument(new Definition('ClosureTarget')),
            new ServiceClosureArgument(new Reference('lazy_target', ContainerInterface::IGNORE_ON_UNINITIALIZED_REFERENCE)),
            new IteratorArgument([new Reference('iter_target', ContainerInterface::IGNORE_ON_UNINITIALIZED_REFERENCE)]),
            new TaggedIteratorArgument('app.handler', 'foo-bar baz'),
            new TaggedIteratorArgument('app.handler', null, true),
            new ServiceLocatorArgument(new TaggedIteratorArgument('app.locator', 'key', false, ['one', 'two'])),
            new Parameter('ref_param'),
            new Expression('1 + "two"'),
            new AbstractArgument('to be set.'),
            FooUnitEnum::FOO,
            "\x01\x02",
            "\xff",
            "a\rb",
            '',
            null,
            true,
            false,
            0,
            -1.5,
            '42',
            'null',
            ['k"ey' => 'v', '<k>' => ['x' => new Reference('nested_target')], 5 => 'five'],
            [],
            'http://'.$port.'/',
            (new Definition('Inline'))->addArgument("\xfe")->addArgument(new Reference('inline_target'))->addTag('inline_tag', ['a' => 'b']),
        ])
        ->setProperties(['p' => new Reference('prop_target'), 'q' => 'quoted "q"'])
        ->addMethodCall('withClone', ['x'], true)
        ->addMethodCall('noArgs', [], true)
        ->addMethodCall('call', [new Reference('call_target')])
        ->setDecoratedService('decorated_one', 'inner', -5, ContainerInterface::IGNORE_ON_INVALID_REFERENCE)
        ->setConfigurator('configure_function');

    $container->register('closure_factory', 'Closure')->setFactory(['Closure', 'fromCallable'])->setPublic(true);
    $container->register('null_class')->setFactory([null, 'create']);
    $container->register('invokable', 'Invokable')->setFactory(new Reference('edge'));
    $container->register('function_factory', 'Made')->setFactory('make_it');
    $container->register('we"ird<&>', 'Weird')->setAbstract(true)->setDeprecated('vendor/pkg', '2.0', 'The "%service_id%" <service> is "gone".');
    $container->setAlias('edge_alias', 'edge')->setDeprecated('vendor/pkg', '3.0', 'The "%alias_id%" alias is gone.');
    $container->setAlias('public_alias', 'edge')->setPublic(true);
    $container->setAlias('chained_private', 'public_alias');
    $container->setAlias('private_alias', 'invokable');

    return $container;
}

$cases = [
    'xml:empty' => fn () => new ContainerBuilder(),
    'xml:container8' => fn () => fixture('container8'),
    'xml:container9' => fn () => fixture('container9'),
    'xml:container11' => fn () => fixture('container11'),
    'xml:container12' => fn () => fixture('container12'),
    'xml:container15' => fn () => fixture('container15'),
    'xml:container16' => fn () => fixture('container16'),
    'xml:container34' => fn () => fixture('container34'),
    'xml:container21' => fn () => fixture('container21'),
    'xml:container24' => fn () => fixture('container24'),
    'xml:tagged_arguments' => fn () => tagged_arguments(false),
    'xml:legacy_tagged_arguments' => fn () => tagged_arguments(true),
    'xml:service_closure' => 'service_closure',
    'xml:env_closure' => 'env_closure',
    'xml:container_abstract' => fn () => fixture('container_abstract'),
    'xml:enumeration' => 'enumeration',
    'xml:default_array' => fn () => default_attribute('FooClassWithDefaultArrayAttribute'),
    'xml:default_object' => fn () => default_attribute('FooClassWithDefaultObjectAttribute'),
    'xml:default_enumeration' => fn () => default_attribute('FooClassWithDefaultEnumAttribute'),
    'xml:abstract_argument' => 'abstract_argument',
    'xml:non_scalar_tags' => fn () => fixture('container_non_scalar_tags'),
    'xml:object_argument' => 'object_argument',
    'xml:edge' => 'edge',
    'xml:compiled_container8' => fn () => compiled(fixture('container8')),
    'xml:compiled_container11' => fn () => compiled(fixture('container11')),
    'xml:compiled_container12' => fn () => compiled(fixture('container12')),
    'xml:compiled_container14' => fn () => compiled(fixture('container14')),
    'yaml:empty' => fn () => new ContainerBuilder(),
    'yaml:container8' => fn () => fixture('container8'),
    'yaml:container9' => fn () => fixture('container9'),
    'yaml:container24' => fn () => fixture('container24'),
    'yaml:container34' => fn () => fixture('container34'),
    'yaml:inline_services' => 'inline_services',
    'yaml:tagged_arguments' => fn () => yaml_tagged_arguments(false),
    'yaml:deprecated_tagged_arguments' => fn () => yaml_tagged_arguments(true),
    'yaml:tagged_with_index' => 'tagged_with_index',
    'yaml:service_closure' => 'service_closure',
    'yaml:env_closure' => 'env_closure',
    'yaml:enumeration' => 'enumeration',
    'yaml:default_array' => fn () => default_attribute('FooClassWithDefaultArrayAttribute'),
    'yaml:default_object' => fn () => default_attribute('FooClassWithDefaultObjectAttribute'),
    'yaml:default_enumeration' => fn () => default_attribute('FooClassWithDefaultEnumAttribute'),
    'yaml:abstract_argument' => 'abstract_argument',
    'yaml:non_scalar_tags' => fn () => fixture('container_non_scalar_tags'),
    'yaml:env_placeholders' => 'env_placeholders',
    'yaml:object_argument' => 'object_argument',
    'yaml:exclude_self_on_tag_name' => 'exclude_self_on_tag_name',
    'yaml:edge' => 'edge',
    'dot:empty' => fn () => new ContainerBuilder(),
    'dot:container9' => fn () => fixture('container9'),
    'dot:container10' => fn () => fixture('container10'),
    'dot:container10_options' => fn () => fixture('container10'),
    'dot:container13' => fn () => fixture('container13'),
    'dot:container14' => fn () => fixture('container14'),
    'dot:container17' => fn () => fixture('container17'),
    'dot:inline_definition' => 'inline_definition',
    'dot:edge' => 'edge',
];

$dotOptions = [
    'graph' => ['ratio' => 'normal'],
    'node' => ['fontsize' => 13, 'fontname' => 'Verdana', 'shape' => 'square'],
    'edge' => ['fontsize' => 12, 'fontname' => 'Verdana', 'color' => 'white', 'arrowhead' => 'closed', 'arrowsize' => 1],
    'node.instance' => ['fillcolor' => 'green', 'style' => 'empty'],
    'node.definition' => ['fillcolor' => 'grey'],
    'node.missing' => ['fillcolor' => 'red', 'style' => 'empty'],
];

$out = [];
foreach ($cases as $name => $build) {
    [$dumper] = explode(':', $name);
    try {
        $container = $build();
        $output = match ($dumper) {
            'xml' => (new XmlDumper($container))->dump(),
            'yaml' => (new YamlDumper($container))->dump(),
            'dot' => (new GraphvizDumper($container))->dump('dot:container10_options' === $name ? $dotOptions : []),
        };
        $output = str_replace($fixtures.\DIRECTORY_SEPARATOR.'includes'.\DIRECTORY_SEPARATOR, '%path%', $output);
        $out[$name] = ['ok', base64_encode($output)];
    } catch (\Throwable $e) {
        $out[$name] = ['throws', $e::class, $e->getMessage()];
    }
}

echo json_encode($out, \JSON_PRETTY_PRINT | \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR | \JSON_INVALID_UTF8_SUBSTITUTE), "\n";
