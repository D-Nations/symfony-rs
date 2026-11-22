<?php
// The PHP functions symfony/string's ByteString, CodePointString and
// UnicodeString call, as PHP answers them (scripts/record.sh string), for
// crates/string/tests/builtins.rs: the str*pos(), mb_*pos() and
// grapheme_*pos() families with every offset, mb_substr(),
// grapheme_substr(), grapheme_extract(), grapheme_str_split(), case
// mapping of whole strings (final sigma), str_pad(), ucwords(),
// substr_replace(), trim() with ranges, the normalizer forms, and the
// Windows-1250 / Windows-1252 code pages of mb_convert_encoding() and
// mb_detect_encoding(). Plain JSON: every string here is UTF-8; a result
// is a string, an int, false, an array, or {"throws": class}.

ini_set('memory_limit', '1G');

$haystacks = [
    '', 'a', 'abc', 'Symfony is awesome', 'SYMFONY is AWESOME', 'abcABCabc',
    'déjà-vu', "de\u{301}ja\u{300}-vu", 'Späßchen', "Spa\u{308}ßchen", 'नमस्ते', 'अनुच्छेद',
    "ab\u{200B}é", "ab\u{200B}e\u{301}", "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}x\u{1F468}",
    'ΣΑΣ ΣΑΣ.', 'ΌΣΟΣ Σ', 'Straße STRASSE', 'İstanbul ISTANBUL', 'ǅemal ǄEMAL', 'ﬁne FINE',
    '會意文字/会意文字', 'éÉéÉ', "a\u{301}\u{301}b",
];
$needles = ['', 'a', 'b', 'c', 'A', 'abc', 'is', 'IS', 'é', "e\u{301}", 'É', 'ß', 'SS', 'ss',
    'स्', 'स', "\u{200B}", 'e', "\u{1F468}", 'σ', 'Σ', 'ς', 'i', 'İ', 'ǅ', 'ǆ', 'fi', 'ﬁ', '文字', "\u{301}"];
$offsets = [-20, -5, -1, 0, 1, 3, 20];

$calls = [];
$call = static function (string $f, array $args) use (&$calls): void {
    try {
        $result = $f(...$args);
    } catch (\Throwable $e) {
        $result = ['throws' => $e::class];
    }
    $calls[] = [$f, $args, $result];
};

// $pos[$f][$hi][$ni]: one answer per offset (an int, false, or "throws")
$pos = [];
foreach ($haystacks as $hi => $h) {
    foreach (['strpos', 'stripos', 'strrpos', 'strripos', 'mb_strpos', 'mb_stripos', 'mb_strrpos', 'mb_strripos', 'grapheme_strpos', 'grapheme_stripos', 'grapheme_strrpos', 'grapheme_strripos'] as $f) {
        foreach ($needles as $ni => $n) {
            foreach ($offsets as $o) {
                try {
                    $pos[$f][$hi][$ni][] = '' === $n && str_starts_with($f, 'grapheme_') ? 'skipped' : $f($h, $n, $o);
                } catch (\Throwable) {
                    $pos[$f][$hi][$ni][] = 'throws';
                }
            }
        }
    }
    foreach ($offsets as $o) {
        foreach ([null, -3, -1, 0, 1, 2, 20] as $l) {
            $call('mb_substr', [$h, $o, $l]);
            $call('grapheme_substr', [$h, $o, $l]);
        }
    }
    foreach ([0, 1, 2, 3, 4, 7, 100] as $size) {
        foreach ([0, 1, 2, 3, 5] as $start) {
            $call('grapheme_extract', [$h, $size, \GRAPHEME_EXTR_MAXBYTES, $start]);
        }
    }
    if ('' !== $h) {
        $call('grapheme_str_split', [$h]);
    }
    $call('mb_strtolower', [$h]);
    $call('mb_strtoupper', [$h]);
    $call('ucwords', [$h]);
    foreach ([\Normalizer::NFC, \Normalizer::NFD, \Normalizer::NFKC, \Normalizer::NFKD, \Normalizer::NFKC_CF] as $form) {
        $call('normalizer_normalize', [$h, $form]);
        $call('normalizer_is_normalized', [$h, $form]);
    }
}
foreach (['', 'x', 'ab', 'abc'] as $pad) {
    foreach (['Sym', 'Symfony', ''] as $s) {
        foreach ([-1, 0, 3, 8, 11, 12] as $len) {
            foreach ([\STR_PAD_LEFT, \STR_PAD_RIGHT, \STR_PAD_BOTH] as $type) {
                $call('str_pad', [$s, $len, $pad, $type]);
            }
        }
    }
}
foreach (['Symfony is awesome', 'ab', ''] as $s) {
    foreach ([-30, -5, -1, 0, 1, 5, 30] as $start) {
        foreach ([null, -30, -3, 0, 2, 30] as $l) {
            $call('substr_replace', [$s, 'XY', $start, $l]);
            $call('substr', [$s, $start, $l]);
        }
    }
}
foreach (['  abc  ', 'xxabczz', "\0\tabc\x0B", 'a..zabcz..a', '..abc..', 'abc-def'] as $s) {
    foreach (['', ' ', 'a..c', 'x..z', 'z..a', '..', 'a..', '..z', ' .', "\0..\x20", 'a-c'] as $chars) {
        foreach (['trim', 'ltrim', 'rtrim'] as $f) {
            $call($f, [$s, $chars]);
        }
    }
}
foreach (['hello world', "a\tb\nc\rd\x0Ce\x0Bf", 'HELLO', '  x', 'émile ëlie'] as $s) {
    $call('ucwords', [$s]);
    $call('ucfirst', [$s]);
    $call('lcfirst', [$s]);
}

// Windows-1252 through mbstring; windows-1250, which mbstring lacks (a
// ValueError), through the iconv() fallback upstream takes
$encodings = [];
$detect = [];
for ($b = 0; $b < 256; ++$b) {
    $encodings['Windows-1252'][] = bin2hex(mb_convert_encoding(\chr($b), 'UTF-8', 'Windows-1252'));
    $detect['Windows-1252'][] = false !== mb_detect_encoding(\chr($b), 'Windows-1252', true);
    $u = @iconv('windows-1250', 'UTF-8', \chr($b));
    $encodings['windows-1250'][] = false === $u ? null : bin2hex($u);
}
// characters a code page cannot hold: mbstring substitutes '?', iconv fails
$unmappable = [
    'Windows-1252' => bin2hex(mb_convert_encoding('žsžsý ☃ 日本', 'Windows-1252', 'UTF-8')),
    'windows-1250' => var_export(@iconv('UTF-8', 'windows-1250', 'žsžsý ☃'), true),
    'windows-1250-mappable' => bin2hex(iconv('UTF-8', 'windows-1250', 'žsžsý')),
];

echo json_encode([
    'versions' => ['php' => \PHP_VERSION, 'icu' => \INTL_ICU_VERSION],
    'haystacks' => $haystacks,
    'needles' => $needles,
    'offsets' => $offsets,
    'pos' => $pos,
    'calls' => $calls,
    'encodings' => $encodings,
    'detect' => $detect,
    'unmappable' => $unmappable,
], \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR), "\n";
