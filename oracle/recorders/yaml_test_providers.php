<?php
// The data providers of an upstream Symfony Yaml test class, as typed
// JSON (scripts/record.sh yaml): for each test method with a
// #[DataProvider], the provider's rows (name, arguments). The Rust ports
// of those tests read these rows, so they run on exactly upstream's data.
//
//   php yaml_test_providers.php InlineTest

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

$tests = '/work/upstream/symfony/yaml/Tests';
foreach (glob("$tests/Fixtures/*.php") as $fixture) {
    require_once $fixture;
}
$class = $argv[1] ?? 'InlineTest';
require_once "$tests/$class.php";

$reflection = new ReflectionClass("Symfony\\Component\\Yaml\\Tests\\$class");
$out = [];
foreach ($reflection->getMethods() as $method) {
    foreach ($method->getAttributes(PHPUnit\Framework\Attributes\DataProvider::class) as $attribute) {
        $provider = $attribute->getArguments()[0];
        $rows = [];
        foreach ($reflection->getMethod($provider)->invoke(null) as $name => $arguments) {
            $rows[] = [(string) $name, array_map('typed_value', array_values($arguments))];
        }
        $out[$method->getName()] = ['provider' => $provider, 'rows' => $rows];
    }
}
ksort($out);

$lines = [];
foreach ($out as $test => $data) {
    $lines[] = '  ' . json_encode($test) . ': ' . json_encode($data, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
}
echo "{\n", implode(",\n", $lines), "\n}\n";
