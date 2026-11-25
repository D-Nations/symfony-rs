<?php
// ParserTest::testParsingNotReadableFilesThrowsException, with live Unix
// permissions instead of the root oracle's skipped recording.
// Symfony source and tests copyright (c) Fabien Potencier; MIT.
// Run only in the pinned oracle as an unprivileged user:
// scripts/record.sh yaml-unreadable

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Yaml\Parser;

$directory = sys_get_temp_dir() . '/symfony-yaml-unreadable-' . bin2hex(random_bytes(8));
if (!mkdir($directory, 0o700)) {
    throw new RuntimeException('Cannot create the temporary directory.');
}
$fixtures = $directory . '/Fixtures';
$file = $fixtures . '/not_readable.yml';

try {
    if (!mkdir($fixtures, 0o700)
        || !copy('/work/upstream/symfony/yaml/Tests/Fixtures/not_readable.yml', $file)) {
        throw new RuntimeException('Cannot copy the upstream fixture.');
    }
    $parser = new Parser();
    $readable = typed_call(static fn () => $parser->parseFile($file));
    if (!chmod($file, 0o200)) {
        throw new RuntimeException('Cannot set the fixture permissions.');
    }
    clearstatcache(true, $file);
    if (!is_file($file) || (fileperms($file) & 0o777) !== 0o200 || is_readable($file)) {
        throw new LogicException('The fixture must exist and be unreadable. Run the oracle as a non-root user.');
    }
    $unreadable = typed_call(static fn () => $parser->parseFile($file));
    if (!chmod($file, 0o644)) {
        throw new RuntimeException('Cannot restore the fixture permissions.');
    }
    clearstatcache(true, $file);
    $restored = typed_call(static fn () => $parser->parseFile($file));
    // Replace only the generated directory; keep Symfony's message intact.
    if ('throws' === $unreadable[0]) {
        $unreadable[2] = str_replace($directory, '{tmp}', $unreadable[2]);
    }
    $out = [
        'php' => PHP_VERSION,
        'path' => '{tmp}/Fixtures/not_readable.yml',
        'mode' => '0200',
        'readable' => $readable,
        'unreadable' => $unreadable,
        'restored' => $restored,
    ];
    $lines = [];
    foreach ($out as $key => $value) {
        $lines[] = '  ' . json_encode($key) . ': '
            . json_encode($value, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
    }
    echo "{\n", implode(",\n", $lines), "\n}\n";
} finally {
    if (is_file($file)) {
        chmod($file, 0o644);
        unlink($file);
    }
    if (is_dir($fixtures)) {
        rmdir($fixtures);
    }
    rmdir($directory);
}

