<?php
// What Symfony\Component\Config's caches and the symfony/filesystem calls
// they make answer, for crates/config/tests/data/cache.json
// (scripts/record.sh config-cache).
//
// Recorded beyond the upstream tests:
// - write(): the cache, .meta and .meta.json files (or their absence) and
//   their modes, for Config's filesystem resources, the test fixtures and
//   a Probe resource whose __serialize() returns any array (nested
//   arrays and objects, mangled and NUL-holding names, ';O:' inside
//   strings, invalid UTF-8, INF, U+2028...);
// - isFresh() after a write, with the cache file's modification time
//   moved, resources changed, and the meta file replaced by other bytes,
//   for ResourceCheckerConfigCache with no, self-checking, skipping and
//   test checkers, and ConfigCache in and out of debug: true, false or
//   the throwable [class, message] (a TypeError's ", called in" suffix,
//   which names PHP's file, is cut);
// - Filesystem::dumpFile(), readFile(), chmod() and tempnam() on missing
//   directories, files in the way, symbolic links (relative, dangling,
//   '0', chains), file:// URLs and relative paths: the files afterwards
//   or the exception [class, message].
//
// Paths: everything happens under $base, which the replay replaces with a
// directory of the same length. Temporary names (prefix + 19 random
// characters) are written as <prefix>@TMP@.

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\Config\ConfigCache;
use Symfony\Component\Config\Resource\DirectoryResource;
use Symfony\Component\Config\Resource\FileExistenceResource;
use Symfony\Component\Config\Resource\FileResource;
use Symfony\Component\Config\Resource\GlobResource;
use Symfony\Component\Config\Resource\ResourceInterface;
use Symfony\Component\Config\Resource\SelfCheckingResourceChecker;
use Symfony\Component\Config\Resource\SkippingResourceChecker;
use Symfony\Component\Config\ResourceCheckerConfigCache;
use Symfony\Component\Config\ResourceCheckerInterface;
use Symfony\Component\Config\Tests\Fixtures\ResourceWithVeryVeryVeryVeryVeryVeryVeryVeryLongName;
use Symfony\Component\Config\Tests\Resource\ResourceStub;
use Symfony\Component\Filesystem\Filesystem;

class ProbeResource implements ResourceInterface
{
    public function __construct(private array $data)
    {
    }

    public function __toString(): string
    {
        return 'probe';
    }

    public function __serialize(): array
    {
        return $this->data;
    }

    public function __unserialize(array $data): void
    {
        $this->data = $data;
    }
}

/** An object with private, protected and public properties. */
class Hidden
{
    private $secret = 's';
    protected $prot = ['p'];
    public $pub = null;
}

/** Answers what it was told; supports() only for the given class. */
class TestChecker implements ResourceCheckerInterface
{
    public function __construct(private ?string $class, private bool $fresh)
    {
    }

    public function supports(ResourceInterface $metadata): bool
    {
        return null === $this->class || $metadata::class === $this->class;
    }

    public function isFresh(ResourceInterface $resource, int $timestamp): bool
    {
        return $this->fresh;
    }
}

const T = 1000000000;
$base = '/tmp/cc-00000000';
umask(0o022);

function reset_base(): void
{
    global $base;
    exec('rm -rf '.escapeshellarg($base));
    mkdir("$base/sub", 0o777, true);
    mkdir("$base/empty");
    foreach (['a.txt' => 'a', 'b.txt' => 'b', 'sub/c.txt' => 'c'] as $f => $c) {
        file_put_contents("$base/$f", $c);
        touch("$base/$f", T);
    }
    touch("$base/sub", T);
    touch("$base/empty", T);
    touch($base, T);
    chdir($base);
}

function masked(?string $s): ?string
{
    global $base;
    if (null === $s) {
        return null;
    }

    return preg_replace('/([^\/]*?)[a-z0-9]{13}[a-zA-Z0-9]{6}(?=[",)\/]|$)/', '$1@TMP@', $s);
}

function b64(?string $s): ?string
{
    return null === $s ? null : base64_encode($s);
}

function file_state(string $path): ?array
{
    clearstatcache();
    if (!file_exists($path) && !is_link($path)) {
        return null;
    }

    return [
        'content' => is_file($path) ? base64_encode(file_get_contents($path)) : null,
        'mode' => decoct(fileperms($path) & 0o7777),
        'link' => is_link($path) ? readlink($path) : null,
    ];
}

