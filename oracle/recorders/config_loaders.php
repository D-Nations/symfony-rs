<?php
// What Symfony Config's FileLocator, LoaderLoadException, GlobResource,
// DirectoryResource and FileLoader answer, for
// crates/config/tests/data/loaders.json (scripts/record.sh config-loaders).
//
// Recorded beyond the upstream tests:
// - parse_url($name, PHP_URL_SCHEME) and FileLocator::isAbsolutePath() over
//   a corpus of names (schemes, ports, drive letters, empty authorities);
// - LoaderLoadException's message for every kind of resource value, with and
//   without a previous exception, a source resource and a type; and
//   FileLoaderImportCircularReferenceException's;
// - FileLocator::locate() over a copy of the fixtures: results, messages and
//   getPaths();
// - GlobResource's paths and __toString() for patterns x prefixes x
//   recursive x forExclusion x excluded prefixes, with GLOB_BRACE and
//   without; its serialized hash; DirectoryResource::__toString();
// - FileLoader::import() with globs, exclusions, ignoreErrors and types.
//
// Paths: the fixtures (crates/config/tests/fixtures) are copied to $root;
// every path in the output has $root replaced by "{root}".

require '/demo/vendor/autoload.php';

use Symfony\Component\Config\Exception\FileLoaderImportCircularReferenceException;
use Symfony\Component\Config\Exception\FileLocatorFileNotFoundException;
use Symfony\Component\Config\Exception\LoaderLoadException;
use Symfony\Component\Config\FileLocator;
use Symfony\Component\Config\Loader\FileLoader;
use Symfony\Component\Config\Resource\DirectoryResource;
use Symfony\Component\Config\Resource\GlobResource;

function copy_tree(string $from, string $to): void
{
    @mkdir($to, 0777, true);
    foreach (scandir($from) as $name) {
        if ('.' === $name || '..' === $name) {
            continue;
        }
        if (is_dir("$from/$name")) {
            copy_tree("$from/$name", "$to/$name");
        } else {
            copy("$from/$name", "$to/$name");
        }
    }
}

@mkdir('/tmp/loaders');
copy_tree('/work/crates/config/tests/fixtures', '/tmp/loaders/Fixtures');
$root = realpath('/tmp/loaders/Fixtures');
chdir('/tmp/loaders');

$rel = function ($v) use (&$rel, $root) {
    if (is_string($v)) {
        return str_replace($root, '{root}', $v);
    }
    if (is_array($v)) {
        $out = [];
        foreach ($v as $k => $x) {
            $out[$rel((string) $k)] = $rel($x);
        }
        return array_is_list($v) ? array_values($out) : $out;
    }
    return $v;
};
$abs = fn (string $s) => str_replace('{root}', $root, $s);

$error = function (\Throwable $e) use ($rel) {
    $r = ['class' => $e::class, 'message' => $rel($e->getMessage())];
    if ($e instanceof FileLocatorFileNotFoundException) {
        $r['paths'] = $rel($e->getPaths());
    }
    if ($e->getPrevious()) {
        $r['previous'] = $e->getPrevious()::class;
    }
    return $r;
};

$out = [];

// --- parse_url() schemes and isAbsolutePath() -----------------------------
$names = [
    '/foo.xml', 'c:\\\\foo.xml', 'c:/foo.xml', '\\server\\foo.xml', 'https://server/foo.xml',
    'phar://server/foo.xml', 'phar:///server/foo.xml', 'foo.xml', 'my_resource', 'localhost:80',
    'localhost:80/x', 'a:123456', 'a:12345', 'a:1234567/x', 'a:123456/x', 'a:12345/x', 'c:', 'c:foo',
    'c:\\foo', 'C:\\foo', 'c:\\', 'ab:\\', '0:foo', '0://x', 'a+b.c-d:x', 'a_b:x', '1a:x', 'http:///foo',
    'file:///foo', 'FILE:///foo', 'http://', 'file://', 'x://y', 'x:/y', 'x:?y', 'x:#y', 'mailto:me@x',
    'foo/bar:baz', '@bundle/x.xml', '::', 'a::b', 'http://h:80', 'http://h:abc', 'http://[::1]/x', 'x:',
    ':x', 'a:0', 'a:00000/', 'a:000000', 'urn:isbn:1', 'javascript:x', 'ab:/', 'ab://', 'ab:///',
    'ab:////x', 'http:/x', 'http://@/x', 'http://:80/x', 'http://user@/x', 'tel:+1-800', 'data:text/plain,x',
    'phar:///', 'phar://', 'a:b:c', 'a:1:c', 'a:1/', 'a:/1', 'ab:?', 'ab://?x', 'ab://#x', 'ab://x?',
    '-a:b', '.a:b', '+a:b', 'é:x', 'a b:c', 'a:b c', "a\tb:c", 'ab://h:99999/x', 'ab://h:65536', 'ab://h:-1',
    'ab://h:', 'ab://:', 'ab://u:p@h', 'ab://u:p@', 'ab:///x?y', 'a:12:34', 'a:1x', 'a:x1', 'C:/', 'z:\\x',
    '9:/x', 'a1:/x', 'A:B',
];
$locator = new FileLocator([]);
$m = (new \ReflectionObject($locator))->getMethod('isAbsolutePath');
foreach ($names as $name) {
    $out['scheme'][] = [
        'name' => $name,
        'scheme' => @parse_url($name, \PHP_URL_SCHEME),
        'absolute' => $m->invoke($locator, $name),
    ];
}

