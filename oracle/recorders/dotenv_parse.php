<?php
// Dotenv::parse() on generated .env inputs (scripts/record.sh dotenv): the
// values PHP returns, or the exception's class and message. The inputs
// combine the pieces the lexer and the resolvers treat specially (quotes,
// escapes, $ forms, comments, blanks, export, line breaks); no $(...)
// commands, so the vectors hold on every platform. $_ENV, $_SERVER and
// getenv() hold a few variables the inputs refer to.

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Dotenv\Dotenv;

putenv('GETENV_ONLY=from getenv');
$_ENV['IN_ENV'] = 'from $_ENV';
$_SERVER['IN_SERVER'] = 'from \\ server';
$_SERVER['HTTP_HOST'] = 'example.com';
unset($_ENV['SYMFONY_DOTENV_VARS'], $_SERVER['SYMFONY_DOTENV_VARS']);

$values = [
    '', 'bar', 'b a r', '"b a r"', "'b a r'", '"a\\"b"', "'a\\'b'", 'a\\"b', "a\\'b",
    '"a\\nb\\rc"', "'a\\nb'", '"multi' . "\n" . 'line"', "'multi\nline'", 'a#b', 'a #b', '"a" #c', "'a'b'c'",
    '"a"\'b\'c', '$FOO', '${FOO}', '\\$FOO', '\\\\$FOO', '\\\\\\$FOO', '"$FOO"', '"\\$FOO"', "'\$FOO'",
    '${FOO:-d}', '${UNSET:-d}', '${UNSET:=d}', '${UNSET:-}', '$UNSET:-d}', '${FOO', '$FOO}', '${}',
    '$', '$1', '$_', '${UNSET:-a b}', '${UNSET:-"x"}', '${UNSET:-$x}', '$IN_ENV', '$IN_SERVER',
    '$GETENV_ONLY', '$HTTP_HOST', '"${IN_ENV}x"', 'a\\\\b', '"a\\\\b"', "'a\\\\b'", '"c:\\\\"', 'x\\',
    '"', "'", '"a', "'a", '"a\\"', 'a b', ' x', "\t", '#', ' #c', '"\\$2y\\$10"', '\\$2y\\$10$x',
    '$FOO$FOO', '${FOO}${UNSET}${FOO}', '$(echo', '\\$(echo x)',
];
// Not '$(' at the very end of the data: upstream's lexNestedExpression()
// then reads past the end and loops forever (the port fails as for an
// unclosed parenthesis; crate README, "Differences"). "$(\n" is fine.
$names = ['FOO', 'export FOO', '_X', 'x1', '1X', 'FOO BAR', 'Стас', 'FOO!', '  FOO', 'export  ', 'exportFOO'];

$cases = [];
foreach ($values as $v) {
    $cases[] = "FOO=bar\nBAR=$v";
    $cases[] = "BAR=$v\n";
    $cases[] = "# c\nBAR=$v\r\nNEXT=1";
}
foreach ($names as $n) {
    $cases[] = "$n=1";
    $cases[] = $n;
}
foreach (['', "BAR=$(\n", "BAR=$(\nX=1", "\n", "\n\n# x\n", " \t\n", "FOO=", "FOO=\n\n", "\r\r", "FOO=1\rBAR=2", "#\nFOO=1 # x\n#"] as $c) {
    $cases[] = $c;
}

$out = [];
foreach ($cases as $data) {
    try {
        $result = typed_value((new Dotenv())->parse($data));
    } catch (\Throwable $e) {
        $result = ['throws', $e::class, base64_encode($e->getMessage())];
    }
    $out[] = ['data' => base64_encode($data), 'result' => $result];
}
echo json_encode($out, JSON_UNESCAPED_SLASHES | JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR), "\n";