/** [class, message] of a throwable, without PHP's location suffix. */
function thrown(\Throwable $e): array
{
    $message = preg_replace('/, called in .* on line \d+$/', '', $e->getMessage());

    return ['throws', $e::class, base64_encode(masked($message))];
}

function resource(array $spec): ResourceInterface
{
    global $base;

    return match ($spec[0]) {
        'file' => new FileResource("$base/{$spec[1]}"),
        'existence' => new FileExistenceResource("$base/{$spec[1]}"),
        'directory' => new DirectoryResource("$base/{$spec[1]}", $spec[2]),
        'glob' => new GlobResource("$base/{$spec[1]}", $spec[2], $spec[3], $spec[4]),
        'stub' => (function () use ($spec) {
            $r = new ResourceStub();
            $r->setFresh($spec[1]);

            return $r;
        })(),
        'long' => new ResourceWithVeryVeryVeryVeryVeryVeryVeryVeryLongName("$base/{$spec[1]}"),
        'probe' => new ProbeResource($spec[1]),
    };
}

/** typed_value(), with an object's properties as (array) gives them
 * (mangled names included), which is what serialize() writes. */
function typed_probe(mixed $v): array
{
    if (\is_array($v)) {
        $pairs = [];
        foreach ($v as $k => $x) {
            $pairs[] = [typed_value($k), typed_probe($x)];
        }

        return ['array', $pairs];
    }
    if (\is_object($v)) {
        $pairs = [];
        foreach ((array) $v as $k => $x) {
            $pairs[] = [typed_value((string) $k), typed_probe($x)];
        }

        return ['object', $v::class, $pairs];
    }

    return typed_value($v);
}

/** The spec for the replay: Probe data typed, strings in base64. */
function spec_json(array $spec): array
{
    if ('probe' === $spec[0]) {
        return ['probe', typed_probe($spec[1])];
    }

    return array_merge([$spec[0]], array_map(fn ($a) => \is_string($a) ? base64_encode($a) : $a, \array_slice($spec, 1)));
}

$out = ['base' => $base, 'write' => [], 'fresh' => [], 'filesystem' => []];

// --- write() -----------------------------------------------------------

$o = new \stdClass();
$o->x = 1;
$o->{"a b"} = "spaced name";
$nested = new Hidden();
$probes = [
    [],
    ['a' => 'b'],
    ['list', 'of', 'values'],
    [5 => 'int key', '7' => 'numeric string key', '' => 'empty key'],
    ["\0Some\\Class\0name" => 'private', "\0*\0prot" => 'protected', "\0\0" => 'two NULs', "\0abc" => 'one NUL', "a\0b\0c" => 'inner NULs'],
    ['s' => "\0X\0y", 't' => ';O:5:"Hello":0:{}', 'u' => 's:3:"'."\0a\0".'"'],
    ['utf8' => "é/ü\u{2028}\u{1F600}", 'ctl' => "\x01\t\n\"\\/", 'bad' => "\xff\xfe"],
    ['bad key' => 1, "\xff" => 2],
    ['f' => 0.1, 'g' => 1e100, 'h' => -0.0, 'i' => 1.0, 'j' => 7.0E-10, 'k' => PHP_INT_MAX, 'l' => PHP_INT_MIN],
    ['inf' => INF],
    ['nan' => NAN],
    ['t' => true, 'f' => false, 'n' => null],
    ['nested' => ['deep' => ['deeper' => [1, 2, [3]]]], 'empty' => [], 'map' => [1 => 'a', 0 => 'b']],
    ['object' => $o, 'emptyobject' => new \stdClass()],
    ['resource' => $nested, 'list' => [$nested, $o]],
];

$writes = [
    ['metadata' => null],
    ['metadata' => []],
    ['metadata' => [['file', 'a.txt']]],
    ['metadata' => [['file', 'a.txt'], ['file', 'sub/c.txt'], ['existence', 'a.txt'], ['existence', 'missing']]],
    ['metadata' => [['directory', 'sub', null], ['directory', 'sub', '/\.txt$/'], ['directory', 'empty', '']]],
    ['metadata' => [['glob', 'sub', '/*.txt', true, false], ['glob', '', '/*.txt', false, true]]],
    ['metadata' => [['stub', true], ['stub', false]]],
    ['metadata' => [['long', 'a.txt']]],
    ['metadata' => [['existence', "bad\xffname"]]],
    ['metadata' => [['existence', "é name"]]],
    ['metadata' => null, 'content' => "\0binary\r\n\xff"],
];
foreach ($probes as $p) {
    $writes[] = ['metadata' => [['probe', $p]]];
}
$writes[] = ['metadata' => array_map(fn ($p) => ['probe', $p], array_slice($probes, 0, 6))];

