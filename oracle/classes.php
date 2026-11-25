<?php
// Lists the PHP source files of one upstream package, with the classes they
// declare and their size, using PHP's own tokenizer (nothing is loaded or
// executed).
//
//   php oracle/classes.php upstream/symfony/routing
//
// Prints one row per file: <path>\t<classes>\t<code lines>. Test code is
// left out: the Tests/ and tests/ directories (matched by exact name, as in
// oracle/manifest.php), vendor/ and .git/. <classes> is the short names of
// the classes, interfaces, traits and enums the file declares, separated by
// spaces, or `-` for a file that declares none (functions, config, data).
// <code lines> counts the lines that hold a token other than whitespace, a
// comment, a docblock or the open tag; a token over several lines (a
// heredoc, a long string) counts every line it covers.

$dir = rtrim($argv[1] ?? '', '/');
if ($dir === '' || !is_dir($dir)) {
    fwrite(STDERR, "usage: php classes.php PACKAGE_DIR\n");
    exit(2);
}

$skip = ['Tests', 'tests', 'vendor', '.git'];
$paths = [];
$iterator = new RecursiveIteratorIterator(new RecursiveCallbackFilterIterator(
    new RecursiveDirectoryIterator($dir, FilesystemIterator::SKIP_DOTS),
    function (SplFileInfo $file) use ($dir, $skip): bool {
        if (!$file->isDir()) {
            return true;
        }
        $relative = substr($file->getPathname(), strlen($dir) + 1);
        // Only top-level Tests/, tests/ and vendor/; .git anywhere.
        return !in_array($relative, $skip, true) && $file->getFilename() !== '.git';
    },
));
foreach ($iterator as $file) {
    if ($file->isFile() && str_ends_with($file->getFilename(), '.php')) {
        $paths[] = substr($file->getPathname(), strlen($dir) + 1);
    }
}
$paths = array_map(fn (string $p): string => str_replace('\\', '/', $p), $paths);
sort($paths, SORT_STRING);

$ignored = [T_WHITESPACE, T_COMMENT, T_DOC_COMMENT, T_OPEN_TAG, T_INLINE_HTML];
$declares = [T_CLASS, T_INTERFACE, T_TRAIT, T_ENUM];

foreach ($paths as $path) {
    $source = file_get_contents("$dir/$path");
    if ($source === false) {
        fwrite(STDERR, "cannot read $dir/$path\n");
        exit(1);
    }
    $tokens = token_get_all($source);
    $lines = [];
    $classes = [];
    $previous = null; // last significant token
    $current = 1;     // line where the last token ends
    $count = count($tokens);
    for ($i = 0; $i < $count; $i++) {
        $token = $tokens[$i];
        if (is_string($token)) {
            // A single-character token has no line of its own: it is on
            // the line where the previous token ends.
            $lines[$current] = true;
            $previous = $token;
            continue;
        }
        [$id, $text, $line] = $token;
        $span = substr_count($text, "\n");
        $current = $line + $span;
        if (in_array($id, $ignored, true)) {
            continue;
        }
        for ($l = $line; $l <= $line + $span; $l++) {
            $lines[$l] = true;
        }
        if (in_array($id, $declares, true)) {
            $isConstant = is_array($previous) && $previous[0] === T_DOUBLE_COLON; // Foo::class
            $isAnonymous = is_array($previous) && $previous[0] === T_NEW;        // new class
            if (!$isConstant && !$isAnonymous) {
                for ($j = $i + 1; $j < $count; $j++) {
                    $next = $tokens[$j];
                    if (is_array($next) && in_array($next[0], $ignored, true)) {
                        continue;
                    }
                    if (is_array($next) && $next[0] === T_STRING) {
                        $classes[] = $next[1];
                    }
                    break;
                }
            }
        }
        $previous = $token;
    }
    $names = $classes === [] ? '-' : implode(' ', $classes);
    echo $path, "\t", $names, "\t", count($lines), "\n";
}
