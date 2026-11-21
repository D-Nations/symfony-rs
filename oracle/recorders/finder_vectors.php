<?php
// Vectors for symfony/finder's port (scripts/record.sh finder): what PHP
// 8.4.27 answers for the PHP behaviour the port writes itself (preg_match
// on the patterns Finder builds or accepts, the comparison operators,
// strnatcmp & co., the date strings DateComparator parses) and what the
// pinned Glob, Gitignore and NumberComparator return. Strings are base64
// where they may hold bytes outside ASCII.
//
//   php finder_vectors.php

require '/demo/vendor/autoload.php';

use Symfony\Component\Finder\Comparator\NumberComparator;
use Symfony\Component\Finder\Gitignore;
use Symfony\Component\Finder\Glob;

date_default_timezone_set('UTC');
$b = static fn (string $s): string => base64_encode($s);

// --- preg_match: pattern x subject -> 1, 0 or false ---------------------
$patterns = [
    '/a/', '/foo/', '/foo/i', '/foo/imsxu', '#foo#', '{foo}', '[foo]', '(foo)', '<foo>', '/foo/n',
    '/foo/bar/', '{*.php,*.twig}', '//', ' /a/ ', '/a', 'afooa', '\\foo\\', '/[unterminated/',
    '/^A\/B\/C/', '/C$/', '/\.php$/', '~^test~i', '~\\.php$~i', '~.*t\\.p.+~i', '~t.*s~i',
    '/^lorem ipsum dolor sit amet$/m', '/^IPSUM/m', '@^L@m', '/A\\/B\\//', '{^A/B/}',
    '#(?:^|/)(?:foo|bar)(?:/|$)#', '#(^|/)\..+(/|$)#',
    '/a$/', '/a$/D', '/a\Z/', '/a\z/', '/^b/m', '/a.b/', '/a.b/s', '/A/i', '/\x41/', '/\xe9/', '/\xe9/u',
    '/é/', '/é/u', '/./u', '/^.$/', '/^.$/u', '/\w/', '/\w/u', '/\d+/', '/\s/', '/\s/u', '/\h/', '/\v/', '/\R/',
    '/[[:alpha:]]+$/', '/[[:^digit:]]/', '/[a-c]/', '/[c-a]/', '/[]a]/', '/[^]a]/', '/[a[]/', '/[\]x]/',
    '/[a\-c]/', '/[{-}]/', '/[\d-z]/', '/a{2}/', '/a{2,}/', '/a{,2}b/', '/a{2,1}/', '/a{/', '/a{x}/', '/{/',
    '/x{1,2}+y/', '/a++b/', '/a*+a/', '/(?=a)a/', '/(?!a)./', '/(?<=a)b/', '/(?<!a)b/', '/(?>a+)b/',
    '/(a)\1/', '/(?<n>a)\k<n>/', '/(?P<n>a)(?P=n)/', '/(?i)a/', '/a(?i)b/', '/(?i:a)b/', '/a(?#c)b/',
    '/a b/x', "/a # c\n b/x", '/a\ b/x', '/[ ]/x', '/a+?/U', '/a+/U', '/(a)|b/n', '/\Qa.b\E/', '/a**/',
    '/*/', '/+a/', '/a|*/', '/(*FAIL)/', '/(?R)/', '/(a/', '/a)/', '/\i/', '/\y/', '/\8/', '/a$f/',
    '/\\\\/', '/\//', '/\#/', '#\##', '/[\b]/', '/\cA/', '/\o{101}/', '/\101/', '/\0/', '/\x{41}/',
    '/\x{100}/', '/\x{100}/u', '/[[:foo:]]/', '/[[.a.]]/', '/(?J)a/', '/a/e', '/a/r', '/a/Z', "/a/\0",
    "/a/ \n", '/(?<1a>x)/', '/(?<a>x)(?<a>y)/', '/\g{-1}/', '/(a)\g{-1}/', '/(a)\g1/',
    '/^(?:(?:(?!\/\/).(?<!\/\/))+)?$/', '~^(?:$f)~s', '~^(?:(?:[^/]+/)*a(?:$|/))~s',
];
$subjects = ['', 'a', 'A', 'aa', 'ab', 'b', 'ba', "a\n", "a\nb", "\nb", 'a b', 'ab c', 'a.b', "a\nb\n",
    'foo', 'FOO', 'xfoox', 'foo/bar', 'test.php', 'test.py', 'TEST.PHP', 'A/B/C/abc.dat', 'copy/A/B/C', 'A/B/',
    'é', "\xe9", "\xc3", 'a1', '12', ' ', "\t", "\x0b", "\x85", "\xc2\x85", '{', '[', ']', '-', 'x', 'y', 'xy', 'xxy',
    'aaa', 'aab', '#', '/', '\\', "\x08", "\x01", 'lorem ipsum dolor sit amet', "lorem ipsum dolor sit amet\nIPSUM",
    'Lorem', 'dir/a', '.git', 'x/.foo/y', '//', 'a//b', 'a/b', 'z', '|', 'a.b.c', 'a*b'];
