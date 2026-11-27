<?php
// Record YamlDumper's output for a Windows-style absolute path.
// Run with scripts/record.sh di-yaml-windows-file.
require __DIR__.'/lib/di_autoload.php';

spl_autoload_register(static function (string $class): void {
    $prefix = 'Symfony\\Component\\Yaml\\';
    if (str_starts_with($class, $prefix)) {
        $file = '/work/upstream/symfony/yaml/'.str_replace('\\', '/', substr($class, strlen($prefix))).'.php';
        if (is_file($file)) {
            require $file;
        }
    }
});

$container = new Symfony\Component\DependencyInjection\ContainerBuilder();
$container->register('example', 'Example')
    ->setPublic(true)
    ->setFile('C:\\project\\includes\\foo.php');
echo (new Symfony\Component\DependencyInjection\Dumper\YamlDumper($container))->dump();
