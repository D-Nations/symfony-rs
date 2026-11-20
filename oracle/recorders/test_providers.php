<?php
// The data providers of an upstream test class, as typed JSON
// (scripts/record.sh): for each test method with a #[DataProvider], the
// provider's rows (name, arguments), so the Rust ports of those tests run
// on exactly upstream's data. Providers run on the oracle (Linux), so
// rows upstream adds only off Windows are included.
//
//   php test_providers.php symfony/dotenv DotenvTest

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

[, $package, $class] = $argv + [null, null, null];
if (!$package || !$class) {
    fwrite(STDERR, "usage: php test_providers.php PACKAGE CLASS\n");
    exit(2);
}
$tests = "/work/upstream/$package/Tests";
// Parent test cases and fixtures of the package's own Tests namespace
// (e.g. symfony/string's AbstractUnicodeTestCase).
spl_autoload_register(static function (string $name) use ($tests): void {
    $at = strpos($name, '\\Tests\\');
    if (false !== $at && is_file($file = $tests . '/' . str_replace('\\', '/', substr($name, $at + 7)) . '.php')) {
        require_once $file;
    }
});
require_once "$tests/$class.php";

$namespace = null;
foreach (get_declared_classes() as $declared) {
    if (str_ends_with($declared, "\\Tests\\$class")) {
        $namespace = $declared;
    }
}
$reflection = new ReflectionClass($namespace);
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