$pcre = [];
foreach ($patterns as $p) {
    $row = [];
    foreach ($subjects as $s) {
        $r = @preg_match($p, $s);
        $row[] = false === $r ? null : $r;
    }
    $pcre[] = [$b($p), $row];
}

// --- comparisons ---------------------------------------------------------
$scalars = [0, 1, -1, 500, 999, 1000, 1001, 1024, 1.5, 1000.0, '0', '1', '1000', '1000.0', '1e3', ' 1000', '1000 ',
    '500', '999', '1001', '1500', 'abc', '', '10', '9', 'a1000', '1000a', '0x10', '00', '0.0', '-1', '+1', ' ', '1.'];
$compare = [];
foreach ($scalars as $x) {
    foreach ($scalars as $y) {
        $compare[] = [$x <=> $y, $x == $y];
    }
}
$enc = static fn ($v) => \is_string($v) ? ['s', $v] : (\is_int($v) ? ['i', $v] : ['f', $v]);

// --- natural and case-insensitive order ----------------------------------
$words = ['', ' ', 'a', 'A', 'b', 'B', 'a1', 'a2', 'a10', 'a01', 'a001', 'a1b', 'a 1', 'a  2', '0', '00', '01', '1',
    '10', '010', '1.5', '1.10', 'x0.5', 'x0.05', 'qux_0_1.php', 'qux_10_2.php', 'qux_2_0.php', 'qux_1000_1.php',
    'qux_1002_0.php', 'qux_12_0.php', 'foo bar', 'foo/bar.tmp', 'foo', 'Zephire.php', 'zebulon.php', 'img12', 'img10',
    'IMG12', '  lead', 'lead', '0lead', '007', '7', 'a0', 'a00', 'a0b', '_', '[', 'é', 'É',
    // whitespace right after a run of digits is compared, not skipped
    '2 B', '2-', '02-', '2 a', '2a', '2  x', '2 x', 'x2 B', 'x2-'];
$nat = [];
foreach ($words as $x) {
    foreach ($words as $y) {
        $nat[] = [strnatcmp($x, $y) <=> 0, strnatcasecmp($x, $y) <=> 0, strcasecmp($x, $y) <=> 0];
    }
}

// --- NumberComparator ----------------------------------------------------
$numbers = ['1', '0', '3.5', '33.55', '123.456', '123456.78', '.1', '.123', '.0', '0.0', '1.', '0.', '123.',
    '==1', '!=1', '<1', '>1', '<=1', '>=1', '==1k', '==1ki', '==1m', '==1mi', '==1g', '==1gi', '1k', '1ki', '1m',
    '1mi', '1g', '1gi', '', ' ', 'foobar', '=1', '===1', '0 . 1', '123 .45', '234. 567', '..', '.0.', '0.1.2',
    '< 1K', '<1k', '  < 1 K ', '> 0.5K', '1.5k', '0.1k', '0.001k', '1.5Gi', '9223372036854775807', '9223372036854775807k',
    '99999999999999999999', '1e3', '1KI', '1kI', "1\n", "\n1", '1 k i', '00010', '0.5', '.', '1.2.3k'];
$numberComparator = [];
foreach ($numbers as $n) {
    try {
        $c = new NumberComparator($n);
        $numberComparator[] = [$n, $c->getTarget(), $c->getOperator()];
    } catch (\Exception $e) {
        $numberComparator[] = [$n, null, \get_class($e) . ': ' . $e->getMessage()];
    }
}

