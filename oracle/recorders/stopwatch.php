<?php
// The PHP built-ins symfony/stopwatch relies on, as vectors for
// crates/stopwatch/tests/data/php.json (scripts/record.sh stopwatch):
//   round:  round($x, 1), as used by StopwatchEvent::formatTime()
//   int:    (int) $x, as used by StopwatchPeriod in default precision
//   d, F:   sprintf('%d', $x) and sprintf('%.2F', $x) of __toString()
// plus the messages and orders of Stopwatch edge cases.
// Floats travel as 16 hex digits of their IEEE 754 bits, so they are exact.
// Inputs come from a fixed seed, so re-recording reproduces the file.

require '/demo/vendor/autoload.php';

use Symfony\Component\Stopwatch\Stopwatch;

function bits(float $x): string
{
    return bin2hex(pack('E', $x)); // big-endian IEEE 754
}

mt_srand(20261119);
$floats = [0.0, -0.0, 0.05, 0.15, 0.25, 0.35, 0.45, 0.95, 1.45, 2.675, 1.005, 0.285, -0.15, -2.675,
    1e15 + 0.05, 1.5e15, 4.35, 1.0000000000000002, 5e-324, 1e300, -1e300, INF, -INF, NAN];
for ($i = 0; $i < 500; ++$i) {
    // Milliseconds since the epoch, as microtime(true) * 1000 gives them.
    $floats[] = (1763500000 + mt_rand(0, 100000000) + mt_rand(0, 999999) / 1000000) * 1000;
    // Relative times: now - origin, at microsecond resolution.
    $floats[] = mt_rand(0, 10000000) / 1000 + mt_rand(0, 999) / 1000000;
    // Values on and next to a .x5 boundary.
    $k = mt_rand(0, 100000);
    $floats[] = $k / 10 + 0.05;
    $floats[] = ($k * 10 + 5) / 100;
    $floats[] = -($k / 10 + 0.05);
    // Arbitrary doubles of every magnitude.
    $floats[] = mt_rand() / mt_getrandmax() * 10 ** mt_rand(-10, 20);
}

$round = [];
foreach ($floats as $x) {
    $round[] = [bits($x), bits(round($x, 1))];
}

$ints = [];
foreach ([0.0, -0.0, 2.71, -2.71, 1e18, 9.2233720368547758e18, 1e19, -1e19, 1e20, 1.5e300, INF, -INF, NAN] as $x) {
    $ints[] = [bits($x), (int) $x, sprintf('%d', $x)];
}
for ($i = 0; $i < 200; ++$i) {
    $x = mt_rand() / mt_getrandmax() * 10 ** mt_rand(0, 25) * (mt_rand(0, 1) ? 1 : -1);
    $ints[] = [bits($x), (int) $x, sprintf('%d', $x)];
}

$fixed = [];
foreach ([0, 1, 5242, 10485, 131072, 393216, 655360, 1048576, 1572864, 2097152, 134217728, PHP_INT_MAX] as $memory) {
    $fixed[] = [bits($memory / 1024 / 1024), sprintf('%.2F', $memory / 1024 / 1024)];
}
foreach ([0.005, 0.015, 0.125, 0.375, 1.005, 2.675, 1e15, 1e20, -0.125, -0.0, -0.001, INF, -INF, NAN] as $x) {
    $fixed[] = [bits($x), sprintf('%.2F', $x)];
}

$messages = [];
$try = static function (string $label, callable $f) use (&$messages) {
    try {
        $f();
        $messages[] = [$label, null];
    } catch (\Throwable $e) {
        $messages[] = [$label, get_class($e) . ': ' . $e->getMessage()];
    }
};
$try('stopSection on a new stopwatch', static fn () => (new Stopwatch())->stopSection('x'));
$try('openSection of an unknown id', static fn () => (new Stopwatch())->openSection('x'));
$try('stop of an unknown event', static fn () => (new Stopwatch())->stop('x'));
$try('getEvent of an unknown event', static fn () => (new Stopwatch())->getEvent('x'));
$try('lap of an unknown event', static fn () => (new Stopwatch())->lap('x'));
$try('stop of a stopped event', static function () {
    $s = new Stopwatch();
    $s->start('x');
    $s->stop('x');
    $s->stop('x');
});
$try('reopen a section from another level', static function () {
    $s = new Stopwatch();
    $s->openSection();
    $s->openSection();
    $s->stopSection('inner');
    $s->stopSection('outer');
    $s->openSection('inner');
});

// Event and section order.
$s = new Stopwatch();
$s->start('b');
$s->start('a', 'cat');
$s->openSection();
$s->start('c');
$s->stopSection('one');
$s->openSection();
$s->stopSection('two');
$s->openSection('one');
$s->start('d');
$s->stopSection('one');
$orders = [
    'root events' => array_keys($s->getRootSectionEvents()),
    'sections' => array_keys($s->getSections()),
    'section one' => array_keys($s->getSectionEvents('one')),
    'section two' => array_keys($s->getSectionEvents('two')),
    'root categories' => array_map(static fn ($e) => $e->getCategory(), $s->getRootSectionEvents()),
    'section one categories' => array_map(static fn ($e) => $e->getCategory(), $s->getSectionEvents('one')),
    'section one periods' => array_map(static fn ($e) => count($e->getPeriods()), $s->getSectionEvents('one')),
];

// One vector per line, so a re-recording diffs readably.
$out = [];
foreach (['round' => $round, 'int' => $ints, 'fixed' => $fixed, 'messages' => $messages] as $key => $rows) {
    $out[] = '  ' . json_encode($key) . ": [
    " . implode(",
    ", array_map(
        static fn ($row) => json_encode($row, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR),
        $rows,
    )) . "
  ]";
}
$out[] = '  "orders": ' . json_encode($orders, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
echo "{
", implode(",
", $out), "
}
";
