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
//   create_from_format, date_create, date_interval
//              what the Types read with PHP's date functions:
//              DateTimeImmutable::createFromFormat($format, $text) for
//              formats that set the whole date (so the answer does not
//              depend on the clock), new DateTimeImmutable($text) and new
//              DateInterval($text), each ["ok", text] (the date as
//              'Y-m-d H:i:s.u e', the interval as '%R %y %m %d %h %i %s'),
//              ["false"] or ["throws", class, message].
//
// A mutable DateTime in a provider row is ["datetime_mutable", <the same
// instant as a typed DateTimeImmutable>].
//
// The test classes are loaded with a stub PHPUnit TestCase: only their
// static providers run.

namespace PHPUnit\Framework {
    if (!class_exists(TestCase::class)) {
        abstract class TestCase
        {
            public static function assertIsString(mixed $actual, string $message = ''): void
            {
            }
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

    function dbal_typed(mixed $v): array
    {
        if ($v instanceof DateTime) {
            return ['datetime_mutable', typed_value(DateTimeImmutable::createFromMutable($v))];
        }
        if (is_array($v)) {
            $pairs = [];
            foreach ($v as $k => $item) {
                $pairs[] = [typed_value($k), dbal_typed($item)];
            }

            return ['array', $pairs];
        }

        return typed_value($v);
    }

    $providers = [];
    $classes = [
        'SQL/ParserTest', 'Connection/ExpandArrayParametersTest',
        'Types/BinaryTest', 'Types/ConversionExceptionTest', 'Types/DateIntervalTest', 'Types/DateTest',
        'Types/DateTimeTest', 'Types/DateTimeTzTest', 'Types/JsonObjectTest', 'Types/JsonTest',
        'Types/TimeTest', 'Types/TypeTest',
    ];
    foreach ($classes as $path) {
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
                    $rows[] = [(string) $name, array_map('dbal_typed', array_values($arguments))];
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

    function date_outcome(callable $f): array
    {
        try {
            $result = $f();
        } catch (Throwable $e) {
            return ['throws', $e::class, $e->getMessage()];
        }
        if ($result === false) {
            return ['false'];
        }
        if ($result instanceof DateInterval) {
            return ['ok', $result->format('%R %y %m %d %h %i %s')];
        }

        return ['ok', $result->format('Y-m-d H:i:s.u e')];
    }

    // Formats that set the year, month and day (or start with '!').
    $formats = [
        '!Y-m-d', 'Y-m-d H:i:s', '!H:i:s', 'Y-m-d H:i:sO', 'Y-m-d H:i:sP', 'Y-m-d H:i:s T', 'Y-m-d H:i:s.u',
        '!Y-m-d H:i:s.v', 'Y-m-d\\TH:i:s', '!d/m/Y', '!j.n.Y G:i', '!Y-m-d|', 'Y-m-d H:i:s e', '!', '!Ymd',
        'Y-m-d  H:i:s', '!Y-m-d H', '!(Y);m,d',
    ];
    $datePieces = [
        '2016', '1985', '85', '0', '00', '01', '1', '09', '12', '13', '30', '31', '45', '99', '123', '1234567',
        '-', '/', ':', '.', ',', ' ', '  ', "\t", 'T', 't', 'Z', 'z', 'UTC', 'utc', 'GMT', '+0200', '+02:00',
        '-05', '+2', 'EST', 'Europe/Paris', 'abc', 'now', '(', ')', ';', '+', '@1',
    ];
    mt_srand(20261125);
    $texts = [
        '2016-01-01', '2016-01-01 15:58:59', '15:58:59', '5:30:55', '2016-01-01 15:58:59 UTC',
        '2016-01-01 15:58:59.123456', '2016-13-45 25:61:61', '2016-1-1 1:1:1', '19850-09-01', '85-09-01',
        '2016-02-30', '2016-00-00', '24:00:00', '99:99:99', '', 'abcdefg', 'invalid date string',
    ];
    for ($i = 0; $i < 600; ++$i) {
        $text = '';
        for ($n = mt_rand(1, 9); $n > 0; --$n) {
            $text .= $datePieces[mt_rand(0, count($datePieces) - 1)];
        }
        $texts[] = $text;
    }
    // Texts built like each format's own output.
    foreach (['2016-01-01 15:58:59', '1985-09-01 00:00:00', '2009-11-01 23:59:60'] as $sample) {
        foreach ($formats as $format) {
            $texts[] = (new DateTimeImmutable($sample))->format(ltrim($format, '!|'));
        }
    }
    $createFromFormat = [];
    foreach ($formats as $format) {
        foreach ($texts as $text) {
            $createFromFormat[] = [$format, base64_encode($text), date_outcome(static fn () => DateTimeImmutable::createFromFormat($format, $text))];
        }
    }

    $words = ['foo', 'abcdefg', 'invalid', 'x', 'Abcdefgh', 'zzz', 'qwerty', 'now', 'today', 'monday', 'utc', 'nowhere'];
    $dateCreate = [];
    foreach (array_merge($texts, $words, array_map(static fn ($w) => $w . ' date string', $words)) as $text) {
        $dateCreate[] = [base64_encode($text), date_outcome(static fn () => new DateTimeImmutable($text))];
    }

    $intervalPieces = ['P', 'T', 'Y', 'M', 'W', 'D', 'H', 'S', '1', '2', '0', '12', '99', '1234567890', ' ', '-', '+', '.', 'p', 'R5/', ':'];
    $intervals = [
        'P2Y1DT1H2M3S', 'P02Y00M01DT01H02M03S', 'abcdefg', '', 'P1W', 'P1W2D', 'PT36H', 'P0D', 'P', 'PT', 'P1DT',
        'P1D T1H', 'P-1D', 'P1Y1Y', 'P1H', 'PT1D', 'P1M1Y', ' P1D', "P1D\t", 'P0001-02-03T04:05:06', 'R5/P1D',
    ];
    for ($i = 0; $i < 2000; ++$i) {
        $text = '';
        for ($n = mt_rand(1, 8); $n > 0; --$n) {
            $text .= $intervalPieces[mt_rand(0, count($intervalPieces) - 1)];
        }
        $intervals[] = $text;
    }
    $dateInterval = [];
    foreach ($intervals as $text) {
        $dateInterval[] = [base64_encode($text), date_outcome(static fn () => new DateInterval($text))];
    }

    echo json_encode([
        'providers' => $providers,
        'parser' => $parser,
        'expand' => $expand,
        'create_from_format' => $createFromFormat,
        'date_create' => $dateCreate,
        'date_interval' => $dateInterval,
    ], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
}
