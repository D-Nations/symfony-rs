<?php
// AcceptHeader and AcceptHeaderItem answers for what their tests leave
// open, for crates/http-foundation/tests/data/accept_header.json
// (scripts/record.sh http-foundation): generated headers through
// fromString(), all() (keys, values, qualities, indexes, attributes),
// __toString() before sorting, has() and get() for generated queries,
// filter() with a few patterns, and AcceptHeaderItem::fromString() and
// __toString(). Strings are base64; a thrown error is ["throw", class,
// message].

error_reporting(E_ALL);

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\HttpFoundation\AcceptHeader;
use Symfony\Component\HttpFoundation\AcceptHeaderItem;

mt_srand(20261123);

function item_answer(AcceptHeaderItem $item): array
{
    return [base64_encode($item->getValue()), bin2hex(pack('E', $item->getQuality())), $item->getIndex(), typed_value($item->getAttributes()), base64_encode((string) $item)];
}

function attempt(callable $f): mixed
{
    try {
        return $f();
    } catch (\Throwable $e) {
        return ['throw', $e::class, $e->getMessage()];
    }
}

$values = ['text/html', 'text/plain', 'text/*', '*/*', '*', 'TEXT/Plain', 'text/', 'text', 'application/json', 'utf-8', 'UTF-8',
    'fr-FR', 'fr', 'en', 'gzip', '0', '1', '"q;u=o,t"', '"a\\"b"', 'é', 'x y', ''];
$params = ['q=0.5', 'q=0', 'q=1', 'q=0.25', 'Q=0.7', 'q=abc', 'q=0.8x', 'q', 'q=', 'q=1e-1', 'q=2', 'q=-1',
    'format=flowed', 'Format=fixed', 'format=FLOWED', 'level=1', 'level=2', 'charset=utf-8', 'Charset=UTF-8', 'flowed',
    'param="value with space"', 'param="a\\\\b"', 'param=', 'param=x', '1=a', 'a=1', 'p="x;y"', 'p=a=b'];
$spaces = ['', ' ', "\t", '  '];

function generated(int $maxItems): string
{
    global $values, $params, $spaces;
    $items = [];
    $count = mt_rand(0, $maxItems);
    for ($i = 0; $i < $count; ++$i) {
        $item = $values[mt_rand(0, \count($values) - 1)];
        $paramCount = mt_rand(0, 3);
        for ($j = 0; $j < $paramCount; ++$j) {
            $item .= ';' . $spaces[mt_rand(0, \count($spaces) - 1)] . $params[mt_rand(0, \count($params) - 1)];
        }
        $items[] = $item;
    }

    return implode(',' . $spaces[mt_rand(0, \count($spaces) - 1)], $items);
}

$headers = [];
for ($n = 0; $n < 1500; ++$n) {
    $string = generated(6);
    $header = AcceptHeader::fromString($string);
    $unsorted = (string) $header;
    $all = [];
    foreach ($header->all() as $key => $item) {
        $all[] = [typed_value($key), item_answer($item)];
    }
    $queries = [];
    for ($q = 0; $q < 4; ++$q) {
        $query = generated(1);
        $has = attempt(fn () => $header->has($query));
        $get = attempt(fn () => ($found = $header->get($query)) ? item_answer($found) : null);
        $queries[] = [base64_encode($query), $has, $get];
    }
    $headers[] = [base64_encode($string), base64_encode($unsorted), $all, $queries];
}

$filters = [];
foreach (['fr-FR,fr;q=0.8,en-US;q=0.6,en;q=0.4', 'text/html,text/plain;q=0.5,application/json', 'gzip, deflate, br'] as $string) {
    foreach (['/fr.*/', '/^en/i', '/x/', '#text/(html|plain)#', '/[/', '/e(?=n)/', '/^$/'] as $pattern) {
        $filtered = attempt(fn () => array_map('strval', array_keys(AcceptHeader::fromString($string)->filter($pattern)->all())));
        $filters[] = [base64_encode($string), base64_encode($pattern), $filtered];
    }
}

$items = [];
for ($n = 0; $n < 500; ++$n) {
    $string = generated(1);
    $items[] = [base64_encode($string), attempt(fn () => item_answer(AcceptHeaderItem::fromString($string)))];
}

// get() among overlapping ranges, where specificity (type precision, then
// the parameter count), quality and index decide: [header, query, answer]
$ranges = ['*/*', '*', 'text/*', 'text/plain', 'TEXT/*', 'text/html', 'utf-8', '*/plain'];
$rangeParams = ['format=flowed', 'charset=utf-8', 'level=1', 'Format=FLOWED', 'flowed'];
$qualities = ['', ';q=0.1', ';q=0.3', ';q=0.5', ';q=0.8', ';q=0'];
$queryValues = ['text/plain', 'text/html', 'TEXT/PLAIN', 'utf-8', 'image/png', '*', 'text/*'];
$specific = [];
for ($n = 0; $n < 2000; ++$n) {
    $rangeItems = [];
    $count = mt_rand(1, 5);
    for ($i = 0; $i < $count; ++$i) {
        $item = $ranges[mt_rand(0, \count($ranges) - 1)];
        $paramCount = mt_rand(0, 2);
        for ($j = 0; $j < $paramCount; ++$j) {
            $item .= ';' . $rangeParams[mt_rand(0, \count($rangeParams) - 1)];
        }
        $rangeItems[] = $item . $qualities[mt_rand(0, \count($qualities) - 1)];
    }
    $string = implode(', ', $rangeItems);
    $query = $queryValues[mt_rand(0, \count($queryValues) - 1)];
    $paramCount = mt_rand(0, 3);
    for ($j = 0; $j < $paramCount; ++$j) {
        $query .= ';' . $rangeParams[mt_rand(0, \count($rangeParams) - 1)];
    }
    $header = AcceptHeader::fromString($string);
    $specific[] = [base64_encode($string), base64_encode($query), attempt(fn () => ($found = $header->get($query)) ? item_answer($found) : null)];
}

echo json_encode(['headers' => $headers, 'filters' => $filters, 'items' => $items, 'specific' => $specific], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