// --- dates (strtotime at a fixed time) -----------------------------------
$now = 1700000000; // 2023-11-14 22:13:20 UTC
$dateStrings = ['2005-10-10', '2005-10-15', '2005/10/15', '2005-10-10 12:30', '2005-10-10 12:30:45', '2005-10-10T12:30:45',
    'now', 'today', 'midnight', 'noon', 'yesterday', 'tomorrow', '20 years ago', '2 months ago', 'last month', 'next month',
    'last year', '-1 year', '+1 day', '1 day ago', '- 2 hours', 'now - 2 hours', '+2 weeks', '1 fortnight ago', 'this month',
    '3 days', '2days', '10:00', '@1234567890', '@-5', 'NOW', 'Yesterday', '2 Months Ago', '1 hour 30 minutes ago',
    '2024-01-31 +1 month', '2024-02-29 +1 year', '2005-10-10 -1 day'];
$dates = [];
foreach ($dateStrings as $d) {
    $dates[] = [$d, strtotime($d, $now)];
}

// --- Glob::toRegex -------------------------------------------------------
$globs = ['*', '*.php', 'test.ph*', 'test.p{hp,y}', '{*.php,*.py}', '**/*.txt', '/**/*.neon', '/Fixtures/one/**', 'foo/**/bar',
    '**', 'a\**', '.*', '#', 'a[*?]', '[a\-c]', 'file[0-9].txt', '[[:alpha:]]', '[[:alpha:]', 'file\[1\].txt', 'a\[b',
    '[a\]]', 'a[b', '[]', '[a\]', '[a[b', '[*.txt', '[]]', '[a[]', 'a{b', '{a,b', '{a,{b,c}', '{a{b,c}', '{{a}', '{*',
    '{a,b}}', 'a,b', '[{]', '[{}]', '{a,[{]}', '{[a{]a}', '{a,[,]}', '{a,[b}', '[{-}]', 'a?c', '\*', '\\\\', 'a/b/c',
    '[!a]', '[^a]', '[a/b]', '[a\/b]', 'x(y)|z+^$', 'a.b', 'é*', '**/', 'a/**', '/**', '**/**', '**a', 'a**b', 'a/**/',
    '{a,b}{c,d}', '\{a,b}', '{a\,b}', '[\\\\]', '[a\\', 'a\\'];
$glob = [];
foreach ($globs as $g) {
    foreach ([[true, true, '#'], [false, true, '#'], [true, false, '#'], [false, false, '#'], [true, true, '/'], [true, true, '']] as [$dot, $slash, $delim]) {
        $glob[] = [$b($g), $dot, $slash, $delim, $b(Glob::toRegex($g, $dot, $slash, $delim))];
    }
}

// --- Gitignore::toRegex on extra inputs ----------------------------------
$gitignore = [];
foreach (["a\\", "\\", "[", "[]", "[!]", "[^a]", "[!a-c]", "[\\x-\\z]", "[z-a]", "[a-]", "[-a]", "a**", "**a", "a/**/b",
    "**/", "/**", "\\#", "\\!", "a\\ ", "a\\  ", "a\\\\ ", "*.txt\r", "x[*]", "x[?]", "é", "[é]", "a/", "/", "//", "!",
    "!!", " ", "\t#", "a\\b", "\\x41", "[\\x41-\\x43]", "x[a\\]b]"] as $line) {
    $gitignore[] = [$b($line), $b(Gitignore::toRegex($line)), $b(Gitignore::toRegexMatchingNegatedPatterns($line))];
}

$out = [
    'subjects' => array_map($b, $subjects),
    'pcre' => $pcre,
    'scalars' => array_map($enc, $scalars),
    'compare' => $compare,
    'words' => array_map($b, $words),
    'natural' => $nat,
    'number_comparator' => $numberComparator,
    'now' => $now,
    'dates' => $dates,
    'glob' => $glob,
    'gitignore' => $gitignore,
];
$lines = [];
foreach ($out as $k => $v) {
    $lines[] = '  ' . json_encode($k) . ': ' . json_encode($v, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION);
}
echo "{\n", implode(",\n", $lines), "\n}\n";
