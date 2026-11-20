<?php
// The data providers of symfony/finder's test classes, as typed JSON
// (scripts/record.sh finder): for each test method, the rows of every
// #[DataProvider] it names (in attribute order; GitignoreTest::testToRegex
// has two), so the Rust ports run on exactly upstream's data. Unlike
// test_providers.php this reaches classes in Tests/ subdirectories.
// Providers that build paths under the Finder test directory
// (RealIteratorTestCase::toAbsolute) return them under "tmpDir" below;
// the Rust tests replace that prefix with their own fixture directory.
// Closures and mock objects cannot be replayed; those rows are encoded
// as objects and the Rust tests write their providers by hand.
//
//   php finder_providers.php

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

$tests = '/work/upstream/symfony/finder/Tests';
spl_autoload_register(static function (string $name) use ($tests): void {
    $prefix = 'Symfony\\Component\\Finder\\Tests\\';
    if (str_starts_with($name, $prefix) && is_file($file = $tests . '/' . str_replace('\\', '/', substr($name, \strlen($prefix))) . '.php')) {
        require_once $file;
    }
});

$classes = [
    'Comparator/ComparatorTest',
    'Comparator/DateComparatorTest',
    'Comparator/NumberComparatorTest',
    'FinderTest',
    'GitignoreTest',
    'Iterator/FilenameFilterIteratorTest',
    'Iterator/MultiplePcreFilterIteratorTest',
    'Iterator/VcsIgnoredFilterIteratorTest',
];

$out = [];
foreach ($classes as $path) {
    $class = 'Symfony\\Component\\Finder\\Tests\\' . str_replace('/', '\\', $path);
    $reflection = new ReflectionClass($class);
    $methods = [];
    foreach ($reflection->getMethods() as $method) {
        $rows = [];
        $providers = [];
        foreach ($method->getAttributes(PHPUnit\Framework\Attributes\DataProvider::class) as $attribute) {
            $provider = $attribute->getArguments()[0];
            $providers[] = $provider;
            foreach ($reflection->getMethod($provider)->invoke(null) as $name => $arguments) {
                $rows[] = [(string) $name, array_map('typed_value', array_values($arguments))];
            }
        }
        if ($providers) {
            $methods[$method->getName()] = ['providers' => $providers, 'rows' => $rows];
        }
    }
    ksort($methods);
    $out[basename($path)] = $methods;
}

$tmpDir = realpath(sys_get_temp_dir()) . '/symfony_finder';
$lines = ['  "tmpDir": ' . json_encode($tmpDir, JSON_UNESCAPED_SLASHES)];
foreach ($out as $class => $methods) {
    $inner = [];
    foreach ($methods as $test => $data) {
        $inner[] = '    ' . json_encode($test) . ': ' . json_encode($data, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
    }
    $lines[] = '  ' . json_encode($class) . ": {\n" . implode(",\n", $inner) . "\n  }";
}
echo "{\n", implode(",\n", $lines), "\n}\n";
