<?php

// lint:yaml's exact output and exit code (scripts/record.sh yaml), for
// what LintCommandTest checks only in part: each YAML content in each
// format (txt, verbose txt, json, github), several files, --exclude,
// --parse-tags and a directory with one YAML file and one other file.
// Files are written under the relative directory ../../target/yl, which
// the Rust replay uses too (its tests run in crates/yaml), so paths and
// the padding SymfonyStyle adds after them are the same.
// crates/yaml/tests/lint_command_vectors.rs replays the cases.
//
//   php yaml_lint.php

require '/demo/vendor/autoload.php';

use Symfony\Component\Console\Application;
use Symfony\Component\Console\Output\OutputInterface;
use Symfony\Component\Console\Tester\CommandTester;
use Symfony\Component\Yaml\Command\LintCommand;

$contents = [
    'valid' => "foo: bar\n",
    'empty' => '',
    'incorrect' => "\nfoo:\nbar",
    'tabs' => "foo:\n\tbar: baz\n",
    'custom_tag' => "foo: !my_tag {foo: bar}\n",
    'duplicate_key' => "foo: 1\nfoo: 2\n",
    'unterminated' => "foo: \"bar\n",
    'unicode' => "clé: [é, ü\n",
    'unknown_constant' => "foo: !php/const NOT_A_CONSTANT\n",
    'slash_path' => "a/b: {c: d\n",
    'sequence_in_mapping' => "foo: bar\n- baz\n",
];
@mkdir(sys_get_temp_dir().'/a/b', 0777, true);
chdir(sys_get_temp_dir().'/a/b');
$base = '../../target/yl';
@mkdir($base, 0777, true);
foreach ($contents as $name => $content) {
    file_put_contents("$base/$name.yaml", $content);
}
@mkdir("$base/dir");
file_put_contents("$base/dir/only.yml", "a: b\n");
file_put_contents("$base/dir/skipped.txt", "not: [yaml\n");

$run = static function (array $input, int $verbosity) use ($base): array {
    $application = new Application();
    $application->addCommand(new LintCommand());
    $tester = new CommandTester($application->find('lint:yaml'));
    try {
        $status = $tester->execute($input, ['verbosity' => $verbosity, 'decorated' => false]);
        $result = ['status' => $status, 'display' => $tester->getDisplay(true)];
    } catch (\Throwable $e) {
        $result = ['throws' => $e::class, 'message' => $e->getMessage()];
    }

    return $result;
};

putenv('GITHUB_ACTIONS');
putenv('COLUMNS=80'); // the width SymfonyStyle pads to; the replay sets it too
$cases = [];
foreach (array_keys($contents) as $name) {
    foreach (['txt', 'json', 'github'] as $format) {
        foreach ([OutputInterface::VERBOSITY_NORMAL, OutputInterface::VERBOSITY_VERBOSE] as $verbosity) {
            foreach ([false, true] as $parseTags) {
                $input = ['filename' => "BASE/$name.yaml", '--format' => $format];
                if ($parseTags) {
                    $input['--parse-tags'] = true;
                }
                $cases[] = ['input' => $input, 'verbosity' => $verbosity] + $run(str_replace('BASE', $base, $input), $verbosity);
            }
        }
    }
}
$more = [
    ['filename' => ['BASE/valid.yaml', 'BASE/incorrect.yaml', 'BASE/tabs.yaml']],
    ['filename' => ['BASE/valid.yaml', 'BASE/incorrect.yaml'], '--format' => 'json'],
    ['filename' => ['BASE/valid.yaml', 'BASE/incorrect.yaml'], '--exclude' => ['BASE/incorrect.yaml']],
    ['filename' => ['BASE/dir']],
    ['filename' => ['BASE/dir/'], '--format' => 'json'],
    ['filename' => ['BASE/missing.yaml']],
    ['filename' => []],
    ['filename' => ['BASE/valid.yaml'], '--format' => 'xml'],
    ['filename' => ['BASE/custom_tag.yaml', 'BASE/valid.yaml'], '--no-parse-tags' => true],
];
foreach ($more as $input) {
    foreach ([OutputInterface::VERBOSITY_NORMAL, OutputInterface::VERBOSITY_VERBOSE] as $verbosity) {
        $replaced = json_decode(str_replace('BASE', $base, json_encode($input, JSON_UNESCAPED_SLASHES)), true);
        $cases[] = ['input' => $input, 'verbosity' => $verbosity] + $run($replaced, $verbosity);
    }
}

echo json_encode($cases, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