// --- LoaderLoadException --------------------------------------------------
$resources = [
    ['string', 'resource'], ['bundle', '@resource'], ['bundle-path', '@bundle/sub/x.yaml'], ['empty-string', ''],
    ['int', 42], ['float', 4.5], ['float-int', 1.0], ['true', true], ['false', false], ['null', null],
    ['assoc', ['a' => 1, 'b' => [true, null]]], ['list', [1, 'two']], ['empty', []],
    ['object', new \stdClass()], ['inf', \INF], ['bad-utf8', "\xff"], ['bad-utf8-array', ["\xff"]],
    ['slash', ['a/b' => 'é']],
];
$previous = [null, 'There was an error.', 'There was an error', 'Two dots..', '', '.'];
foreach ($resources as [$label, $resource]) {
    foreach ($previous as $p) {
        foreach ([null, 'src.yaml'] as $source) {
            foreach ([null, 'foobar'] as $type) {
                $e = new LoaderLoadException($resource, $source, 3, null === $p ? null : new \Exception($p), $type);
                $out['loader_load'][] = [
                    'resource' => $label,
                    'previous' => $p,
                    'source' => $source,
                    'type' => $type,
                    'message' => $e->getMessage(),
                    'code' => $e->getCode(),
                ];
            }
        }
    }
}
foreach ([['a'], ['a', 'b', 'c'], [1, 'x'], ['', 'y']] as $list) {
    $e = new FileLoaderImportCircularReferenceException($list);
    $out['circular'][] = ['resources' => array_map('strval', $list), 'message' => $e->getMessage()];
}

// --- FileLocator::locate() ------------------------------------------------
$pathsets = [[], ['{root}'], ['{root}', '{root}/Again'], ['{root}/Again', '{root}'], ['{root}', '{root}'],
    ['{root}/Include', '{root}/missing']];
$lnames = ['foo.xml', 'Again/foo.xml', 'missing.xml', '', '{root}/foo.xml', '{root}/missing.xml',
    'file://{root}/foo.xml', 'Include', 'Include/', 'IncludeFile.txt', '../Fixtures/foo.xml', 'foo.xml/'];
foreach ($pathsets as $paths) {
    $locator = new FileLocator(array_map($abs, $paths));
    foreach ($lnames as $name) {
        foreach ([null, '{root}', '{root}/Again'] as $current) {
            foreach ([true, false] as $first) {
                $case = ['paths' => $paths, 'name' => $name, 'current' => $current, 'first' => $first];
                try {
                    $case['result'] = $rel($locator->locate($abs($name), null === $current ? null : $abs($current), $first));
                } catch (\Throwable $e) {
                    $case['error'] = $error($e);
                }
                $out['locate'][] = $case;
            }
        }
    }
}

// --- GlobResource ---------------------------------------------------------
$patterns = ['', '/*', '/**', '/**/*', '/*/', '/Resource', '/Resource/', '/**/Resource', '/**/Resource/',
    '/*.xml', '/*/*.txt', '/**/*.txt', '/{Include,Exclude}', '/{Include,Exclude}/*', '/*{/*/*.txt,.x{m,n}l}',
    '/*{/*/*.txt,.x{m,nl}', '/Include/**/{ExcludeFile.txt}', '/Include/{Exclude*.txt}', '/[IE]*', '/[!IE]*',
    '/?oo.xml', '/.hidden*', '/Resource/.*', '/Res*/*', '/Include/**', '/**/', '/Exclude', '/*Exclude*',
    '/*Exclude/*', '/\{x\}', '/**/\{x\}', '/Include/SubDir/../*', 'Include', '/**/SubDir/*', '/*/**/*.txt'];
