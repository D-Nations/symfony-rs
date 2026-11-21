<?php

// What symfony-console's Command, StringInput and Application port need
// from PHP beyond their upstream tests (scripts/record.sh console):
//
// - stripcslashes() and StringInput's tokens on generated strings;
// - levenshtein(), strnatcasecmp() and the default `<=>` of two strings
//   (sort()), which Application::find() and findAlternatives() use;
// - OutputWrapper::wrap() on generated text;
// - Application::find() and findNamespace() on a fixed set of commands
//   for generated names: the command found, or the exception's class,
//   message and alternatives.
//
// Byte strings that may not be UTF-8 are written as hex. Deterministic:
// a fixed seed.
//
//   php console_builtins.php

require '/demo/vendor/autoload.php';

use Symfony\Component\Console\Application;
use Symfony\Component\Console\Command\Command;
use Symfony\Component\Console\Exception\CommandNotFoundException;
use Symfony\Component\Console\Input\ArgvInput;
use Symfony\Component\Console\Input\StringInput;

mt_srand(20261121);

$random = static function (array $alphabet, int $min, int $max): string {
    $s = '';
    for ($i = 0, $n = mt_rand($min, $max); $i < $n; ++$i) {
        $s .= $alphabet[mt_rand(0, count($alphabet) - 1)];
    }

    return $s;
};

$out = [];

$alphabet = ['\\', '\\', 'n', 't', 'r', 'a', 'b', 'f', 'v', 'x', 'X', '0', '1', '7', '8', '9', 'F', 'g', 'e', '"', "'", ' '];
foreach (['', '\\', '\\\\', '\\x', '\\x4', '\\x41', '\\x414', '\\0', '\\08', '\\101', '\\1011', '\\400', '\\777', '\\8', 'a\\'] as $s) {
    $out['stripcslashes'][] = [$s, bin2hex(stripcslashes($s))];
}
for ($n = 0; $n < 3000; ++$n) {
    $s = $random($alphabet, 1, 8);
    $out['stripcslashes'][] = [$s, bin2hex(stripcslashes($s))];
}

$tokens = new ReflectionProperty(ArgvInput::class, 'tokens');
$alphabet = [' ', ' ', 'a', 'b', '-', '=', '"', '"', "'", "'", '\\', "\t", "\n", "\r", 'x', '0', 'n'];
for ($n = 0; $n < 4000; ++$n) {
    $s = $random($alphabet, 1, 12);
    try {
        $input = new StringInput($s);
        $out['string_input'][] = [$s, array_map('bin2hex', $tokens->getValue($input))];
    } catch (\Throwable $e) {
        $out['string_input'][] = [$s, [$e::class, $e->getMessage()]];
    }
}

$alphabet = ['a', 'b', 'c', ':', 'f', 'o'];
for ($n = 0; $n < 2000; ++$n) {
    $a = $random($alphabet, 0, 7);
    $b = $random($alphabet, 0, 7);
    $out['levenshtein'][] = [$a, $b, levenshtein($a, $b)];
}

$alphabet = ['a', 'A', 'b', 'B', '0', '0', '1', '2', '9', ' ', ':', '-', 'x', '.'];
for ($n = 0; $n < 3000; ++$n) {
    $a = $random($alphabet, 0, 6);
    $b = $random($alphabet, 0, 6);
    $out['strnatcasecmp'][] = [$a, $b, max(-1, min(1, strnatcasecmp($a, $b)))];
}

$alphabet = ['1', '2', '0', '.', 'e', ' ', '-', '+', 'a', ':'];
for ($n = 0; $n < 3000; ++$n) {
    $a = $random($alphabet, 0, 5);
    $b = $random($alphabet, 0, 5);
    $out['compare'][] = [$a, $b, $a <=> $b];
}

// OutputWrapper::wrap() on generated text: words, spaces, line breaks,
// style tags, URLs and multi-byte characters, at small widths.
$words = ['a', 'Lorem', 'ipsum', 'consectetur', 'Árvíztűrő', '表示する', '🎉', '<info>', '</info>', '<fg=red;bg=blue>', '</>', '<comment>x</comment>',
    'https://example.com/a/long/path', 'http://x.y', '\\<escaped\\>', '<a\\>b>', 'x\\', '-'];
$separators = [' ', ' ', ' ', '  ', "\n", "\r\n", "\t", ''];
for ($n = 0; $n < 2500; ++$n) {
    $text = '';
    for ($i = 0, $count = mt_rand(0, 10); $i < $count; ++$i) {
        $text .= $words[mt_rand(0, count($words) - 1)].$separators[mt_rand(0, count($separators) - 1)];
    }
    $width = mt_rand(-2, 24);
    $cut = 0 === mt_rand(0, 2);
    $out['wrap'][] = [$text, $width, $cut, (new Symfony\Component\Console\Helper\OutputWrapper($cut))->wrap($text, $width)];
}

// Application::find() on a fixed set of commands.
putenv('COLUMNS=120');
$commands = [
    ['foo:bar', ['afoobar'], 'The foo:bar command', false],
    ['foo:bar1', ['afoobar1'], 'The foo:bar1 command', false],
    ['foo1:bar', ['afoobar2'], 'The foo1:bar command', false],
    ['foo3:bar', [], 'The foo3:bar command', false],
    ['foo3:bar:toh', [], '', false],
    ['foo:hidden', ['afoohidden'], '', true],
    ['foo:BAR', [], 'foo:BAR command', false],
    ['bar:buc', [], '', false],
    ['test-ambiguous', ['test'], 'The test-ambiguous command', false],
    ['test-ambiguous2', [], 'The test-ambiguous2 command', false],
    ['aliased', ['a', 'alias', 'alias2'], 'Aliased command', false],
    ['cache:clear', ['cc'], 'Clear the cache with a description long enough to be cut at the usable width of the terminal, which is 110 columns', false],
    ['cache:pool:clear', [], 'Clear pools', false],
    ['10', [], 'A numeric name', false],
    ['9:x', [], '', false],
];
$out['commands'] = $commands;
$application = new Application();
foreach ($commands as [$name, $aliases, $description, $hidden]) {
    $application->addCommand((new Command($name))->setAliases($aliases)->setDescription($description)->setHidden($hidden));
}
$parts = ['', 'f', 'fo', 'foo', 'Foo', 'foo1', 'foo3', 'fooo', 'b', 'ba', 'bar', 'BAR', 'bar1', 'baR', 'a', 'afoo', 'al', 't', 'te', 'test', 'h', 'l', 'c', 'ca', 'cache', 'p', 'x', '1', '9', 'hid', 'toh'];
$names = ['help', 'list', '_complete', 'completion'];
foreach ($parts as $a) {
    $names[] = $a;
    foreach ($parts as $b) {
        $names[] = "$a:$b";
    }
}
for ($n = 0; $n < 400; ++$n) {
    $names[] = $parts[mt_rand(0, count($parts) - 1)].':'.$parts[mt_rand(0, count($parts) - 1)].':'.$parts[mt_rand(0, count($parts) - 1)];
}
$names = array_values(array_unique($names));
$outcome = static function (callable $f) {
    try {
        $result = $f();

        return ['ok', $result instanceof Command ? $result->getName() : $result];
    } catch (CommandNotFoundException $e) {
        return [$e::class, $e->getMessage(), $e->getAlternatives()];
    }
};
foreach ($names as $name) {
    $out['find'][] = [$name, $outcome(static fn () => $application->find($name))];
    $out['find_namespace'][] = [$name, $outcome(static fn () => $application->findNamespace($name))];
}

echo json_encode($out, JSON_THROW_ON_ERROR), "\n";
