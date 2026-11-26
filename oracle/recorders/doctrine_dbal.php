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
//   query      Query\QueryBuilder call sequences that the tests leave open,
//              each run on two platforms: "stub" (QueryBuilderTest's
//              platform stub: no FOR UPDATE, union parts as given) and
//              "default" (AbstractPlatform's own methods, as
//              PostgreSQLPlatform keeps them). Per call the result: ["ok"],
//              ["sql", text] for getSQL(), ["value", typed] for a call that
//              returns a value, or ["throws", class, message]; then the
//              final getSQL() (or throws), getParameters() and
//              getParameterTypes().
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
// instant as a typed DateTimeImmutable>]; a
// Query\Expression\CompositeExpression is ["composite", type, [parts]].
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

// doctrine/deprecations is not installed: the test traits load and the
// deprecations are dropped.
namespace Doctrine\Deprecations {
    if (!class_exists(Deprecation::class)) {
        final class Deprecation
        {
            public static function trigger(string $package, string $link, string $message, mixed ...$args): void
            {
            }

            public static function triggerIfCalledFromOutside(string $package, string $link, string $message, mixed ...$args): void
            {
            }
        }
    }
}

namespace Doctrine\Deprecations\PHPUnit {
    if (!trait_exists(VerifyDeprecations::class)) {
        trait VerifyDeprecations
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
    use Doctrine\DBAL\Connection;
    use Doctrine\DBAL\ExpandArrayParameters;
    use Doctrine\DBAL\ParameterType;
    use Doctrine\DBAL\Platforms\AbstractPlatform;
    use Doctrine\DBAL\Platforms\PostgreSQLPlatform;
    use Doctrine\DBAL\Query\Expression\CompositeExpression;
    use Doctrine\DBAL\Query\ForUpdate\ConflictResolutionMode;
    use Doctrine\DBAL\Query\QueryBuilder;
    use Doctrine\DBAL\Query\UnionType;
    use Doctrine\DBAL\SQL\Builder\DefaultSelectSQLBuilder;
    use Doctrine\DBAL\SQL\Builder\SelectSQLBuilder;
    use Doctrine\DBAL\SQL\Parser;
    use Doctrine\DBAL\SQL\Parser\Visitor;

    // typed_value(), with CompositeExpression's private parts.
    function dbal_typed(mixed $v): array
    {
        if ($v instanceof CompositeExpression) {
            $parts = (new ReflectionProperty(CompositeExpression::class, 'parts'))->getValue($v);

            return ['composite', $v->getType(), array_map('dbal_typed', array_values($parts))];
        }
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
        'SQL/ParserTest',
        'Connection/ExpandArrayParametersTest',
        'Query/QueryBuilderTest',
        'Query/Expression/ExpressionBuilderTest',
        'Query/Expression/CompositeExpressionTest',
        'Types/BinaryTest', 'Types/ConversionExceptionTest', 'Types/DateIntervalTest', 'Types/DateTest',
        'Types/DateTimeTest', 'Types/DateTimeTzTest', 'Types/JsonObjectTest', 'Types/JsonTest',
        'Types/TimeTest', 'Types/TypeTest',
        'Platforms/SQLitePlatformTest',
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

    // A Connection that only gives its platform (QueryBuilder needs nothing
    // else of it here); the parent constructor is not called.
    final class RecorderConnection extends Connection
    {
        public function __construct(private readonly AbstractPlatform $recorderPlatform)
        {
        }

        public function getDatabasePlatform(): AbstractPlatform
        {
            return $this->recorderPlatform;
        }
    }

    // QueryBuilderTest's platform stub, on AbstractPlatform's other methods.
    final class StubPlatform extends PostgreSQLPlatform
    {
        public function getUnionSelectPartSQL(string $subQuery): string
        {
            return $subQuery;
        }

        public function createSelectSQLBuilder(): SelectSQLBuilder
        {
            return new DefaultSelectSQLBuilder($this, null, null);
        }
    }

    // Calls as [method, ...arguments]. Arguments are JSON values; for
    // values() a list of [column, value] pairs; for forUpdate() and
    // addUnion() the enum case name; for setParameter() and
    // create*Parameter() the type as a ParameterType or ArrayParameterType
    // case name ("ArrayParameterType::INTEGER") or a type name string.
    $queries = [
        'limit only' => [['select', 'a'], ['from', 't'], ['setMaxResults', 5]],
        'offset only' => [['select', 'a'], ['from', 't'], ['setFirstResult', 3]],
        'limit zero' => [['select', 'a'], ['from', 't'], ['setMaxResults', 0]],
        'limit and offset' => [['select', 'a'], ['distinct'], ['from', 't'], ['orderBy', 'a', 'DESC'], ['setMaxResults', 10], ['setFirstResult', 20]],
        'negative offset' => [['select', 'a'], ['from', 't'], ['setFirstResult', -1]],
        'limit reset to null' => [['select', 'a'], ['from', 't'], ['setMaxResults', 5], ['getSQL'], ['setMaxResults', null], ['getSQL']],
        'for update' => [['select', 'a'], ['from', 't'], ['forUpdate']],
        'for update skip locked' => [['select', 'a'], ['from', 't'], ['forUpdate', 'SKIP_LOCKED']],
        'for update with limit' => [['select', 'a'], ['from', 't'], ['setMaxResults', 1], ['forUpdate', 'ORDINARY']],
        'union of three' => [['union', 'SELECT 1'], ['addUnion', 'SELECT 2', 'ALL'], ['addUnion', 'SELECT 3', 'DISTINCT'], ['orderBy', 'x'], ['setMaxResults', 2]],
        'union with offset' => [['union', 'SELECT 1'], ['addUnion', 'SELECT 2'], ['setFirstResult', 4]],
        'union with cte' => [['with', 'c', 'SELECT 1', ['x']], ['union', 'SELECT x FROM c'], ['addUnion', 'SELECT 2']],
        'union replaced by union()' => [['union', 'SELECT 1'], ['addUnion', 'SELECT 2'], ['union', 'SELECT 3'], ['getSQL']],
        'select after union' => [['union', 'SELECT 1'], ['addUnion', 'SELECT 2'], ['select', 'a'], ['from', 't']],
        'addUnion first changes type' => [['select', 'a'], ['addUnion', 'SELECT 1'], ['getSQL']],
        'addUnion first keeps cached sql' => [['select', 'a'], ['getSQL'], ['addUnion', 'SELECT 1'], ['getSQL']],
        'cte columns null and list' => [['with', 'a', 'SELECT 1'], ['with', 'b', 'SELECT 2', null], ['with', 'c', 'SELECT 3', ['x', 'y']], ['select', '*'], ['from', 'c']],
        'cte empty columns' => [['with', 'a', 'SELECT 1', []], ['select', '*']],
        'cte without select' => [['with', 'a', 'SELECT 1']],
        'where with several predicates' => [['select', 'a'], ['from', 't'], ['where', 'x = 1', 'y = 2', 'z = 3']],
        'andWhere with several predicates first' => [['select', 'a'], ['from', 't'], ['andWhere', 'x = 1', 'y = 2']],
        'orWhere with several predicates on a string' => [['select', 'a'], ['from', 't'], ['where', 'x = 1'], ['orWhere', 'y = 2', 'z = 3']],
        'orWhere then andWhere several' => [['select', 'a'], ['from', 't'], ['orWhere', 'x = 1', 'y = 2'], ['andWhere', 'z = 3', 'w = 4'], ['orWhere', 'v = 5']],
        'having several' => [['select', 'a'], ['from', 't'], ['groupBy', 'a', 'b'], ['having', 'x', 'y'], ['orHaving', 'z']],
        'groupBy replaces' => [['select', 'a'], ['from', 't'], ['groupBy', 'a'], ['addGroupBy', 'b'], ['groupBy', 'c', 'd']],
        'orderBy replaces' => [['select', 'a'], ['from', 't'], ['addOrderBy', 'a'], ['orderBy', 'b', 'ASC'], ['addOrderBy', 'c', '']],
        'same table twice' => [['select', 'a'], ['from', 't'], ['from', 't']],
        'same alias twice' => [['select', 'a'], ['from', 't', 'x'], ['from', 'u', 'y'], ['from', 'v', 'x']],
        'same alias twice with join' => [['select', 'a'], ['from', 't', 'x'], ['join', 'x', 'j', 'k'], ['from', 'v', 'x']],
        'join without from' => [['select', 'a'], ['join', 'x', 'j', 'k']],
        'join to a later from' => [['select', 'a'], ['from', 't', 'x'], ['join', 'y', 'j', 'k'], ['from', 'u', 'y']],
        'numeric aliases' => [['select', 'a'], ['from', 't', '1'], ['join', '1', 'u', '2', 'c'], ['join', '2', 'v', '3']],
        'numeric alias not unique' => [['select', 'a'], ['from', 't', '1'], ['join', '1', 'u', '1']],
        // PHP's array key makes the alias an int, and UnknownAlias::new()
        // refuses it with a TypeError (README.md, Differences).
        'numeric alias unknown' => [['select', 'a'], ['from', 't', '1'], ['join', '9', 'v', '3']],
        'join alias equals from table' => [['select', 'a'], ['from', 't'], ['join', 't', 'u', 't']],
        'join cycle' => [['select', 'a'], ['from', 't', 'a'], ['join', 'a', 'u', 'b'], ['join', 'b', 'v', 'a']],
        'deep joins' => [['select', '*'], ['from', 't', 'a'], ['leftJoin', 'a', 'u', 'b', 'b.a = a.id'], ['rightJoin', 'b', 'v', 'c'], ['fullJoin', 'a', 'w', 'd', 'd.a = a.id'], ['innerJoin', 'c', 'x', 'e', '']],
        'update without set' => [['update', 't']],
        'update with composite where' => [['update', 't'], ['set', 'a', '1'], ['where', 'x', 'y'], ['orWhere', 'z']],
        'insert without values' => [['insert', 't']],
        'insert values then setValue cached' => [['insert', 't'], ['setValue', 'a', '1'], ['getSQL'], ['setValue', 'b', '2'], ['getSQL'], ['values', [['c', '3']]], ['getSQL']],
        'insert numeric columns' => [['insert', 't'], ['values', [['1', 'a'], ['b', 'b']]], ['setValue', '1', 'c']],
        'delete with andWhere' => [['delete', 't'], ['andWhere', 'a'], ['andWhere', 'b']],
        'delete ignores select parts' => [['select', 'a'], ['from', 'x'], ['delete', 't'], ['orderBy', 'a'], ['setMaxResults', 1]],
        'select resets to select' => [['delete', 't'], ['addSelect', 'a', 'b'], ['from', 'u']],
        'reset where then andWhere' => [['select', 'a'], ['from', 't'], ['where', 'x', 'y'], ['resetWhere'], ['andWhere', 'z']],
        'reset having then orHaving' => [['select', 'a'], ['from', 't'], ['having', 'x'], ['resetHaving'], ['orHaving', 'z'], ['resetGroupBy'], ['resetOrderBy']],
        'distinct false' => [['select', 'a'], ['distinct'], ['distinct', false]],
        'expressions' => [['select', 'a'], ['from', 't'], ['where', ['expr', 'like', 'a', "'x'", "'!'"], ['expr', 'notLike', 'b', "'y'"], ['expr', 'in', 'c', []], ['expr', 'notIn', 'd', ['1', '2']], ['expr', 'isNull', 'e'], ['expr', 'isNotNull', 'f'], ['expr', 'comparison', 'g', '~', 'h']]],
        'composite expressions' => [['select', 'a'], ['from', 't'], ['where', ['expr', 'or', 'x']], ['andWhere', ['expr', 'or', 'y', ['expr', 'and', 'z']]], ['having', ['expr', 'and', 'p', 'q']], ['andHaving', 'r'], ['orHaving', ['expr', 'or', 's', 't']]],
        'named parameters' => [['createNamedParameter', 1, 'ParameterType::INTEGER'], ['createNamedParameter', 'x', 'ParameterType::STRING', ':name'], ['createNamedParameter', 'y', 'ParameterType::STRING', 'abc'], ['createNamedParameter', [1, 2], 'ArrayParameterType::INTEGER'], ['select', 'a']],
        'positional parameters' => [['createPositionalParameter', 1, 'ParameterType::INTEGER'], ['createNamedParameter', 2, 'ParameterType::STRING'], ['createPositionalParameter', true, 'ParameterType::BOOLEAN'], ['createPositionalParameter', null, 'datetime']],
        'setParameter keys' => [['setParameter', '1', 'a', 'ParameterType::STRING'], ['setParameter', 'x', 'b', 'ParameterType::INTEGER'], ['setParameter', 1, 'c', 'string'], ['setParameter', '01', 'd', 'ParameterType::ASCII'], ['getParameter', 1], ['getParameter', 'missing'], ['getParameterType', '01'], ['getParameterType', 'missing']],
        'setParameters replaces' => [['setParameter', 'x', 'a', 'ParameterType::INTEGER'], ['setParameters', [['y', 'b']], [['y', 'ParameterType::BOOLEAN']]], ['getParameterType', 'x'], ['setParameters', [['z', 'c'], ['w', 'd']]]],
        'setParameter keeps cached sql' => [['select', 'a'], ['getSQL'], ['setParameter', 'x', 1, 'ParameterType::INTEGER'], ['getSQL']],
        'first and max results' => [['setFirstResult', 7], ['getFirstResult'], ['getMaxResults'], ['setMaxResults', 3], ['getMaxResults']],
    ];

    function query_parameter_type(string $type): ParameterType|ArrayParameterType|string
    {
        if (str_starts_with($type, 'ParameterType::')) {
            return constant(ParameterType::class . '::' . substr($type, 15));
        }
        if (str_starts_with($type, 'ArrayParameterType::')) {
            return constant(ArrayParameterType::class . '::' . substr($type, 20));
        }

        return $type;
    }

    function query_argument(QueryBuilder $qb, mixed $argument): mixed
    {
        if (!is_array($argument) || ($argument[0] ?? null) !== 'expr') {
            return $argument;
        }
        $method = $argument[1];
        $arguments = array_map(static fn ($a) => query_argument($qb, $a), array_slice($argument, 2));

        return $qb->expr()->$method(...$arguments);
    }

    function query_pairs(array $pairs, bool $types = false): array
    {
        $array = [];
        foreach ($pairs as [$key, $value]) {
            $array[$key] = $types ? query_parameter_type($value) : $value;
        }

        return $array;
    }

    function query_call(QueryBuilder $qb, array $call): mixed
    {
        $method = array_shift($call);

        return match ($method) {
            'forUpdate' => isset($call[0]) ? $qb->forUpdate(constant(ConflictResolutionMode::class . '::' . $call[0])) : $qb->forUpdate(),
            'addUnion' => isset($call[1]) ? $qb->addUnion($call[0], constant(UnionType::class . '::' . $call[1])) : $qb->addUnion($call[0]),
            'values' => $qb->values(query_pairs($call[0])),
            'setParameters' => $qb->setParameters(query_pairs($call[0]), query_pairs($call[1] ?? [], true)),
            'setParameter' => $qb->setParameter($call[0], $call[1], query_parameter_type($call[2])),
            'createNamedParameter' => $qb->createNamedParameter($call[0], query_parameter_type($call[1]), ...array_slice($call, 2)),
            'createPositionalParameter' => $qb->createPositionalParameter($call[0], query_parameter_type($call[1])),
            default => $qb->$method(...array_map(static fn ($a) => query_argument($qb, $a), $call)),
        };
    }

    $query = [];
    foreach ($queries as $name => $calls) {
        foreach (['stub' => new StubPlatform(), 'default' => new PostgreSQLPlatform()] as $platformName => $platform) {
            $qb = new QueryBuilder(new RecorderConnection($platform));
            $results = [];
            foreach ($calls as $call) {
                try {
                    $result = query_call($qb, $call);
                    $results[] = match (true) {
                        $call[0] === 'getSQL' => ['sql', base64_encode($result)],
                        $result === $qb => ['ok'],
                        default => ['value', typed_value($result)],
                    };
                } catch (Throwable $e) {
                    $results[] = ['throws', $e::class, $e->getMessage()];
                }
            }
            try {
                $sql = ['sql', base64_encode($qb->getSQL())];
            } catch (Throwable $e) {
                $sql = ['throws', $e::class, $e->getMessage()];
            }
            $query[] = [
                $name,
                $platformName,
                $calls,
                $results,
                $sql,
                typed_value($qb->getParameters()),
                typed_value($qb->getParameterTypes()),
            ];
        }
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

    // SQLitePlatform on generated inputs (sqlite_platform).
    function sqlite_outcome(callable $f): array
    {
        try {
            $result = $f();
        } catch (Throwable $e) {
            // A TypeError names the PHP file and line of the call.
            return ['throws', $e::class, preg_replace('/, called in \S+ on line \d+$/', '', $e->getMessage())];
        }

        return ['ok', $result];
    }

    function sqlite_column(array $spec): Doctrine\DBAL\Schema\Column
    {
        [$name, $type, $options] = $spec;

        return new Doctrine\DBAL\Schema\Column($name, Doctrine\DBAL\Types\Type::getType($type), typed_decode_options($options));
    }

    function typed_decode_options(array $options): array
    {
        $out = [];
        foreach ($options as [$key, $value]) {
            $out[$key] = $value;
        }

        return $out;
    }

    function sqlite_table(array $spec): Doctrine\DBAL\Schema\Table
    {
        $table = new Doctrine\DBAL\Schema\Table($spec['name']);
        foreach ($spec['columns'] as [$name, $type, $options]) {
            $table->addColumn($name, $type, typed_decode_options($options));
        }
        if ($spec['pk'] !== null) {
            $table->setPrimaryKey($spec['pk']);
        }
        foreach ($spec['indexes'] as [$name, $columns, $unique]) {
            if ($unique) {
                $table->addUniqueIndex($columns, $name);
            } else {
                $table->addIndex($columns, $name);
            }
        }
        foreach ($spec['fks'] as [$name, $local, $foreignTable, $foreign, $options]) {
            $table->addForeignKeyConstraint($foreignTable, $local, $foreign, typed_decode_options($options), $name);
        }
        if ($spec['comment'] !== null) {
            $table->setComment($spec['comment']);
        }

        return $table;
    }

    mt_srand(20261126);
    $pick = static fn (array $a) => $a[mt_rand(0, count($a) - 1)];
    $sqlite = new Doctrine\DBAL\Platforms\SQLitePlatform();

    $exprPieces = ['a', 'col', 't.c', "'x'", "'it''s'", '?', ':p', '1', '0', '-3', ' ', '%', '_', '\\', '"', '(', ')', ',', 'é', 'NULL'];
    $expr = static function () use ($pick, $exprPieces): string {
        $text = '';
        for ($n = mt_rand(1, 4); $n > 0; --$n) {
            $text .= $pick($exprPieces);
        }

        return $text;
    };
    $optional = static fn (callable $f) => mt_rand(0, 2) === 0 ? null : $f();
    $units = ['seconds' => 'Seconds', 'minutes' => 'Minutes', 'hour' => 'Hour', 'days' => 'Days', 'weeks' => 'Weeks', 'month' => 'Month', 'quarters' => 'Quarters', 'years' => 'Years'];
    $expressions = [];
    $calls = ['trim', 'locate', 'substring', 'date_add', 'date_sub', 'date_diff', 'mod', 'concat', 'length', 'escape_like', 'quote_literal', 'quote_single_identifier', 'truncate', 'modify_limit', 'bit_and', 'bit_or'];
    for ($i = 0; $i < 1500; ++$i) {
        $call = $calls[$i % count($calls)];
        switch ($call) {
            case 'trim':
                $mode = $pick(['UNSPECIFIED', 'LEADING', 'TRAILING', 'BOTH']);
                $args = [$mode, $expr(), $optional(static fn () => $pick(["'x'", "' '", "'%'", "'a''b'", 'c', '?']))];
                $f = static fn () => $sqlite->getTrimExpression($args[1], constant('Doctrine\\DBAL\\Platforms\\TrimMode::' . $mode), $args[2]);
                break;
            case 'locate':
                $args = [$expr(), $expr(), $optional($expr)];
                $f = static fn () => $sqlite->getLocateExpression(...$args);
                break;
            case 'substring':
                $args = [$expr(), $expr(), $optional($expr)];
                $f = static fn () => $sqlite->getSubstringExpression(...$args);
                break;
            case 'date_add':
            case 'date_sub':
                $unit = $pick(array_keys($units));
                $args = [$unit, $expr(), $pick(['1', '-1', '0', '12', '?', ':n', 'col', '1 + 2', "'3'"])];
                $method = 'getDate' . ($call === 'date_add' ? 'Add' : 'Sub') . $units[$unit] . 'Expression';
                $f = static fn () => $sqlite->$method($args[1], $args[2]);
                break;
            case 'date_diff':
                $args = [$expr(), $expr()];
                $f = static fn () => $sqlite->getDateDiffExpression(...$args);
                break;
            case 'mod':
            case 'bit_and':
            case 'bit_or':
                $args = [$expr(), $expr()];
                $method = ['mod' => 'getModExpression', 'bit_and' => 'getBitAndComparisonExpression', 'bit_or' => 'getBitOrComparisonExpression'][$call];
                $f = static fn () => $sqlite->$method(...$args);
                break;
            case 'concat':
                $args = [];
                for ($n = mt_rand(0, 4); $n > 0; --$n) {
                    $args[] = $expr();
                }
                $f = static fn () => $sqlite->getConcatExpression(...$args);
                break;
            case 'length':
                $args = [$expr()];
                $f = static fn () => $sqlite->getLengthExpression(...$args);
                break;
            case 'escape_like':
                $args = [$expr(), $pick(['\\', '!', '%', '_', '#'])];
                $f = static fn () => $sqlite->escapeStringForLike(...$args);
                break;
            case 'quote_literal':
            case 'quote_single_identifier':
                $args = [$expr()];
                $method = $call === 'quote_literal' ? 'quoteStringLiteral' : 'quoteSingleIdentifier';
                $f = static fn () => $sqlite->$method(...$args);
                break;
            case 'truncate':
                $args = [$pick(['t', 'users', 'main.users', 'select', 'Mixed', '"quoted"', '`bq`', 'a.b.c']), (bool) mt_rand(0, 1)];
                $f = static fn () => $sqlite->getTruncateTableSQL(...$args);
                break;
            case 'modify_limit':
                $args = [$pick(['SELECT * FROM t', 'SELECT 1', 'SELECT a FROM t ORDER BY a']), $optional(static fn () => mt_rand(-2, 20)), mt_rand(-1, 3) === -1 ? -1 : mt_rand(0, 30)];
                $f = static fn () => $sqlite->modifyLimitQuery(...$args);
                break;
        }
        $expressions[] = [$call, $args, sqlite_outcome($f)];
    }

    // Columns: [name, type, [[option, value], ...]].
    $columnNames = ['id', 'name', 'Mixed', 'select', 'order', 'from_x', 'a b', 'col"q', 'é'];
    $types = ['integer', 'bigint', 'smallint', 'string', 'ascii_string', 'text', 'boolean', 'decimal', 'float', 'smallfloat', 'date', 'date_immutable', 'datetime', 'datetime_immutable', 'datetimetz', 'time', 'guid', 'blob', 'binary', 'json', 'simple_array', 'dateinterval'];
    $columnSpec = static function (?string $name = null) use ($pick, $columnNames, $types): array {
        $type = $pick($types);
        $options = [];
        if (mt_rand(0, 1) === 1) {
            $options[] = ['notnull', (bool) mt_rand(0, 1)];
        }
        if (mt_rand(0, 2) === 0) {
            $options[] = ['default', $pick([null, 0, 1, 42, -7, '', 'abc', "it's", '1', true, false, 'CURRENT_TIMESTAMP', 'CURRENT_DATE', 'CURRENT_TIME', 1.5])];
        }
        if (mt_rand(0, 2) === 0) {
            $options[] = ['length', $pick([null, 1, 10, 255, 4000])];
        }
        if (mt_rand(0, 3) === 0) {
            $options[] = ['fixed', (bool) mt_rand(0, 1)];
        }
        if (mt_rand(0, 3) === 0) {
            $options[] = ['precision', $pick([null, 5, 10])];
            $options[] = ['scale', $pick([0, 2])];
        }
        if (mt_rand(0, 3) === 0) {
            $options[] = ['unsigned', (bool) mt_rand(0, 1)];
        }
        if (mt_rand(0, 3) === 0) {
            $options[] = ['autoincrement', (bool) mt_rand(0, 1)];
        }
        if (mt_rand(0, 3) === 0) {
            $options[] = ['comment', $pick(['', 'a comment', "it's", "multi\nline", '--'])];
        }
        if (mt_rand(0, 12) === 0) {
            $options[] = ['columnDefinition', $pick(['INTEGER NOT NULL', 'TEXT DEFAULT \'x\''])];
        }

        return [$name ?? $pick($columnNames), $type, $options];
    };
    $tableNames = ['t', 'users', 'main.users', 'select', 'Mixed', 'a.b', '"quoted"'];
    $tableSpec = static function () use ($pick, $tableNames, $columnNames, $columnSpec): array {
        $columns = [];
        // Distinct column names, but now and then a repeated one.
        $pool = $columnNames;
        shuffle($pool);
        for ($n = mt_rand(0, 4); $n > 0; --$n) {
            $columns[] = $columnSpec(mt_rand(0, 15) === 0 ? $pick($columnNames) : array_pop($pool));
        }
        $names = array_values(array_unique(array_map(static fn ($c) => $c[0], $columns)));
        $some = static function () use ($names, $pick): array {
            if ($names === [] || mt_rand(0, 49) === 0) {
                return [$pick(['missing', 'id'])];
            }
            $out = [];
            for ($n = mt_rand(1, min(2, count($names))); $n > 0; --$n) {
                $out[] = $pick($names);
            }

            return array_values(array_unique($out));
        };
        $indexes = [];
        for ($n = mt_rand(0, 2); $n > 0; --$n) {
            $indexes[] = [$pick([null, 'idx_a', 'idx_b', 'uniq_c', 'select']), $some(), (bool) mt_rand(0, 1)];
        }
        $fks = [];
        for ($n = mt_rand(0, 2) === 0 ? mt_rand(1, 2) : 0; $n > 0; --$n) {
            $options = [];
            if (mt_rand(0, 1) === 1) {
                $options[] = ['onDelete', $pick(['CASCADE', 'SET NULL', 'RESTRICT', 'NO ACTION', 'SET DEFAULT'])];
            }
            if (mt_rand(0, 2) === 0) {
                $options[] = ['onUpdate', $pick(['CASCADE', 'RESTRICT'])];
            }
            if (mt_rand(0, 3) === 0) {
                $options[] = ['deferrable', true];
                $options[] = ['deferred', (bool) mt_rand(0, 1)];
            }
            $local = $some();
            $fks[] = [$pick([null, 'fk_a', 'fk_b']), $local, $pick(['other', 'main.other', 'Other']), array_slice(['id', 'id2'], 0, count($local)), $options];
        }

        return [
            'name' => $pick($tableNames),
            'columns' => $columns,
            'pk' => mt_rand(0, 2) === 0 ? null : (mt_rand(0, 1) ? $some() : null),
            'indexes' => $indexes,
            'fks' => $fks,
            'comment' => mt_rand(0, 4) === 0 ? $pick(['a table', "it's", '']) : null,
        ];
    };
    $typedSpec = static function (array $spec): array {
        $typed = static fn (array $options) => array_map(static fn ($o) => [$o[0], typed_value($o[1])], $options);
        $spec['columns'] = array_map(static fn ($c) => [$c[0], $c[1], $typed($c[2])], $spec['columns']);
        $spec['fks'] = array_map(static fn ($f) => [$f[0], $f[1], $f[2], $f[3], $typed($f[4])], $spec['fks']);

        return $spec;
    };

    $createTable = [];
    for ($i = 0; $i < 600; ++$i) {
        $spec = $tableSpec();
        $createTable[] = [$typedSpec($spec), sqlite_outcome(static fn () => $sqlite->getCreateTableSQL(sqlite_table($spec)))];
    }

    $alterTable = [];
    for ($i = 0; $i < 400; ++$i) {
        $old = $tableSpec();
        $oldColumns = array_values(array_unique(array_map(static fn ($c) => $c[0], $old['columns'])));
        $oldIndexes = array_values(array_unique(array_filter(array_map(static fn ($x) => $x[0], $old['indexes']))));
        $oldFks = array_values(array_unique(array_filter(array_map(static fn ($x) => $x[0], $old['fks']))));
        $diff = ['added_columns' => [], 'dropped_columns' => [], 'changed_columns' => [], 'added_indexes' => [], 'dropped_indexes' => [], 'renamed_indexes' => [], 'added_fks' => [], 'dropped_fks' => []];
        for ($n = mt_rand(0, 2); $n > 0; --$n) {
            $diff['added_columns'][] = $columnSpec(mt_rand(0, 9) === 0 ? $pick(['id', 'name']) : $pick(['new_col', 'other', 'select', 'Added']));
        }
        foreach ($oldColumns as $name) {
            $r = mt_rand(0, 4);
            if ($r === 0) {
                $diff['dropped_columns'][] = $name;
            } elseif ($r === 1) {
                $diff['changed_columns'][] = [$name, $columnSpec(mt_rand(0, 1) ? $name : $pick(['renamed', 'Renamed', 'order_x']))];
            }
        }
        if (mt_rand(0, 9) === 0) {
            $diff['dropped_columns'][] = 'missing';
        }
        for ($n = mt_rand(0, 1); $n > 0; --$n) {
            $cols = $oldColumns === [] ? ['x'] : [$pick($oldColumns)];
            $diff['added_indexes'][] = [$pick(['idx_new', 'uniq_new', 'idx_a']), $cols, (bool) mt_rand(0, 1)];
        }
        foreach ($oldIndexes as $name) {
            $r = mt_rand(0, 3);
            if ($r === 0) {
                $diff['dropped_indexes'][] = $name;
            } elseif ($r === 1 && $oldColumns !== []) {
                $diff['renamed_indexes'][] = [$name, [$pick(['idx_renamed', 'Renamed_idx']), [$pick($oldColumns)], (bool) mt_rand(0, 1)]];
            }
        }
        if (mt_rand(0, 3) === 0 && $oldColumns !== []) {
            $diff['added_fks'][] = [$pick(['fk_new', null]), [$pick($oldColumns)], 'other', ['id'], mt_rand(0, 1) ? [['onDelete', 'CASCADE']] : []];
        }
        foreach ($oldFks as $name) {
            if (mt_rand(0, 2) === 0) {
                $diff['dropped_fks'][] = $name;
            }
        }
        $outcome = sqlite_outcome(static function () use ($sqlite, $old, $diff) {
            $table = sqlite_table($old);
            $index = static fn (array $s) => new Doctrine\DBAL\Schema\Index($s[0], $s[1], $s[2]);
            $renamed = [];
            foreach ($diff['renamed_indexes'] as [$oldName, $s]) {
                $renamed[$oldName] = $index($s);
            }

            return $sqlite->getAlterTableSQL(new Doctrine\DBAL\Schema\TableDiff(
                $table,
                addedColumns: array_map('sqlite_column', $diff['added_columns']),
                changedColumns: array_map(static fn ($c) => new Doctrine\DBAL\Schema\ColumnDiff($table->getColumn($c[0]), sqlite_column($c[1])), $diff['changed_columns']),
                droppedColumns: array_map(static fn ($n) => $table->getColumn($n), $diff['dropped_columns']),
                addedIndexes: array_map($index, $diff['added_indexes']),
                droppedIndexes: array_map(static fn ($n) => $table->getIndex($n), $diff['dropped_indexes']),
                renamedIndexes: $renamed,
                addedForeignKeys: array_map(static fn ($f) => new Doctrine\DBAL\Schema\ForeignKeyConstraint($f[1], $f[2], $f[3], $f[0] ?? '', typed_decode_options($f[4])), $diff['added_fks']),
                droppedForeignKeys: array_map(static fn ($n) => $table->getForeignKey($n), $diff['dropped_fks']),
            ));
        });
        $typedDiff = $diff;
        $typedDiff['added_columns'] = $typedSpec(['columns' => $diff['added_columns'], 'fks' => []])['columns'];
        $typedDiff['changed_columns'] = array_map(static fn ($c) => [$c[0], $typedSpec(['columns' => [$c[1]], 'fks' => []])['columns'][0]], $diff['changed_columns']);
        $typedDiff['added_fks'] = $typedSpec(['columns' => [], 'fks' => $diff['added_fks']])['fks'];
        $alterTable[] = [$typedSpec($old), $typedDiff, $outcome];
    }

    $sqlitePlatform = [
        'expressions' => $expressions,
        'create_table' => $createTable,
        'alter_table' => $alterTable,
    ];

    echo json_encode([
        'providers' => $providers,
        'parser' => $parser,
        'expand' => $expand,
        'query' => $query,
        'create_from_format' => $createFromFormat,
        'date_create' => $dateCreate,
        'date_interval' => $dateInterval,
        'sqlite_platform' => $sqlitePlatform,
    ], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
}
