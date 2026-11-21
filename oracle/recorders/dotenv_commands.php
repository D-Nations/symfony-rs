<?php

// debug:dotenv's and dotenv:dump's exact output, and the .env.local.php
// dotenv:dump writes (scripts/record.sh dotenv), for what their tests
// check only in part. Each case starts from empty $_SERVER and $_ENV
// (the Rust replay's Globals::new()), writes its project's files, and
// for debug:dotenv may boot the environment first, as the tests do.
// COLUMNS=80, which the replay sets too. crates/dotenv/tests/
// command_vectors.rs replays the cases.
//
//   php dotenv_commands.php

require '/demo/vendor/autoload.php';

use Symfony\Component\Console\Application;
use Symfony\Component\Console\Helper\FormatterHelper;
use Symfony\Component\Console\Helper\HelperSet;
use Symfony\Component\Console\Tester\CommandTester;
use Symfony\Component\Dotenv\Command\DebugCommand;
use Symfony\Component\Dotenv\Command\DotenvDumpCommand;
use Symfony\Component\Dotenv\Dotenv;

putenv('COLUMNS=80');
$root = sys_get_temp_dir().'/dotenv-cases';

$projects = [
    'plain' => ['.env' => "FOO=bar\nTEST123=true\n", '.env.local' => "FOO=baz\n", '.env.prod.local' => "HELLO=world\n", '.env.test' => "TEST123=false\n"],
    'dist' => ['.env.dist' => "TEST=0000\n", '.env.local.php' => "<?php\nreturn [\n    'FOO' => 'BaR',\n    'TEST' => '1234',\n];\n", '.env.prod' => "FOO=BaR\nTEST=1234\n"],
    'both' => ['.env' => "FOO=BAR\n", '.env.dist' => "FOO=BAZ\n"],
    'values' => [
        '.env' => "LONG=".str_repeat('x', 40)."\nTAGS=\"<info>tag</info>\"\nQUOTE=\"it's \\\\ here\"\nEMPTY=\nZETA=1\nalpha=2\nUNI=été\n",
        '.env.dev' => "LONG=".str_repeat('y', 31)."\nNEW=1\n",
    ],
    'none' => [],
];

$cases = [];
$run = static function (callable $f): array {
    try {
        return $f();
    } catch (\Throwable $e) {
        return ['throws' => $e::class, 'message' => $e->getMessage()];
    }
};
$setUp = static function (string $name, array $files) use ($root): string {
    $dir = "$root/$name";
    if (is_dir($dir)) {
        foreach (scandir($dir) as $f) {
            if (is_file("$dir/$f")) {
                unlink("$dir/$f");
            }
        }
    } else {
        mkdir($dir, 0777, true);
    }
    foreach ($files as $file => $content) {
        file_put_contents("$dir/$file", $content);
    }

    return $dir;
};

// debug:dotenv
foreach ($projects as $name => $files) {
    foreach (['dev', 'test', 'prod'] as $env) {
        foreach ([null, 'fo', 'FOO', 'x', 'LONG'] as $filter) {
            foreach (['boot', 'vars_only', 'uninitialized'] as $setup) {
                $_SERVER = [];
                $_ENV = [];
                $dir = $setUp($name, $files);
                $result = $run(static function () use ($dir, $env, $filter, $setup) {
                    if ('boot' === $setup) {
                        $_SERVER['TEST_ENV_KEY'] = $env;
                        (new Dotenv('TEST_ENV_KEY'))->bootEnv($dir.'/.env');
                    } elseif ('vars_only' === $setup) {
                        $_SERVER['SYMFONY_DOTENV_VARS'] = 'FOO,LONG';
                        $_SERVER['FOO'] = 'server';
                        $_SERVER['LONG'] = str_repeat('x', 40);
                    }
                    $command = new DebugCommand($env, $dir);
                    $command->setHelperSet(new HelperSet([new FormatterHelper()]));
                    $tester = new CommandTester($command);
                    $status = $tester->execute(null === $filter ? [] : ['filter' => $filter], ['decorated' => false]);

                    return ['status' => $status, 'display' => $tester->getDisplay(true)];
                });
                $cases[] = ['command' => 'debug:dotenv', 'project' => $name, 'files' => $files, 'env' => $env, 'filter' => $filter, 'setup' => $setup] + $result;
            }
        }
    }
}
// a custom dotenv path: the project directory is the parent
$_SERVER = [];
$_ENV = [];
$dir = $setUp('nested/inner', $projects['plain']);
$result = $run(static function () use ($root) {
    $_SERVER['APP_RUNTIME_OPTIONS'] = ['dotenv_path' => 'nested/inner/.env'];
    $_SERVER['TEST_ENV_KEY'] = 'prod';
    (new Dotenv('TEST_ENV_KEY'))->bootEnv("$root/nested/inner/.env");
    $command = new DebugCommand('prod', $root);
    $command->setHelperSet(new HelperSet([new FormatterHelper()]));
    $tester = new CommandTester($command);
    $status = $tester->execute([], ['decorated' => false]);

    return ['status' => $status, 'display' => $tester->getDisplay(true)];
});
$cases[] = ['command' => 'debug:dotenv', 'project' => 'nested/inner', 'files' => $projects['plain'], 'env' => 'prod', 'filter' => null, 'setup' => 'custom_path'] + $result;

// dotenv:dump
$dumps = [
    'basic' => ['.env' => "APP_ENV=dev\nAPP_SECRET=abc123", '.env.local' => 'APP_LOCAL=yes'],
    'quotes' => ['.env' => "APP_ENV=prod\nQ=\"it's\"\nB='back\\\\slash'\nEMPTY=\nNUM=12\n"],
    'own_env' => ['.env' => "APP_ENV=staging\nX=1\n", '.env.staging' => "X=2\n"],
    'composer' => ['.env' => "MY_ENV=dev\nY=1\n", '.env.test' => "Y=test\n", '.env.local' => "Z=local\n", 'composer.json' => '{"extra":{"runtime":{"env_var_name":"MY_ENV","test_envs":[]}}}'],
    'dotenv_path' => ['.env.path' => "APP_ENV=test\nAPP_SECRET=newpath123", '.env.path.local' => 'LOCAL_PATH=yes', 'composer.json' => '{"extra":{"runtime":{"dotenv_path":".env.path"}}}'],
];
foreach ($dumps as $name => $files) {
    foreach ([['env' => 'test'], ['env' => 'prod'], ['env' => 'test', '--empty' => true], []] as $i => $input) {
        foreach ([null, 'prod'] as $defaultEnv) {
            $_SERVER = [];
            $_ENV = [];
            $dir = $setUp("dump-$name", $files);
            $result = $run(static function () use ($dir, $input, $defaultEnv) {
                $application = new Application();
                $application->addCommand(new DotenvDumpCommand($dir, $defaultEnv));
                $tester = new CommandTester($application->find('dotenv:dump'));
                $status = $tester->execute($input, ['decorated' => false]);
                // glob() skips dot files, so the directory is listed
                $dumped = array_values(array_map(static fn ($f) => "$dir/$f", array_filter(scandir($dir), static fn ($f) => str_ends_with($f, '.local.php'))));

                return ['status' => $status, 'display' => $tester->getDisplay(true), 'dumped' => $dumped ? [basename($dumped[0]), file_get_contents($dumped[0])] : null];
            });
            $cases[] = ['command' => 'dotenv:dump', 'project' => "dump-$name", 'files' => $files, 'input' => $input, 'default_env' => $defaultEnv] + $result;
        }
    }
}

echo json_encode($cases, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