foreach ($writes as $w) {
    reset_base();
    $metadata = null === $w['metadata'] ? null : array_map('resource', $w['metadata']);
    $content = $w['content'] ?? 'content';
    $cache = new ResourceCheckerConfigCache("$base/cache.php");
    set_error_handler(fn () => true);
    try {
        $cache->write($content, $metadata);
        $result = null;
    } catch (\Throwable $e) {
        $result = thrown($e);
    }
    restore_error_handler();
    $out['write'][] = [
        'content' => base64_encode($content),
        'metadata' => null === $w['metadata'] ? null : array_map('spec_json', $w['metadata']),
        'result' => $result,
        'cache' => file_state("$base/cache.php"),
        'meta' => file_state("$base/cache.php.meta"),
        'json' => file_state("$base/cache.php.meta.json"),
    ];
}

// --- isFresh() ---------------------------------------------------------

$checkerSets = [
    'none' => fn () => [],
    'self' => fn () => [new SelfCheckingResourceChecker()],
    'skip-all' => fn () => [new SkippingResourceChecker()],
    'skip-file' => fn () => [new SkippingResourceChecker([FileResource::class]), new SelfCheckingResourceChecker()],
    'test-other-then-self' => fn () => [new TestChecker('Nope', false), new SelfCheckingResourceChecker()],
    'test-stale' => fn () => [new TestChecker(null, false), new SelfCheckingResourceChecker()],
    'test-fresh' => fn () => [new TestChecker(null, true), new SelfCheckingResourceChecker()],
    'config-debug' => null,
    'config-prod' => null,
    'config-debug-skip-file' => null,
];

$serializedFile = null;
$resourceSets = [
    'file' => [['file', 'a.txt']],
    'files' => [['file', 'a.txt'], ['file', 'sub/c.txt']],
    'existence' => [['existence', 'missing'], ['existence', 'a.txt']],
    'directory' => [['directory', 'sub', null]],
    'glob' => [['glob', 'sub', '/*.txt', true, false]],
    'stubs' => [['stub', true], ['stub', false]],
    'long' => [['long', 'a.txt']],
    'probe' => [['probe', ['a' => 1]]],
];

// changes made after write(), before isFresh()
$changes = [
    'none' => fn () => null,
    'cache older' => fn () => touch("$base/cache.php", T - 10),
    'cache same time' => fn () => touch("$base/cache.php", T),
    'touch a.txt' => fn () => touch("$base/a.txt", T + 100),
    'remove a.txt' => fn () => unlink("$base/a.txt"),
    'create missing' => fn () => touch("$base/missing", T),
    'add sub/d.txt' => function () use (&$base) { file_put_contents("$base/sub/d.txt", 'd'); touch("$base/sub/d.txt", T); touch("$base/sub", T); },
    'remove cache' => fn () => unlink("$base/cache.php"),
    'remove meta' => fn () => unlink("$base/cache.php.meta"),
    'meta is a directory' => function () use (&$base) { unlink("$base/cache.php.meta"); mkdir("$base/cache.php.meta"); },
];

