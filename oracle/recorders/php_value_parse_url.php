<?php
// parse_url() answers for crates/php-value/tests/data/parse_url.json
// (scripts/record.sh php-value): hand-picked URLs (the RequestContext
// tests' and the shapes PHP's manual describes), then generated strings
// over the characters parse_url() treats specially, then generated
// scheme/authority/path/query/fragment combinations. Each case is the input
// (base64) and parse_url()'s typed result (false or the component array).

error_reporting(E_ALL);

require __DIR__ . '/lib/typed.php';

mt_srand(20261124);

$fixed = [
    '', '/', '//', '///', ':', '::', '?', '#', '@', 'a', '/a', '//a', '///a', 'a:', 'a:b', 'a:/b', 'a://b', 'a:1', 'a:12', 'a:123/b',
    'a:65535', 'a:65536', 'a:99999', 'a:-1', 'a:0', 'a:1?x', 'a:1#x', 'a:1b', '//a:1', '//a:', '//a:x', '//a:65536', '//a:0',
    'https://test.com:444/index.php', 'http://test.com:8080/', 'https://test.com', 'http://foo.com\\bar', '\\\\foo.com/bar',
    "a\rb", "a\nb", "a\tb", "\0foo", "foo\0", ' foo', 'foo ', 'http://user:pass@host:8080/p/a/t/h?query=string#fragment',
    'http://user@host', 'http://:pass@host', 'http://@host', 'http://user:@host', 'http://[::1]:80/x', 'http://[::1]/x', 'http://[::1',
    'http://host?q', 'http://host#f', 'http://host?q#f', 'http://host/p?q#f', '/p?q#f', '?q', '#f', 'mailto:a@b.c', 'file:///etc/passwd',
    'file://C:/x', 'c:\\x', 'C:/x', 'x:y:z', 'http:', 'http:/', 'http://', 'http:///x', 'http://a:b:c@d', 'http://a@b@c', 'http://a/b@c',
    'http://a?b@c', 'http://a#b@c', 'HTTP://A.B/C', 'h-t.t+p://x', '1http://x', '+http://x', 'http//x', 'http:x', 'http:80',
    'http:80/x', 'localhost:80', 'localhost:80/x', 'localhost:x', '//localhost:80', 'a:b@c', 'a:1@c', '//a@b:1', '//a@b:x',
    'http://h:00080/', 'http://h:+80/', 'http://h:8 0/', 'http://h: 80/', 'http://h:80 /', 'http://h%20x/', 'http://h/%zz',
    'http://h:/', 'http://h:?', 'http://h:#', '//:80', '//@', '//?', '//#', 'http://?', 'http://#', 'http://h/?#', '/x:1', 'x/y:1',
    './a:b', 'a/b:c', 'a?b:c', 'a#b:c', 'http://a.b:1/c:d', '//a/b:c', 'urn:isbn:0451450523', 'http:///', 'http:////x',
];
$alphabet = ['a', 'b', '1', '8', '0', ':', ':', '/', '/', '/', '?', '#', '@', '.', '[', ']', '\\', '%', ' ', '-', '+', 'http', 'https', '//', '://', 'é', "\x80", "\t", '65536', '80',
    "\x01", "\x0b", "\x1f", "\x7f", "\xff", '-1', '000080', 'user:pw@', '[::1]', 'file', 'FILE', ':///'];
// structured: scheme, authority, path, query and fragment, each maybe junk
$schemes = ['', 'http:', 'https:', 'file:', 'File:', 'a+b.c-d:', '1:', ':', 'é:'];
$users = ['', '', 'u@', 'u:p@', ':p@', '@', 'u:p:q@', 'a@b@', "u\x01@"];
$hosts = ['', 'h', 'example.com', 'EXAMPLE.com', '[::1]', '[::1', '::1]', '[]', 'h h', "h\x7f", '1.2.3.4', 'h%2f'];
$ports = ['', ':', ':0', ':80', ':65535', ':65536', ':000080', ':0000080', ':+80', ':-80', ': 80', ':80 ', ':8x', ':x', ':99999', ':-0', ":\t80", ':1:2'];
$paths = ['', '/', '/p', '/p:1', '/p/q', '//p', 'p', '/a@b', '/%41', "/\x00"];
$queries = ['', '?', '?q', '?a=b&c', '?:1', '??', '?#'];
$fragments = ['', '#', '#f', '#a:1', '##', '#?'];

$cases = [];
foreach ($fixed as $uri) {
    $cases[] = [base64_encode($uri), typed_value(parse_url($uri))];
}
for ($n = 0; $n < 6000; ++$n) {
    $uri = '';
    for ($i = mt_rand(1, 8); $i > 0; --$i) {
        $uri .= $alphabet[mt_rand(0, \count($alphabet) - 1)];
    }
    $cases[] = [base64_encode($uri), typed_value(parse_url($uri))];
}

for ($n = 0; $n < 6000; ++$n) {
    $uri = $schemes[mt_rand(0, \count($schemes) - 1)];
    if (mt_rand(0, 4)) {
        $uri .= '//';
    }
    $uri .= $users[mt_rand(0, \count($users) - 1)] . $hosts[mt_rand(0, \count($hosts) - 1)] . $ports[mt_rand(0, \count($ports) - 1)]
        . $paths[mt_rand(0, \count($paths) - 1)] . $queries[mt_rand(0, \count($queries) - 1)] . $fragments[mt_rand(0, \count($fragments) - 1)];
    $cases[] = [base64_encode($uri), typed_value(parse_url($uri))];
}

echo json_encode($cases, \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
