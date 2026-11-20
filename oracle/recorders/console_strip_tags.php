<?php

// PHP's strip_tags($string), which Console's Output::write() applies for
// OUTPUT_PLAIN, on generated strings, so the Rust port can be written from
// the manual and checked here (scripts/record.sh console). Deterministic:
// a fixed seed.
//
// Output JSON: [[input, strip_tags(input)], ...].

mt_srand(20261120);
$cases = [];
foreach (['', 'foo', '<b>foo</b>', '<info>foo</info>', 'a < b', 'a<b', '<a href="x>y">z</a>', "<a title='>'>b",
    '<!-- c -->d', '<?php echo 1; ?>e', '<!DOCTYPE html>f', 'x<', '<>', '< >', 'a <3 b', '<<a>>', "<a\n>b"] as $s) {
    $cases[] = [$s, strip_tags($s)];
}
// A PHP section nested in a tag, after various tag openings.
foreach (['<', '<a', '<a ', '<<', '<a<'] as $prefix) {
    foreach (['<?>', '<??>', '<?>?>', '<?a?>', "<?'?>'?>", '<?>>', '<?a>b?>', '<?'] as $php) {
        foreach (['>x', 'x', '>>x'] as $suffix) {
            $s = $prefix.$php.$suffix;
            $cases[] = [$s, strip_tags($s)];
        }
    }
}
$alphabet = ['a', 'b', ' ', '<', '<', '>', '>', '"', "'", '!', '-', '?', '/', "\n", '=', 'p'];
for ($n = 0; $n < 6000; ++$n) {
    $length = mt_rand(1, 16);
    $s = '';
    for ($i = 0; $i < $length; ++$i) {
        $s .= $alphabet[mt_rand(0, count($alphabet) - 1)];
    }
    $cases[] = [$s, strip_tags($s)];
}
echo json_encode($cases, JSON_THROW_ON_ERROR), "\n";
