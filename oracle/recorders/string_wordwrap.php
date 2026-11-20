<?php

// PHP's native wordwrap() on the masks symfony/string's
// AbstractString::wordwrap() builds (' ' for a space, '?' for any other
// grapheme, '#' for an existing break, break string '#'), so the Rust
// port's wordwrap can be written from the manual and checked here
// (scripts/record.sh string). Deterministic: a fixed seed.
//
// Output JSON: [[mask, width, cut, result], ...].

mt_srand(20261120);
$cases = [];
$alphabet = [' ', '?', '?', '?', '#'];
foreach ([['', 3, false], ['', 3, true], ['#', 2, false], ['   ', 1, true], ['????', 0, false]] as [$mask, $width, $cut]) {
    $cases[] = [$mask, $width, $cut, wordwrap($mask, $width, '#', $cut)];
}
for ($n = 0; $n < 4000; ++$n) {
    $length = mt_rand(1, 24);
    $mask = '';
    for ($i = 0; $i < $length; ++$i) {
        $mask .= $alphabet[mt_rand(0, 4)];
    }
    $width = mt_rand(1, 8);
    $cut = 1 === mt_rand(0, 1);
    $cases[] = [$mask, $width, $cut, wordwrap($mask, $width, '#', $cut)];
}
echo json_encode($cases, JSON_THROW_ON_ERROR), "\n";
