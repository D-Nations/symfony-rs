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
//
// Query\Expression\CompositeExpression values in provider rows are
// ["composite", type, [parts]].
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

    echo json_encode(['providers' => $providers, 'parser' => $parser, 'expand' => $expand, 'query' => $query], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
}