// meta files written by hand, in place of write()'s
$metas = [
    '', 'garbage', 'b:0;', 'b:1;', 'i:5;', 'N;', 'd:0.5;', 's:1:"x";', 'a:0:{}',
    'a:1:{i:0;s:1:"x";}', 'a:1:{i:0;b:1;}', 'a:1:{i:0;b:0;}', 'a:1:{i:0;i:3;}', 'a:1:{i:0;N;}',
    'a:1:{i:0;a:0:{}}', 'a:1:{i:0;d:1.5;}', 'a:1:{i:0;O:8:"stdClass":0:{}}',
    'O:8:"stdClass":0:{}', 'O:8:"stdClass":1:{s:1:"a";s:1:"x";}', 'O:8:"stdClass":1:{s:1:"a";O:8:"stdClass":0:{}}',
    'a:1:{i:0;O:12:"ClassNotHere":0:{}}',
    'a:1:{i:0;O:8:"stdClass":1:{s:1:"a";O:12:"ClassNotHere":0:{}}}',
    'a:1:{s:1:"k";O:46:"Symfony\Component\Config\Resource\FileResource":1:{s:8:"resource";s:@LEN@:"@BASE@/a.txt";}}',
    'a:1:{i:0;O:46:"symfony\component\config\resource\fileresource":1:{s:8:"resource";s:@LEN@:"@BASE@/a.txt";}}',
    'a:1:{i:0;O:46:"Symfony\Component\Config\Resource\FileResource":1:{s:8:"resource";i:5;}}',
    'a:1:{i:0;O:46:"Symfony\Component\Config\Resource\FileResource":0:{}}',
    'a:1:{i:0;O:46:"Symfony\Component\Config\Resource\FileResource":1:{s:8:"resource";s:@LEN@:"@BASE@/a.txt";}}',
    'a:1:{i:0;O:46:"Symfony\Component\Config\Resource\FileResource":1:{s:8:"resource";s:@LEN@:"@BASE@/a.txt";}}junk',
    "a:1:{i:0;O:46:\"Symfony\\Component\\Config\\Resource\\FileResource\":1:{s:8:\"resource\";s:@LEN@:\"@BASE@/a.txt\";}}\n",
    'a:1:{i:0;O:46:"Symfony\Component\Config\Resource\FileResource":1:{S:8:"\72esource";s:@LEN@:"@BASE@/a.txt";}}',
    'O:46:"Symfony\Component\Config\Resource\FileResource":1:{s:8:"resource";s:@LEN@:"@BASE@/a.txt";}',
    'a:2:{i:0;O:46:"Symfony\Component\Config\Resource\FileResource":1:{s:8:"resource";s:@LEN@:"@BASE@/a.txt";}i:1;r:2;}',
    'a:1:{i:0;O:55:"Symfony\Component\Config\Resource\FileExistenceResource":2:{s:8:"resource";s:@LENM@:"@BASE@/missing";s:6:"exists";i:0;}}',
    'a:1:{i:0;O:55:"Symfony\Component\Config\Resource\FileExistenceResource":2:{s:8:"resource";s:@LENM@:"@BASE@/missing";s:6:"exists";b:1;}}',
    'a:1:{i:0;O:46:"Symfony\Component\Config\Resource\GlobResource":1:{s:6:"prefix";O:8:"stdClass":0:{}}}',
    'a:1:{i:0;E:7:"Foo:Bar";}',
    'a:1:{i:0;O:56:"Symfony\Component\Config\Tests\Resource\ResourceStub":1:{s:62:"'."\0".'Symfony\Component\Config\Tests\Resource\ResourceStub'."\0".'fresh";b:0;}}',
];

function run_fresh(string $set, array $resources, ?callable $change, ?string $meta): array
{
    global $base, $checkerSets;
    reset_base();
    $metadata = array_map('resource', $resources);
    $make = function () use ($set, $base, $checkerSets) {
        return match ($set) {
            'config-debug' => new ConfigCache("$base/cache.php", true),
            'config-prod' => new ConfigCache("$base/cache.php", false),
            'config-debug-skip-file' => new ConfigCache("$base/cache.php", true, null, [FileResource::class]),
            default => new ResourceCheckerConfigCache("$base/cache.php", $checkerSets[$set]()),
        };
    };
    (new \ReflectionClass(SelfCheckingResourceChecker::class))->getProperty('cache')->setValue(null, []);
    $make()->write('content', $metadata);
    touch("$base/cache.php", T);
    foreach (['a.txt', 'b.txt', 'sub/c.txt'] as $f) {
        touch("$base/$f", T);
    }
    touch("$base/sub", T);
    if (null !== $meta) {
        $len = strlen("$base/a.txt");
        $lenm = strlen("$base/missing");
        file_put_contents("$base/cache.php.meta", strtr($meta, ['@LEN@' => $len, '@LENM@' => $lenm, '@BASE@' => $base]));
    }
    if (null !== $change) {
        $change();
    }
    clearstatcache();
    set_error_handler(fn () => true);
    try {
        $result = typed_value($make()->isFresh());
    } catch (\Throwable $e) {
        $result = thrown($e);
    }
    restore_error_handler();

    return $result;
}

foreach ($checkerSets as $set => $_) {
    foreach ($resourceSets as $rname => $resources) {
        foreach ($changes as $cname => $change) {
            $out['fresh'][] = ['checkers' => $set, 'resources' => array_map('spec_json', $resources), 'change' => $cname, 'meta' => null,
                'result' => run_fresh($set, $resources, $change, null)];
        }
    }
    foreach ($metas as $meta) {
        $out['fresh'][] = ['checkers' => $set, 'resources' => [spec_json(['file', 'a.txt'])], 'change' => 'none', 'meta' => base64_encode($meta),
            'result' => run_fresh($set, [['file', 'a.txt']], null, $meta)];
    }
}

// --- Filesystem --------------------------------------------------------

