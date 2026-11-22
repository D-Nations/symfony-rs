<?php
// symfony/string's case mappings and normalizations as PHP computes them
// for single code points (scripts/record.sh string), from which
// scripts/gen-string-tables.py builds crates/string/src/case_tables.rs:
// mb_strtolower(), mb_strtoupper(), mb_convert_case() with MB_CASE_TITLE,
// MB_CASE_FOLD and MB_CASE_FOLD_SIMPLE (mbstring's own Unicode data), and
// Normalizer::NFKC_CF (ICU). One line per code point whose answer differs
// from the code point itself in any column, as hex UTF-8; then, for the
// final sigma rule of mb_strtolower(), the ranges of code points mbstring
// treats as cased ("cΣ" lowers to "cς") and as case-ignorable ("AΣca"
// keeps σ though c is not cased); then the versions the answers come from.

$columns = [
    static fn (string $c): string => mb_strtolower($c, 'UTF-8'),
    static fn (string $c): string => mb_strtoupper($c, 'UTF-8'),
    static fn (string $c): string => mb_convert_case($c, \MB_CASE_TITLE, 'UTF-8'),
    static fn (string $c): string => mb_convert_case($c, \MB_CASE_FOLD, 'UTF-8'),
    static fn (string $c): string => mb_convert_case($c, \MB_CASE_FOLD_SIMPLE, 'UTF-8'),
    static fn (string $c): string => (string) normalizer_normalize($c, \Normalizer::NFKC_CF),
];

$sigma = "\u{3C2}";
$flags = ['cased' => [], 'case_ignorable' => []];

echo "# cp\tlower\tupper\ttitle\tfold\tfold_simple\tnfkc_cf (hex UTF-8)\n";
for ($cp = 0; $cp <= 0x10FFFF; ++$cp) {
    if ($cp >= 0xD800 && $cp <= 0xDFFF) {
        continue;
    }
    $c = mb_chr($cp, 'UTF-8');
    $row = [];
    $differs = false;
    foreach ($columns as $f) {
        $v = $f($c);
        $differs = $differs || $v !== $c;
        $row[] = bin2hex($v);
    }
    if ($differs) {
        echo sprintf('%X', $cp), "\t", implode("\t", $row), "\n";
    }

    $cased = str_ends_with(mb_strtolower($c."\u{3A3}", 'UTF-8'), $sigma);
    $ignorable = !$cased && !str_contains(mb_strtolower("A\u{3A3}".$c.'a', 'UTF-8'), $sigma);
    foreach (['cased' => $cased, 'case_ignorable' => $ignorable] as $name => $set) {
        if (!$set) {
            continue;
        }
        $last = array_key_last($flags[$name]);
        if (null !== $last && ($flags[$name][$last][1] === $cp - 1 || (0xD7FF === $flags[$name][$last][1] && 0xE000 === $cp))) {
            $flags[$name][$last][1] = $cp;
        } else {
            $flags[$name][] = [$cp, $cp];
        }
    }
}
foreach ($flags as $name => $ranges) {
    foreach ($ranges as [$from, $to]) {
        echo $name, "\t", sprintf('%X', $from), "\t", sprintf('%X', $to), "\n";
    }
}
echo '# PHP ', \PHP_VERSION, ', ICU ', \INTL_ICU_VERSION, ', ICU Unicode ', IntlChar::getUnicodeVersion()[0], '.', IntlChar::getUnicodeVersion()[1], ', PCRE ', \PCRE_VERSION, "\n";
