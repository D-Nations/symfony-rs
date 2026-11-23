<?php
// parse_str() for crates/php-value/tests/data/parse_str.json
// (scripts/record.sh php-value). Inputs: every raw string of the request
// vectors (crates/http-foundation/tests/data/query_vectors.json) and the
// cases below. Each case is [input (base64), typed result]; warnings are
// not recorded.

error_reporting(E_ALL & ~E_WARNING & ~E_DEPRECATED);

require __DIR__ . '/lib/typed.php';

$inputs = [];
foreach (json_decode(file_get_contents(__DIR__ . '/../../crates/http-foundation/tests/data/query_vectors.json'), true) as $vector) {
    $inputs[] = base64_decode($vector['raw']);
}

$inputs = array_merge($inputs, [
    '', '&', '&&', '=', '==', 'a', 'a=', '=a', 'a==b', 'a=b=c', '+a=1', '%20a=1', ' a=1', 'a =1', 'a%00b=1',
    "a\0b=1", 'a=%00', 'a.b=1', 'a b=1', 'a+b=1', 'a[=1', 'a]=1', 'a[b=1', 'a[b]c=1', 'a[b]c[d]=1', 'a[b][=1',
    'a[]=1&a[]=2', 'a[1]=x&a[]=y', 'a[-5]=x&a[]=y', 'a[9223372036854775807]=x&a[]=y', 'a[9223372036854775807]=x&a[]=y&a[]=z',
    'a[01]=x', 'a[-0]=x', 'a[+1]=x', 'a[ ]=x', 'a[  ]=x', "a[\t]=x", 'a[ 1]=x', 'a=1&a[]=2', 'a[]=1&a=2',
    'a[b]=1&a[b][c]=2', 'a[b][c]=1&a[b]=2', '1=a', '-1=a', '01=a', '9223372036854775808=a', '[a]=1',
    'a%5Bb%5D=1', 'a[%5D=1', 'a%26b=1', 'a=%26', 'a=%zz', 'a=%4', 'a=%', 'a=+', 'a=%2B', "a=\xff", "\xff=1",
    "a[\xff]=1", "x=1&a\0b=1&y=2", "x=1\0&y=2", "\0a=1", "x=1&\0&y=2", 'a.b[c.d]=1', 'a b[c d]=1', 'a[b.c]=1', 'a;b=1', 'a=1;b=2',
    // HeaderUtils::parseQuery()'s encoded names (hex, then the bracket part rawurlencoded)
    '61=b&63=d', '612e62=c', '61%5Bb=c', '61%5Bb%5D=c', '61%5Bb%5D%5Bc.d%5D=c', '66%5B%2525%5D%5B%26%5D%5B%3D%5D%5Bp.c%5D=d',
]);

// Nesting: max_input_nesting_level (64) and around it
foreach ([63, 64, 65, 100] as $depth) {
    $inputs[] = 'a' . str_repeat('[b]', $depth) . '=1&x=2';
    $inputs[] = 'a' . str_repeat('[]', $depth) . '=1&x=2';
}
// max_input_vars (1000) and around it, with and without empty pairs
foreach ([999, 1000, 1001, 1500] as $count) {
    $pairs = [];
    for ($i = 0; $i < $count; ++$i) {
        $pairs[] = "v$i=$i";
    }
    $inputs[] = implode('&', $pairs);
    $inputs[] = implode('&&', $pairs);
    $inputs[] = 'a[]=' . implode('&a[]=', range(1, $count));
}

$cases = [];
foreach ($inputs as $input) {
    $result = null;
    parse_str($input, $result);
    $cases[] = [base64_encode($input), typed_value($result)];
}

echo json_encode($cases, JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR), "\n";
