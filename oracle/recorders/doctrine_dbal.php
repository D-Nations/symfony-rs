<?php
// doctrine/dbal vectors (scripts/record.sh doctrine-dbal), as typed JSON
// (lib/typed.php). Output: crates/doctrine-dbal/tests/data/php.json.
//
//   providers  the #[DataProvider] rows of the upstream test classes that
//              the crate ports (by class path, e.g. "SQL/ParserTest"),
//              enums as ["enum", class, case, null].
//   parser     generated statements, each parsed by the real
//              Doctrine\DBAL\SQL\Parser in ANSI and in MySQL mode: the
//              visitor calls in order (["named"|"positional"|"other",
//              fragment]) or ["throws", class, message, code].
//   expand     ExpandArrayParameters on cases the tests leave open: the SQL,
//              parameters and types it gives, or the exception.
//
// The test classes are loaded with a stub PHPUnit TestCase: only their
// static providers run.

namespace PHPUnit\Framework {
    if (!class_exists(TestCase::class)) {
        abstract class TestCase
        {
        }
    }
}

namespace {
    if (is_file('/demo/vendor/autoload.php')) {
        require '/demo/vendor/autoload.php';
    }
    require __DIR__ . '/lib/typed.php';

    $dbal = '/work/upstream/doctrine/dbal';
    spl_autoload_register(static function (string $name) use ($dbal): void {
        foreach (['Doctrine\\DBAL\\Tests\\' => '/tests/', 'Doctrine\\DBAL\\' => '/src/'] as $prefix => $dir) {
            if (str_starts_with($name, $prefix)) {
                $file = $dbal . $dir . str_replace('\\', '/', substr($name, strlen($prefix))) . '.php';
                if (is_file($file)) {
                    require_once $file;
                }

                return;
            }
        }
    });

    use Doctrine\DBAL\ArrayParameterType;
    use Doctrine\DBAL\ExpandArrayParameters;
    use Doctrine\DBAL\ParameterType;
    use Doctrine\DBAL\SQL\Parser;
    use Doctrine\DBAL\SQL\Parser\Visitor;

    $providers = [];
    foreach (['SQL/ParserTest', 'Connection/ExpandArrayParametersTest'] as $path) {
        $reflection = new ReflectionClass('Doctrine\\DBAL\\Tests\\' . str_replace('/', '\\', $path));
        $out = [];
        foreach ($reflection->getMethods() as $method) {
            foreach ($method->getAttributes() as $attribute) {
                if (!str_ends_with($attribute->getName(), '\\DataProvider')) {
                    continue;
                }
                $provider = $attribute->getArguments()[0];
                $rows = [];
                foreach ($reflection->getMethod($provider)->invoke(null) as $name => $arguments) {
                    $rows[] = [(string) $name, array_map('typed_value', array_values($arguments))];
                }
                $out[$method->getName()] = ['provider' => $provider, 'rows' => $rows];
            }
        }
        ksort($out);
        $providers[$path] = $out;
    }

    final class RecordingVisitor implements Visitor
    {
        public array $calls = [];

        public function acceptPositionalParameter(string $sql): void
        {
            $this->calls[] = ['positional', base64_encode($sql)];
        }

        public function acceptNamedParameter(string $sql): void
        {
            $this->calls[] = ['named', base64_encode($sql)];
        }

        public function acceptOther(string $sql): void
        {
            $this->calls[] = ['other', base64_encode($sql)];
        }
    }

    // Fragments that start, end or confuse each token of the parser.
    $pieces = [
        '?', '??', '?', ':', '::', ':name', ':a1_', ':1', "'", '"', '`', '[', ']', '\\', '\\\\',
        '-', '--', '/', '/*', '*/', '*', '**', "\n", "\r", ' ', 'a', '_', '0', 'ARRAY', 'array',
        'ArRaY', 'xarray', '_array', '(', ')', ',', '@', '$1', "\xC3\xA9", "\x80", "\xFF", "\t", "\0",
        "''", '""', '``', '[]', "'?'", '"?"', '`?`', '[?]', 'SELECT ', ' FROM t WHERE a = ',
    ];
    mt_srand(20251125);
    $statements = [];
    for ($i = 0; $i < 3000; ++$i) {
        $sql = '';
        for ($n = mt_rand(1, 14); $n > 0; --$n) {
            $sql .= $pieces[mt_rand(0, count($pieces) - 1)];
        }
        $statements[] = $sql;
    }
    // Long inputs, where PCRE stops at its backtrack or JIT stack limit
    // (README.md, Differences).
    foreach ([1000, 8000, 30000] as $length) {
        $statements[] = '/*' . str_repeat('a', $length);
        $statements[] = '/*' . str_repeat('a*', $length / 2);
        $statements[] = "'" . str_repeat('a', $length);
        $statements[] = "'" . str_repeat('\\a', $length / 2);
        $statements[] = "'" . str_repeat('a', $length) . "' = ?";
        $statements[] = '/*' . str_repeat('ab', $length / 2) . '*/ ?';
        $statements[] = str_repeat('a', $length) . ' ?';
        $statements[] = str_repeat('?,', $length / 20);
        $statements[] = '[' . str_repeat('a', $length) . '] = ?';
    }

    $parser = [];
    foreach ($statements as $sql) {
        $row = [base64_encode($sql)];
        foreach ([false, true] as $mySQL) {
            $visitor = new RecordingVisitor();
            try {
                (new Parser($mySQL))->parse($sql, $visitor);
                $row[] = $visitor->calls;
            } catch (Throwable $e) {
                $row[] = ['throws', $e::class, $e->getMessage(), $e->getCode()];
            }
        }
        $parser[] = $row;
    }

    $expand = [];
    $cases = [
        ['SELECT ?', ['a'], [ArrayParameterType::INTEGER]],
        ['SELECT ?', [null], [ArrayParameterType::STRING]],
        ['SELECT ?', [1.5], [ArrayParameterType::STRING]],
        ['SELECT :a', ['a' => true], ['a' => ArrayParameterType::STRING]],
        ['SELECT :1, ?', [1 => 'x', 0 => 'y'], []],
        ['SELECT :1', ['1' => 'x'], ['1' => ParameterType::BOOLEAN]],
        ['SELECT ?, ?', ['x', 'y'], [1 => ArrayParameterType::INTEGER]],
        ['SELECT ?', [[5 => 'a', 'k' => 'b']], [ArrayParameterType::BINARY]],
        ['SELECT ?, :x, ?', ['x' => 1, 0 => 2, 1 => 3], []],
        ['SELECT ?', [], []],
        ['SELECT :x', ['x' => null], ['x' => ParameterType::NULL]],
        ['SELECT :x', ['x' => 1], ['x' => 'datetime']],
        ['SELECT ?', [false], [ArrayParameterType::ASCII]],
        ['SELECT ?', [new ArrayObject([1])], [ArrayParameterType::INTEGER]],
        ['SELECT ?', [new stdClass()], [ArrayParameterType::INTEGER]],
    ];
    foreach ($cases as [$sql, $params, $types]) {
        try {
            $visitor = new ExpandArrayParameters($params, $types);
            (new Parser(true))->parse($sql, $visitor);
            $result = typed_value([$visitor->getSQL(), $visitor->getParameters(), $visitor->getTypes()]);
        } catch (Throwable $e) {
            $result = ['throws', $e::class, $e->getMessage()];
        }
        $expand[] = [typed_value($sql), typed_value($params), typed_value($types), $result];
    }

    echo json_encode(['providers' => $providers, 'parser' => $parser, 'expand' => $expand], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
}
