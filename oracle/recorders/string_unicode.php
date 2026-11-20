<?php

// Unicode behaviour symfony/string relies on, from the oracle's own ICU
// (intl: normalizer_*, grapheme_*) and PCRE2 (\X), so the crates the Rust
// port adopts (unicode-normalization, unicode-segmentation) can be checked
// against it (scripts/record.sh string).
//
// Output JSON:
// - versions: ICU, Unicode (ICU's), PCRE2;
// - extpict: code point ranges matching PCRE2's \p{ExtPict} (wcswidth's
//   EMOJI_REGEX);
// - nfc: [cp, NFC(NFD(char)), NFC(char)] for every code point whose NFD
//   differs from itself (decomposition and recomposition of each);
// - extend / prepend: code point ranges [first, last] where "a".char
//   (resp. char."a") is one grapheme, by ICU and by PCRE2;
// - sequences: strings built from representatives of each
//   Grapheme_Cluster_Break value (pairs and triples, plus emoji, flag and
//   Indic conjunct sequences), with their split by ICU and by PCRE2.

$out = [
    'versions' => [
        'icu' => INTL_ICU_VERSION,
        'unicode' => implode('.', array_slice(IntlChar::getUnicodeVersion(), 0, 3)),
        'pcre' => PCRE_VERSION,
    ],
];

function ch(int $cp): string
{
    return IntlChar::chr($cp);
}

function icu_split(string $s): array
{
    return grapheme_str_split($s);
}

function pcre_split(string $s): array
{
    preg_match_all('/\X/u', $s, $m);

    return $m[0];
}

function add_range(array &$ranges, int $cp): void
{
    $last = array_key_last($ranges);
    if (null !== $last && $ranges[$last][1] === $cp - 1) {
        $ranges[$last][1] = $cp;
    } else {
        $ranges[] = [$cp, $cp];
    }
}

$nfc = [];
$extpict = [];
$extend = ['icu' => [], 'pcre' => []];
$prepend = ['icu' => [], 'pcre' => []];
$reps = [];
for ($cp = 0; $cp <= 0x10FFFF; ++$cp) {
    if ($cp >= 0xD800 && $cp <= 0xDFFF) {
        continue;
    }
    $c = ch($cp);
    $d = normalizer_normalize($c, Normalizer::NFD);
    if ($d !== $c) {
        $nfc[] = [$cp, normalizer_normalize($d, Normalizer::NFC), normalizer_normalize($c, Normalizer::NFC)];
    }
    if (1 === count(icu_split('a'.$c))) {
        add_range($extend['icu'], $cp);
    }
    if (1 === count(pcre_split('a'.$c))) {
        add_range($extend['pcre'], $cp);
    }
    if (1 === count(icu_split($c.'a'))) {
        add_range($prepend['icu'], $cp);
    }
    if (1 === count(pcre_split($c.'a'))) {
        add_range($prepend['pcre'], $cp);
    }
    if (preg_match('/\p{ExtPict}/u', $c)) {
        add_range($extpict, $cp);
    }
    $gcb = IntlChar::getIntPropertyValue($cp, IntlChar::PROPERTY_GRAPHEME_CLUSTER_BREAK);
    $key = $gcb.':'.preg_match('/\p{Extended_Pictographic}/u', $c);
    if (count($reps[$key] ?? []) < 2 && IntlChar::isdefined($cp)) {
        $reps[$key][] = $cp;
    }
}
$out['nfc'] = $nfc;
$out['extpict'] = $extpict;
$out['extend'] = $extend;
$out['prepend'] = $prepend;

$singles = [];
foreach ($reps as $cps) {
    foreach ($cps as $cp) {
        $singles[] = ch($cp);
    }
}
$singles = array_merge($singles, ["\r", "\n", "\u{200D}", "\u{FE0F}", "\u{1F3FB}", "\u{0915}", "\u{094D}", "\u{0937}"]);
$strings = [];
foreach ($singles as $a) {
    foreach ($singles as $b) {
        $strings[] = $a.$b;
        foreach (["\u{200D}", "\u{0301}", "\u{094D}"] as $c) {
            $strings[] = $a.$c.$b;
        }
    }
}
$strings = array_merge($strings, [
    "\u{1F469}\u{200D}\u{1F33E}", "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
    "\u{1F1EB}\u{1F1F7}\u{1F1E9}\u{1F1EA}\u{1F1EB}", "\u{0915}\u{094D}\u{0937}\u{093F}",
    "\u{0915}\u{200D}\u{094D}\u{0937}", "\u{1100}\u{1161}\u{11A8}\u{AC00}\u{11A8}",
    "e\u{0301}\u{0302}x", "\u{1F44D}\u{1F3FD}\u{FE0F}", "#\u{FE0F}\u{20E3}",
]);
$sequences = [];
foreach (array_unique($strings) as $s) {
    $icu = icu_split($s);
    $pcre = pcre_split($s);
    $sequences[] = [$s, array_map('strlen', $icu), array_map('strlen', $pcre)];
}
$out['sequences'] = $sequences;

echo json_encode($out, JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