$prefixes = ['{root}', '{root}/foo.xml', '{root}/Include'];
$excludes = [[], ['{root}/Exclude' => true], ['{root}/Include/SubDir' => true]];
$p = new \ReflectionProperty(GlobResource::class, 'globBrace');
foreach ($prefixes as $prefix) {
    foreach ($patterns as $pattern) {
        foreach ([true, false] as $recursive) {
            foreach ([false, true] as $forExclusion) {
                foreach ($excludes as $ei => $excluded) {
                    if ($forExclusion && $ei) {
                        continue;
                    }
                    $ex = [];
                    foreach ($excluded as $k => $v) {
                        $ex[$abs($k)] = $v;
                    }
                    foreach ([true, false] as $brace) {
                        $case = ['prefix' => $prefix, 'pattern' => $pattern, 'recursive' => $recursive,
                            'forExclusion' => $forExclusion, 'excluded' => array_keys($excluded), 'brace' => $brace];
                        try {
                            $r = new GlobResource($abs($prefix), $pattern, $recursive, $forExclusion, $ex);
                            if (!$brace) {
                                $p->setValue($r, 0);
                            }
                            $case['string'] = $rel((string) $r);
                            $case['paths'] = $rel(array_keys(iterator_to_array($r)));
                        } catch (\Throwable $e) {
                            $case['error'] = $error($e);
                        }
                        $out['glob'][] = $case;
                    }
                }
            }
        }
    }
}
foreach (['{root}/missing', ''] as $prefix) {
    try {
        new GlobResource($abs($prefix), '/*', true);
        $out['glob_new'][] = ['prefix' => $prefix, 'ok' => true];
    } catch (\Throwable $e) {
        $out['glob_new'][] = ['prefix' => $prefix, 'error' => $error($e)];
    }
}

// hash('xxh128') as GlobResource and DirectoryResource use it
foreach (['', 'a', "x\n", str_repeat('abc', 100), str_repeat("\xff", 300), 'hello world'] as $s) {
    $out['xxh128'][] = ['data' => base64_encode($s), 'hash' => hash('xxh128', $s)];
}
foreach ([[null], ['/.xml$/'], ['0']] as [$pattern]) {
    $r = new DirectoryResource($root, $pattern);
    $out['directory'][] = ['pattern' => $pattern, 'string' => (string) $r,
        'serialized' => serialize([$root, $pattern]) === serialize([$r->getResource(), $r->getPattern()])];
}

// --- FileLoader::import() -------------------------------------------------
class RecordingFileLoader extends FileLoader
{
    public function load(mixed $resource, ?string $type = null): mixed
    {
        return $resource;
    }

    public function supports(mixed $resource, ?string $type = null): bool
    {
        return true;
    }
}

$iresources = ['Include/*', 'Include/', 'Include', 'Include/**/*', 'ExcludeTrailingSlash/*', '*.xml', 'foo.xml',
    'missing/*', 'missing.xml', 'Again', './*.xml', '*', '**/*.txt', 'Exclude/**/*', '[foo]', 'foo/bar[foo]',
    '{root}/Include/*', 'Include/{IncludeFile.txt,SubDir}', "two\nlines*"];
$iexcludes = [null, '{root}/Include/{ExcludeFile.txt}', '{root}/Include/**/{Exclude*.txt}',
    ['{root}/Exclude', '{root}/Include/SubDir'], '{root}/missing/*', 'Include/SubDir'];
foreach ($iresources as $resource) {
    foreach ($iexcludes as $exclude) {
        foreach ([false, true] as $ignore) {
            foreach ([null, 'glob'] as $type) {
                $loader = new RecordingFileLoader(new FileLocator([$root]));
                $case = ['resource' => $resource, 'exclude' => $exclude, 'ignore' => $ignore, 'type' => $type];
                $x = null === $exclude ? null : (is_array($exclude) ? array_map($abs, $exclude) : $abs($exclude));
                try {
                    $case['result'] = $rel($loader->import($abs($resource), $type, $ignore, 'src.yaml', $x));
                } catch (\Throwable $e) {
                    $case['error'] = $error($e);
                }
                $out['import'][] = $case;
            }
        }
    }
}

echo json_encode($out, \JSON_PRETTY_PRINT | \JSON_UNESCAPED_SLASHES | \JSON_INVALID_UTF8_SUBSTITUTE | \JSON_PRESERVE_ZERO_FRACTION), "\n";
