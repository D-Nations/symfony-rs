<?php
// Lists every PHPUnit test method of one upstream package, using PHP's own
// tokenizer (nothing is loaded or executed).
//
//   php oracle/manifest.php upstream/symfony/routing
//
// Prints one row per test: <test file>\t<Class>::<method>. A method counts
// as a test when it is public and named test*, or carries #[Test] or an
// @test docblock tag (PHPUnit 11's rules). Methods of abstract classes are
// listed under each concrete subclass in the package, as PHPUnit runs them.

$dir = rtrim($argv[1] ?? '', '/');
if ($dir === '' || !is_dir($dir)) {
    fwrite(STDERR, "usage: php manifest.php PACKAGE_DIR\n");
    exit(2);
}
$testsDir = null;
foreach (['Tests', 'tests'] as $candidate) {
    if (is_dir("$dir/$candidate")) {
        $testsDir = "$dir/$candidate";
        break;
    }
}
if ($testsDir === null) {
    exit(0); // A package without tests has an empty manifest.
}

$paths = [];
$files = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($testsDir, FilesystemIterator::SKIP_DOTS));
foreach ($files as $file) {
    if ($file->isFile() && str_ends_with($file->getFilename(), '.php')) {
        $paths[] = $file->getPathname();
    }
}
sort($paths, SORT_STRING);

/** Next token index after $i that is not whitespace or a comment. */
function nextSignificant(array $tokens, int $i): int
{
    $n = count($tokens);
    for ($i++; $i < $n; $i++) {
        if (!$tokens[$i]->is([T_WHITESPACE, T_COMMENT, T_DOC_COMMENT])) {
            return $i;
        }
    }
    return $n;
}

/** Previous token index before $i that is not whitespace or a comment. */
function previousSignificant(array $tokens, int $i): int
{
    for ($i--; $i >= 0; $i--) {
        if (!$tokens[$i]->is([T_WHITESPACE, T_COMMENT, T_DOC_COMMENT])) {
            return $i;
        }
    }
    return -1;
}

$classes = []; // short name => list of class records
foreach ($paths as $path) {
    $tokens = PhpToken::tokenize(file_get_contents($path));
    $n = count($tokens);
    $depth = 0;
    $class = null;
    $classDepth = null;
    $attrTest = false;
    $doc = '';
    $modifiers = [];
    for ($i = 0; $i < $n; $i++) {
        $t = $tokens[$i];
        if ($t->is([T_WHITESPACE, T_COMMENT])) {
            continue;
        }
        if ($t->is(T_DOC_COMMENT)) {
            $doc = $t->text;
            continue;
        }
        if ($t->is(T_ATTRIBUTE)) {
            $text = '';
            $level = 1;
            while (++$i < $n && $level > 0) {
                if ($tokens[$i]->text === '[') {
                    $level++;
                } elseif ($tokens[$i]->text === ']') {
                    $level--;
                }
                $text .= $tokens[$i]->text;
            }
            $i--;
            // #[Test], #[\PHPUnit\Framework\Attributes\Test], #[Test, Group(...)]
            if (preg_match('/(?:^|[\s,\\\\])Test\s*(?:[\](,]|$)/', $text)) {
                $attrTest = true;
            }
            continue;
        }
        if ($t->text === '{' || $t->is([T_CURLY_OPEN, T_DOLLAR_OPEN_CURLY_BRACES])) {
            $depth++;
            $attrTest = false;
            $doc = '';
            $modifiers = [];
            continue;
        }
        if ($t->text === '}') {
            $depth--;
            if ($class !== null && $depth === $classDepth) {
                $classes[$class['short']][] = $class;
                $class = null;
            }
            continue;
        }
        if ($t->is([T_ABSTRACT, T_FINAL, T_READONLY, T_PUBLIC, T_PROTECTED, T_PRIVATE, T_STATIC])) {
            $modifiers[] = $t->id;
            continue;
        }
        if ($t->is(T_CLASS) && $class === null) {
            $p = previousSignificant($tokens, $i);
            $j = nextSignificant($tokens, $i);
            // Skip `Foo::class` and anonymous `new class`.
            if (($p >= 0 && $tokens[$p]->is([T_DOUBLE_COLON, T_NEW])) || $j >= $n || !$tokens[$j]->is(T_STRING)) {
                $modifiers = [];
                continue;
            }
            $extends = null;
            $k = $j + 1;
            for (; $k < $n && $tokens[$k]->text !== '{'; $k++) {
                if ($tokens[$k]->is(T_EXTENDS)) {
                    $m = nextSignificant($tokens, $k);
                    if ($m < $n) {
                        $parts = explode('\\', $tokens[$m]->text);
                        $extends = end($parts);
                    }
                }
            }
            $class = [
                'short' => $tokens[$j]->text,
                'file' => substr($path, strlen($dir) + 1),
                'abstract' => in_array(T_ABSTRACT, $modifiers, true),
                'extends' => $extends,
                'tests' => [],
            ];
            $classDepth = $depth;
            $modifiers = [];
            $i = $k - 1; // the class body's `{` is read next
            continue;
        }
        if ($t->is(T_FUNCTION) && $class !== null && $depth === $classDepth + 1) {
            $j = nextSignificant($tokens, $i);
            if ($j < $n && $tokens[$j]->text === '&') {
                $j = nextSignificant($tokens, $j);
            }
            $name = $j < $n ? $tokens[$j]->text : '';
            $public = !in_array(T_PROTECTED, $modifiers, true) && !in_array(T_PRIVATE, $modifiers, true);
            $abstract = in_array(T_ABSTRACT, $modifiers, true);
            $isTest = str_starts_with($name, 'test') || $attrTest || preg_match('/@test\b/', $doc) === 1;
            if ($public && !$abstract && $isTest) {
                $class['tests'][] = $name;
            }
            $attrTest = false;
            $doc = '';
            $modifiers = [];
            continue;
        }
        if ($t->text === ';' || $t->is([T_VARIABLE, T_CONST, T_CASE, T_USE])) {
            $attrTest = false;
            $doc = '';
            $modifiers = [];
        }
    }
}

// Inherited tests: walk up `extends` within the package by short name.
$rows = [];
$warnings = [];
foreach ($classes as $short => $records) {
    foreach ($records as $record) {
        if ($record['abstract']) {
            continue;
        }
        $tests = $record['tests'];
        $seen = [$short => true];
        $parent = $record['extends'];
        while ($parent !== null && isset($classes[$parent]) && !isset($seen[$parent])) {
            $seen[$parent] = true;
            if (count($classes[$parent]) > 1) {
                $warnings[] = "ambiguous parent $parent of {$record['file']}";
            }
            $tests = array_merge($tests, $classes[$parent][0]['tests']);
            $parent = $classes[$parent][0]['extends'];
        }
        foreach (array_unique($tests) as $method) {
            $rows[] = "{$record['file']}\t{$short}::{$method}";
        }
    }
}
$rows = array_unique($rows);
sort($rows, SORT_STRING);
foreach ($rows as $row) {
    echo $row, "\n";
}
foreach (array_unique($warnings) as $warning) {
    fwrite(STDERR, "warning: $warning\n");
}
