<?php
// doctrine/dbal Connection slice vectors (scripts/record.sh
// doctrine-dbal-connection), as typed JSON (lib/typed.php). Output:
// crates/doctrine-dbal/tests/data/connection.json.
//
//   providers  the #[DataProvider] rows of the upstream test classes that
//              the slice ports (by class path, e.g. "Tools/DsnParserTest").
//   ddl        the tables of the functional tests, created on pdo_sqlite
//              by the schema manager: the SQL SQLite keeps for each object
//              (sqlite_master.sql, in creation order).
//   sql        the SQL that SQLitePlatform builds for the functional tests
//              (expressions, dummy selects, savepoints), by name.
//   cases      pdo_sqlite behaviour that the tests leave open: each case a
//              list of operations on one fresh in-memory connection
//              (DriverManager, 'pdo_sqlite', 'memory'), each with its
//              outcome: ["ok", typed] or ["throws", class, message, code,
//              sqlstate, [previous classes, outermost first], query sql].
//
// Run with: docker run --rm -v $PWD:/work:ro -w /work php:8.4.27-cli-bookworm \
//   php oracle/recorders/doctrine_dbal_connection.php

namespace PHPUnit\Framework {
    if (!class_exists(TestCase::class)) {
        abstract class TestCase
        {
        }
    }
}

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
    // PDO warns when it binds an array as a string ("Array"): keep the
    // warnings off the JSON.
    ini_set('display_errors', 'stderr');
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
    use Doctrine\DBAL\DriverManager;
    use Doctrine\DBAL\Exception\DriverException;
    use Doctrine\DBAL\ParameterType;
    use Doctrine\DBAL\Platforms\SQLitePlatform;
    use Doctrine\DBAL\Platforms\TrimMode;
    use Doctrine\DBAL\Schema\Column;
    use Doctrine\DBAL\Schema\Index;
    use Doctrine\DBAL\Schema\Index\IndexType;
    use Doctrine\DBAL\Schema\PrimaryKeyConstraint;
    use Doctrine\DBAL\Schema\Table;
    use Doctrine\DBAL\TransactionIsolationLevel;
    use Doctrine\DBAL\Types\Types;

    function sqlite(): Connection
    {
        return DriverManager::getConnection(['driver' => 'pdo_sqlite', 'memory' => true]);
    }

    function outcome(callable $f): array
    {
        try {
            return ['ok', typed_value($f())];
        } catch (Throwable $e) {
            $previous = [];
            for ($p = $e->getPrevious(); $p !== null; $p = $p->getPrevious()) {
                $previous[] = [$p::class, base64_encode($p->getMessage())];
            }

            return [
                'throws',
                $e::class,
                base64_encode($e->getMessage()),
                $e->getCode(),
                $e instanceof DriverException ? $e->getSQLState() : null,
                $previous,
                $e instanceof DriverException && $e->getQuery() !== null ? base64_encode($e->getQuery()->getSQL()) : null,
            ];
        }
    }

    // ---- providers -------------------------------------------------------
    $providers = [];
    $classes = ['Tools/DsnParserTest', 'DriverManagerTest', 'Functional/NamedParametersTest', 'Functional/DataAccessTest'];
    foreach ($classes as $path) {
        $reflection = new ReflectionClass('Doctrine\\DBAL\\Tests\\' . str_replace('/', '\\', $path));
        $out = [];
        foreach ($reflection->getMethods() as $method) {
            foreach ($method->getAttributes() as $attribute) {
                if (!str_ends_with($attribute->getName(), '\\DataProvider')) {
                    continue;
                }
                $provider = $attribute->getArguments()[0];
                if ($provider === 'modeProvider') {
                    // closures: the modes are named below ("sql")
                    continue;
                }
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

    // ---- ddl --------------------------------------------------------------
    function column(string $name, string $type, array $options = []): Column
    {
        $editor = Column::editor()->setUnquotedName($name)->setTypeName($type);
        if (isset($options['length'])) {
            $editor->setLength($options['length']);
        }
        if (isset($options['notnull'])) {
            $editor->setNotNull($options['notnull']);
        }
        if (isset($options['autoincrement'])) {
            $editor->setAutoincrement($options['autoincrement']);
        }

        return $editor->create();
    }

    function table(string $name, array $columns, ?string $primaryKey): Table
    {
        $editor = Table::editor()->setUnquotedName($name)->setColumns(...$columns);
        if ($primaryKey !== null) {
            $editor->setPrimaryKeyConstraint(
                PrimaryKeyConstraint::editor()->setUnquotedColumnNames($primaryKey)->create(),
            );
        }

        return $editor->create();
    }

    $tables = [
        // Functional/DataAccessTest
        table('fetch_table', [
            column('test_int', Types::INTEGER),
            column('test_string', Types::STRING, ['length' => 32]),
            column('test_datetime', Types::DATETIME_MUTABLE, ['notnull' => false]),
        ], 'test_int'),
        table('fetch_table_date_math', [
            column('test_date', Types::DATE_MUTABLE),
            column('test_days', Types::INTEGER),
        ], 'test_date'),
        // Functional/ConnectionTest
        table('connection_test', [column('id', Types::INTEGER)], 'id'),
        // Functional/TransactionTest
        table('storage', [column('test_int', Types::INTEGER)], 'test_int'),
        // Functional/WriteTest
        table('write_table', [
            column('id', Types::INTEGER, ['autoincrement' => true]),
            column('test_int', Types::INTEGER),
            column('test_string', Types::STRING, ['length' => 32, 'notnull' => false]),
        ], 'id'),
        table('test_empty_identity', [column('id', Types::INTEGER, ['autoincrement' => true])], 'id'),
        // Functional/NamedParametersTest
        table('ddc1372_foobar', [
            column('id', Types::INTEGER),
            column('foo', Types::STRING, ['length' => 1]),
            column('bar', Types::STRING, ['length' => 1]),
        ], 'id'),
        // Functional/StatementTest
        table('stmt_test', [
            column('id', Types::INTEGER),
            column('name', Types::TEXT, ['notnull' => false]),
        ], null),
        table('stmt_longer_results', [
            column('param', Types::STRING, ['length' => 24]),
            column('val', Types::TEXT),
        ], null),
        // Functional/ExceptionTest
        table('duplicatekey_table', [column('id', Types::INTEGER)], 'id'),
        table('alreadyexist_table', [column('id', Types::INTEGER)], null),
        table('notnull_table', [column('id', Types::INTEGER), column('val', Types::INTEGER)], null),
        table('bad_columnname_table', [column('id', Types::INTEGER)], null),
        table('ambiguous_list_table_1', [column('id', Types::INTEGER)], null),
        table('ambiguous_list_table_2', [column('id', Types::INTEGER)], null),
        Table::editor()->setUnquotedName('unique_column_table')->setColumns(column('id', Types::INTEGER))
            ->addIndex(Index::editor()->setType(IndexType::UNIQUE)->setUnquotedColumnNames('id'))->create(),
        table('syntax_error_table', [column('id', Types::INTEGER)], null),
        table('no_connection', [column('id', Types::INTEGER)], null),
    ];
    $ddl = [];
    foreach ($tables as $table) {
        $conn = sqlite();
        $conn->createSchemaManager()->createTable($table);
        $ddl[$table->getObjectName()->toString()] = array_map(
            'base64_encode',
            $conn->fetchFirstColumn('SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY rowid'),
        );
    }

    // ---- sql --------------------------------------------------------------
    $platform = new SQLitePlatform();
    $sql = [
        'dummy_select' => $platform->getDummySelectSQL(),
        'dummy_select_foo' => $platform->getDummySelectSQL(':foo'),
        'length_length' => $platform->getDummySelectSQL(
            $platform->getLengthExpression('?') . ', ' . $platform->getLengthExpression('?'),
        ),
        'empty_identity_insert' => $platform->getEmptyIdentityInsertSQL('test_empty_identity', 'id'),
        'datetime_format' => $platform->getDateTimeFormatString(),
        // Functional/Connection/FetchTest, FetchEmptyTest
        'fetch_query' => Doctrine\DBAL\Tests\TestUtil::generateResultSetQuery(
            ['a', 'b'],
            [['foo', 1], ['bar', 2], ['baz', 3]],
            $platform,
        ),
        'fetch_empty_query' => sprintf('SELECT * FROM (%s) t WHERE 1 = 0', $platform->getDummySelectSQL('1 c')),
        'current_database' => $platform->getDummySelectSQL($platform->getCurrentDatabaseExpression()),
        'create_savepoint' => $platform->createSavePoint('DOCTRINE_2'),
        'release_savepoint' => $platform->releaseSavePoint('DOCTRINE_2'),
        'rollback_savepoint' => $platform->rollbackSavePoint('DOCTRINE_2'),
        'sqlite_date_math' => 'SELECT COUNT(*) FROM fetch_table_date_math WHERE '
            . $platform->getDateSubDaysExpression('test_date', 'test_days') . " < '2010-05-12'",
    ];
    $sql['fetch_query_limit'] = $platform->modifyLimitQuery($sql['fetch_query'], 1, 1);
    foreach (TransactionIsolationLevel::cases() as $level) {
        $sql['isolation_' . $level->name] = $platform->getSetTransactionIsolationSQL($level);
    }
    foreach ($providers['Functional/DataAccessTest']['testTrimExpression']['rows'] as $i => $row) {
        $data = DataAccessTestRows::trim()[$i];
        $sql['trim_' . $i] = 'SELECT ' . $platform->getTrimExpression($data[0], $data[1], $data[2]) . ' AS trimmed FROM fetch_table';
    }
    foreach (DataAccessTestRows::substring() as $name => [$string, $start, $length]) {
        $sql['substring_' . $name] = $platform->getDummySelectSQL($platform->getSubstringExpression($string, $start, $length));
    }
    $locate = 'SELECT ';
    $locate .= $platform->getLocateExpression('test_string', "'oo'") . ' AS locate1, ';
    $locate .= $platform->getLocateExpression('test_string', "'foo'") . ' AS locate2, ';
    $locate .= $platform->getLocateExpression('test_string', "'bar'") . ' AS locate3, ';
    $locate .= $platform->getLocateExpression('test_string', 'test_string') . ' AS locate4, ';
    $locate .= $platform->getLocateExpression("'foo'", 'test_string') . ' AS locate5, ';
    $locate .= $platform->getLocateExpression("'barfoobaz'", 'test_string') . ' AS locate6, ';
    $locate .= $platform->getLocateExpression("'bar'", 'test_string') . ' AS locate7, ';
    $locate .= $platform->getLocateExpression('test_string', "'oo'", '2') . ' AS locate8, ';
    $locate .= $platform->getLocateExpression('test_string', "'oo'", '3') . ' AS locate9, ';
    $locate .= $platform->getLocateExpression('test_string', "'foo'", '1') . ' AS locate10, ';
    $locate .= $platform->getLocateExpression('test_string', "'oo'", '1 + 1') . ' AS locate11 ';
    $locate .= 'FROM fetch_table';
    $sql['locate'] = $locate;
    // DataAccessTest::modeProvider(): how the interval is written.
    $modes = [
        'bind' => static fn (int $interval): string => '?',
        'literal' => static fn (int $interval): string => sprintf('%d', $interval),
        'expression' => static fn (int $interval): string => sprintf('(0 + %d)', $interval),
    ];
    $dates = [
        'testDateAddSeconds' => ['getDateAddSecondsExpression', 1],
        'testDateSubSeconds' => ['getDateSubSecondsExpression', 1],
        'testDateAddMinutes' => ['getDateAddMinutesExpression', 5],
        'testDateSubMinutes' => ['getDateSubMinutesExpression', 5],
        'testDateAddHours' => ['getDateAddHourExpression', 3],
        'testDateSubHours' => ['getDateSubHourExpression', 3],
        'testDateAddDays' => ['getDateAddDaysExpression', 10],
        'testDateSubDays' => ['getDateSubDaysExpression', 10],
        'testDateAddWeeks' => ['getDateAddWeeksExpression', 1],
        'testDateSubWeeks' => ['getDateSubWeeksExpression', 1],
        'testDateAddMonths' => ['getDateAddMonthExpression', 2],
        'testDateSubMonths' => ['getDateSubMonthExpression', 2],
        'testDateAddQuarters' => ['getDateAddQuartersExpression', 3],
        'testDateSubQuarters' => ['getDateSubQuartersExpression', 3],
        'testDateAddYears' => ['getDateAddYearsExpression', 6],
        'testDateSubYears' => ['getDateSubYearsExpression', 6],
    ];
    foreach ($dates as $test => [$method, $interval]) {
        foreach ($modes as $mode => $build) {
            $sql[$test . '_' . $mode] = sprintf('SELECT %s FROM fetch_table', $platform->$method('test_datetime', $build($interval)));
        }
    }
    $sql = array_map('base64_encode', $sql);

    // ---- cases ------------------------------------------------------------
    $values = [null, true, false, 0, 1, -5, PHP_INT_MAX, 1.5, 0.3, 1.0, 1e100, -0.0, 'abc', '12', '', "a\0b", [1, 2]];
    $bind = [];
    foreach (ParameterType::cases() as $type) {
        foreach ($values as $value) {
            $bind[] = [typed_value($type), typed_value($value), outcome(static function () use ($value, $type) {
                $conn = sqlite();
                $stmt = $conn->prepare('SELECT typeof(?), ?');
                @$stmt->bindValue(1, $value, $type);
                @$stmt->bindValue(2, $value, $type);

                return $stmt->executeQuery()->fetchNumeric();
            })];
        }
    }

    $cases = [
        'values' => [
            'SELECT 1, -1, 9223372036854775807, 1.5, 0.1, 1e100, -0.0, \'a\', X\'00ff\', NULL',
            'SELECT 1 AS "1", 2 AS a, 3 AS a, 4 AS "0x1", 5 AS "-1", 6 AS "01"',
            'SELECT CAST(1 AS TEXT), CAST(\'2\' AS INTEGER), CAST(\'x\' AS BLOB), 3.0, 1/2, 1.0/2',
        ],
        'errors' => [
            'SELECT * FROM no_table',
            'MUUHAAAAHAAAA',
            'SELECT no_column',
            'CREATE TABLE t (id INTEGER PRIMARY KEY); CREATE TABLE t (id INTEGER)',
            'CREATE TABLE t (id INTEGER PRIMARY KEY); INSERT INTO t (id) VALUES (1); INSERT INTO t (id) VALUES (1)',
            'CREATE TABLE t (id INTEGER NOT NULL); INSERT INTO t (id) VALUES (NULL)',
            'CREATE TABLE t (id INTEGER PRIMARY KEY); INSERT INTO t (id) VALUES (\'a\')',
            'CREATE TABLE t (id INTEGER); INSERT INTO t (nope) VALUES (1)',
            'CREATE TABLE t (id INTEGER CHECK (id > 0)); INSERT INTO t (id) VALUES (0)',
            'CREATE TABLE t (id INTEGER); CREATE TABLE u (id INTEGER); SELECT id FROM t, u',
            'SELECT 1; SELECT * FROM no_table',
            '',
        ],
    ];
    $errors = [];
    foreach ($cases['errors'] as $statement) {
        $errors[] = [base64_encode($statement), outcome(static fn () => sqlite()->executeStatement($statement)),
            outcome(static fn () => sqlite()->executeQuery($statement)->fetchAllNumeric()),
            outcome(static fn () => sqlite()->prepare($statement)->executeQuery()->fetchAllNumeric())];
    }
    $fetched = [];
    foreach ($cases['values'] as $statement) {
        $fetched[] = [base64_encode($statement),
            outcome(static fn () => sqlite()->fetchNumeric($statement)),
            outcome(static fn () => sqlite()->fetchAssociative($statement))];
    }

    // rowCount(), executeStatement() and lastInsertId() in sequence.
    $sequence = [];
    $conn = sqlite();
    $steps = [
        ['lastInsertId'],
        ['statement', 'CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT, v TEXT)'],
        ['lastInsertId'],
        ['statement', "INSERT INTO t (v) VALUES ('a'), ('b'), ('c')"],
        ['lastInsertId'],
        ['rowCount', 'SELECT * FROM t WHERE 0'],
        ['rowCount', 'SELECT * FROM t'],
        ['statement', 'CREATE TABLE u (id INTEGER)'],
        ['rowCount', 'SELECT * FROM t WHERE 0'],
        ['statement', "UPDATE t SET v = 'x' WHERE id > 1"],
        ['statement', "UPDATE t SET v = 'x' WHERE id > 1"],
        ['statement', 'DELETE FROM t WHERE id = 99'],
        ['statement', "INSERT INTO t (v) VALUES ('d'); INSERT INTO t (v) VALUES ('e'), ('f')"],
        ['lastInsertId'],
        ['statementParams', 'INSERT INTO t (v) VALUES (?)', ['g']],
        ['statementParams', 'UPDATE t SET v = ? WHERE id < ?', ['y', 3]],
        ['statementParams', 'SELECT * FROM t WHERE id = ?', [1]],
        ['statement', 'DELETE FROM t'],
        ['lastInsertId'],
        ['quote', "a'b"],
        ['quote', "a\0b'"],
        ['quote', ''],
        ['columnName', 'SELECT 1 AS a, 2 AS "b c"', 1],
        ['columnName', 'SELECT 1 AS a', 5],
        ['columnName', 'SELECT 1 AS a', -1],
        ['statement', 'DROP TABLE nope'],
        ['bindNamed', 'SELECT :foo', 'bar', 'baz'],
        ['bindNamed', 'SELECT :foo', 'foo', 'baz'],
        ['bindNamed', 'SELECT :foo', ':foo', 'baz'],
        ['bindIndex', 'SELECT ?', 2, 'baz'],
        ['bindIndex', 'SELECT 1', 1, null],
        ['statement', 'SELECT 1'],
        ['database'],
    ];
    foreach ($steps as $step) {
        $sequence[] = [typed_value($step), outcome(static function () use ($conn, $step) {
            return match ($step[0]) {
                'lastInsertId' => $conn->lastInsertId(),
                'statement' => $conn->executeStatement($step[1]),
                'statementParams' => $conn->executeStatement($step[1], $step[2]),
                'rowCount' => $conn->executeQuery($step[1])->rowCount(),
                'quote' => $conn->quote($step[1]),
                'columnName' => $conn->executeQuery($step[1])->getColumnName($step[2]),
                'bindNamed', 'bindIndex' => (static function () use ($conn, $step) {
                    $stmt = $conn->prepare($step[1]);
                    $stmt->bindValue($step[2], $step[3]);

                    return $stmt->executeQuery()->fetchAllNumeric();
                })(),
                'database' => $conn->getDatabase(),
            };
        })];
    }

    $connect = [
        outcome(static fn () => DriverManager::getConnection(['driver' => 'pdo_sqlite', 'path' => '/nonexistent/dir/db.sqlite'])->executeQuery('SELECT 1')),
        outcome(static fn () => DriverManager::getConnection(['driver' => 'pdo_sqlite', 'user' => 1, 'memory' => true])->executeQuery('SELECT 1')),
    ];

    echo json_encode([
        'providers' => $providers,
        'ddl' => $ddl,
        'sql' => $sql,
        'bind' => $bind,
        'errors' => $errors,
        'fetched' => $fetched,
        'sequence' => $sequence,
        'connect' => $connect,
    ], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";

    // The rows of the DataAccessTest providers, as arguments.
    final class DataAccessTestRows
    {
        public static function trim(): array
        {
            return Doctrine\DBAL\Tests\Functional\DataAccessTest::getTrimExpressionData();
        }

        public static function substring(): array
        {
            $rows = [];
            foreach (Doctrine\DBAL\Tests\Functional\DataAccessTest::substringExpressionProvider() as $name => $row) {
                $rows[$name] = $row;
            }

            return $rows;
        }
    }
}
