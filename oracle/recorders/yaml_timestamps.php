<?php
// YAML timestamps, for crates/yaml/tests/data/timestamps.json
// (scripts/record.sh yaml). Every input matches
// Symfony's timestamp regex; PHP's date parser decides the rest:
// per input, DateTimeImmutable's view (or its exception), and what
// Symfony's Inline::parse() returns with and without PARSE_DATETIME.

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Yaml\Inline;
use Symfony\Component\Yaml\Yaml;

mt_srand(17);
$pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];
$years = ['0000', '0001', '1730', '1969', '1970', '2001', '2038', '2024', '9999'];
$months = ['1', '01', '2', '02', '12', '13', '0', '00', '9'];
$days = ['1', '01', '28', '29', '30', '31', '32', '0', '00', '9'];
$hours = ['0', '00', '9', '23', '24', '25', '99'];
$minutes = ['00', '07', '59', '60', '99'];
$seconds = ['00', '43', '59', '60', '61', '99'];
$fractions = [null, '', '1', '12', '123', '123456', '1234567', '000000', '5', '100', '999999999'];
$zones = [null, 'Z', '+1', '+01', '-05', '+05:30', '-0', '+14', '+15', '+24', '+99', '+01:60', '-03:00', '+00:00'];
$separators = ['T', 't', ' ', "\t", '  ', " \t"];
$tzSpace = ['', ' ', "\t", '  '];

$inputs = ['2001-12-14t21:59:43.10-05:00', '2001-12-14 21:59:43.10 -5', '2001-12-15 2:59:43.10', '2002-12-14',
    '2001-12-14T21:59:43Z', '1730-10-30T02:59:43Z', '2024-02-30', '2024-13-01', '2023-02-29'];
foreach ($years as $y) {
    foreach (['01-01', '2-29', '12-31', '13-01', '00-10', '02-30'] as $md) {
        $inputs[] = "$y-$md";
    }
}
for ($n = 0; $n < 1200; ++$n) {
    $s = $pick($years) . '-' . $pick($months) . '-' . $pick($days);
    if (mt_rand(0, 9) > 1) {
        $s .= $pick($separators) . $pick($hours) . ':' . $pick($minutes) . ':' . $pick($seconds);
        $fraction = $pick($fractions);
        if (null !== $fraction) {
            $s .= '.' . $fraction;
        }
        $zone = $pick($zones);
        if (null !== $zone) {
            $s .= $pick($tzSpace) . $zone;
        }
    }
    $inputs[] = $s;
}
$inputs = array_values(array_unique($inputs));

$rows = [];
foreach ($inputs as $s) {
    try {
        $time = new \DateTimeImmutable($s, new \DateTimeZone('UTC'));
        $date = [$time->format('U'), $time->format('u'), $time->format('Y-m-d\TH:i:s.uP'), $time->getTimezone()->getName(),
            $time->format('c'), $time->format('Y-m-d\TH:i:s.vP'), $time->format('T'), $time->format('Z')];
    } catch (\Exception $e) {
        $date = null;
    }
    $rows[] = json_encode([
        $s,
        $date,
        typed_call(static fn () => Inline::parse($s)),
        typed_call(static fn () => Inline::parse($s, Yaml::PARSE_DATETIME)),
    ], JSON_UNESCAPED_SLASHES);
}

echo "{\"timestamps\": [\n", implode(",\n", $rows), "\n]}\n";