$fsCases = [
    ['dumpFile', 'new.txt'],
    ['dumpFile', 'deep/er/new.txt'],
    ['dumpFile', 'a.txt'],
    ['dumpFile', 'a.txt/x'],
    ['dumpFile', 'a.txt/x/y'],
    ['dumpFile', 'sub'],
    ['dumpFile', 'sub/'],
    ['dumpFile', 'file://@BASE@/f.txt'],
    ['dumpFile', 'file://@BASE@/new/f.txt'],
    ['dumpFile', 'rel.txt', 'relative'],
    ['dumpFile', 'link-abs', 'link', '@BASE@/a.txt'],
    ['dumpFile', 'link-rel', 'link', 'sub/c.txt'],
    ['dumpFile', 'link-up', 'link', '../cc-00000000/b.txt'],
    ['dumpFile', 'link-dangling', 'link', 'nowhere/t.txt'],
    ['dumpFile', 'link-zero', 'link', '0'],
    ['dumpFile', 'link-dir', 'link', 'sub'],
    ['dumpFile', 'link-chain', 'chain'],
    ['dumpFile', 'sub/link-rel', 'link-in-sub', 'c.txt'],
    ['dumpFile', 'link-rel', 'relative-link', 'b.txt'],
    ['dumpFile', 'mode640.txt', 'mode', '640'],
    ['dumpFile', 'mode400.txt', 'mode', '400'],
    ['dumpFile', str_repeat('x', 70).'.txt'],
    ['readFile', 'a.txt'],
    ['readFile', 'missing'],
    ['readFile', 'sub'],
    ['readFile', 'file://@BASE@/a.txt'],
    ['readFile', 'a.txt/x'],
    ['chmod', 'a.txt', 0o640, 0],
    ['chmod', 'a.txt', 0o777, 0o027],
    ['chmod', 'missing', 0o644, 0],
    ['chmod', 'sub', 0o750, 0],
    ['chmod', 'file://@BASE@/a.txt', 0o600, 0],
    ['tempnam', '', 'pre'],
    ['tempnam', 'sub', 'pre'],
    ['tempnam', 'missing', 'pre'],
    ['tempnam', 'a.txt', 'pre'],
    ['tempnam', 'file://@BASE@/sub', 'pre'],
    ['tempnam', '@BASE@/sub', str_repeat('p', 62).' tail'],
    ['tempnam', '@BASE@/sub', 'dir/pre'],
];

function listing(string $dir): array
{
    $all = [];
    $it = new \RecursiveIteratorIterator(new \RecursiveDirectoryIterator($dir, \FilesystemIterator::SKIP_DOTS), \RecursiveIteratorIterator::SELF_FIRST);
    foreach ($it as $path => $info) {
        $all[masked(substr($path, strlen($dir) + 1))] = file_state($path);
    }
    ksort($all, \SORT_STRING);

    return $all;
}

foreach ($fsCases as $case) {
    reset_base();
    $fs = new Filesystem();
    $path = str_replace('@BASE@', $base, $case[1]);
    $abs = str_contains($path, '://') ? $path : ('relative' === ($case[2] ?? null) ? $path : "$base/$path");
    switch ($case[2] ?? null) {
        case 'link':
            symlink(str_replace('@BASE@', $base, $case[3]), $abs);
            break;
        case 'link-in-sub':
            symlink($case[3], $abs);
            break;
        case 'relative-link':
            symlink($case[3], "$base/$path");
            $abs = $path;
            break;
        case 'chain':
            symlink("$base/a.txt", "$base/link-a");
            symlink('link-a', $abs);
            break;
        case 'mode':
            file_put_contents($abs, 'old');
            chmod($abs, octdec($case[3]));
            break;
    }
    set_error_handler(fn () => true);
    try {
        $value = match ($case[0]) {
            'dumpFile' => $fs->dumpFile($abs, 'new content'),
            'readFile' => base64_encode($fs->readFile($abs)),
            'chmod' => $fs->chmod($abs, $case[2], $case[3]),
            'tempnam' => masked($fs->tempnam(str_replace('@BASE@', $base, $case[1]), $case[2])),
        };
        $result = ['ok', $value];
    } catch (\Throwable $e) {
        $result = thrown($e);
    }
    restore_error_handler();
    chdir('/');
    $out['filesystem'][] = ['call' => $case, 'result' => $result, 'files' => listing($base)];
}

exec('rm -rf '.escapeshellarg($base));
echo json_encode($out, \JSON_PRETTY_PRINT | \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR), "\n";
