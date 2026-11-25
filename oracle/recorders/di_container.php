<?php
// The symfony/demo container after all compiler passes (APP_ENV=prod,
// APP_DEBUG=0), as Symfony's XmlDumper writes it. This is the graph
// PhpDumper turns into the cached PHP container: removal passes have run,
// so unused private services are gone and nothing is abstract.
// scripts/record.sh di-trial writes the output to
// spikes/di-trial/demo/data/container.xml.
//
// With the argument "classes", it prints instead, for each public service,
// the type of the object PHP's compiled container returns
// (get_debug_type()), or the exception it throws, as JSON
// (spikes/di-trial/demo/tests/data/php_classes.json).
//
// XmlDumper writes a tagged iterator or tagged locator as its tag only,
// although the compiled argument already holds the services it resolved
// to (sorted by priority, keyed by index-by). Before dumping, each one is
// replaced by a plain iterator or locator of those resolved values, so the
// file is the whole graph.
//
// The container is the one a production install builds: classes load as
// after `composer install --no-dev` (lib/no_dev_autoload.php), although the
// demo's vendor/ also holds its dev packages.

require __DIR__.'/lib/no_dev_autoload.php';
no_dev_autoload('/demo');

use Symfony\Component\DependencyInjection\Argument\IteratorArgument;
use Symfony\Component\DependencyInjection\Argument\ServiceLocatorArgument;
use Symfony\Component\DependencyInjection\Argument\TaggedIteratorArgument;
use Symfony\Component\DependencyInjection\Definition;
use Symfony\Component\Dotenv\Dotenv;

function untagValue(mixed $value): mixed
{
    if ($value instanceof TaggedIteratorArgument) {
        return new IteratorArgument(untagValue($value->getValues()));
    }
    if ($value instanceof ServiceLocatorArgument) {
        return new ServiceLocatorArgument(untagValue($value->getValues()));
    }
    if ($value instanceof IteratorArgument) {
        $value->setValues(untagValue($value->getValues()));

        return $value;
    }
    if ($value instanceof Definition) {
        untag($value);

        return $value;
    }
    if (is_array($value)) {
        return array_map(untagValue(...), $value);
    }

    return $value;
}

function untag(Definition $definition): void
{
    $definition->setArguments(untagValue($definition->getArguments()));
    $definition->setProperties(untagValue($definition->getProperties()));
    $definition->setMethodCalls(array_map(
        static fn (array $call) => [$call[0], untagValue($call[1]), ...array_slice($call, 2)],
        $definition->getMethodCalls(),
    ));
    if (is_array($factory = $definition->getFactory()) && $factory[0] instanceof Definition) {
        untag($factory[0]);
    }
    if (is_array($configurator = $definition->getConfigurator()) && $configurator[0] instanceof Definition) {
        untag($configurator[0]);
    }
}

// The demo's App\Kernel, renamed to App\DemoKernel, so that the subclass
// below can keep the name App\Kernel: the class name appears in the
// container (kernel.container_class, service ids), and the dump must read
// as the demo's own. Run with a tmpfs on /demo/var (/demo is read-only).
eval('?>'.str_replace('class Kernel extends', 'class DemoKernel extends', file_get_contents('/demo/src/Kernel.php')));
eval(<<<'PHP'
namespace App;

use Symfony\Component\Config\ConfigCache;
use Symfony\Component\DependencyInjection\ContainerBuilder;
use Symfony\Component\DependencyInjection\Dumper\XmlDumper;

final class Kernel extends DemoKernel
{
    public static ?string $xml = null;

    public function getProjectDir(): string
    {
        return '/demo';
    }

    protected function dumpContainer(ConfigCache $cache, ContainerBuilder $container, string $class, string $baseClass): void
    {
        parent::dumpContainer($cache, $container, $class, $baseClass);
        foreach ($container->getDefinitions() as $definition) {
            \untag($definition);
        }
        self::$xml = (new XmlDumper($container))->dump();
    }
}
PHP);

// As bin/console does, with APP_ENV=prod and APP_DEBUG=0 set first.
$_SERVER['APP_ENV'] = $_ENV['APP_ENV'] = 'prod';
$_SERVER['APP_DEBUG'] = $_ENV['APP_DEBUG'] = '0';
(new Dotenv())->bootEnv('/demo/.env');

$kernel = new App\Kernel('prod', false);
$kernel->boot();
if (null === App\Kernel::$xml) {
    fwrite(STDERR, "the container was not compiled\n");
    exit(1);
}
if ('classes' !== ($argv[1] ?? '')) {
    echo App\Kernel::$xml;
    exit(0);
}

$container = $kernel->getContainer();
$ids = [];
$dom = new DOMDocument();
$dom->loadXML(App\Kernel::$xml);
$xpath = new DOMXPath($dom);
$xpath->registerNamespace('c', 'http://symfony.com/schema/dic/services');
foreach ($xpath->query('/c:container/c:services/c:service[@public="true"]') as $node) {
    $ids[] = $node->getAttribute('id');
}
sort($ids);
$types = [];
foreach ($ids as $id) {
    try {
        $types[$id] = get_debug_type($container->get($id));
    } catch (Throwable $e) {
        $types[$id] = 'exception: '.get_class($e).': '.$e->getMessage();
    }
}
echo json_encode($types, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES), "\n";
