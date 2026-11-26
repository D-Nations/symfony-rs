<?php
// PHP's session extension with NativeSessionStorage and
// PhpBridgeSessionStorage, for
// crates/http-foundation/tests/data/native_session.json
// (scripts/record.sh http-foundation-native-session): 1,500 seeded
// scenarios, each run as a request of PHP's built-in web server (the
// session cookie and the cache limiter's headers exist only there) by
// http_foundation_native_session_server.php.
//
// A scenario has a request cookie (an id the store or a session file
// has, one it lacks, ids that are short or not file names, none), the
// session.* ini settings, a save handler (see the server), the handler
// calls that answer false, a mode (session_* calls, NativeSessionStorage
// with options, PhpBridgeSessionStorage on a session PHP may have
// started) and 3 to 12 operations. The clock MetadataBag reads moves on
// between them when an operation says so.
//
// The transcripts are normalized, walking log, headers, status, id,
// session, store, then files: an id PHP generated becomes {id1}, {id2}...
// in order of first appearance (one only a session file names, {file});
// the save path {dir}; dates their shape (digits 9, names Xxx); where
// output or the session started {file}/{line}.

$address = '127.0.0.1:8121';
$router = __DIR__.'/http_foundation_native_session_server.php';
$server = proc_open(
    [PHP_BINARY, '-d', 'display_errors=0', '-d', 'log_errors=0', '-S', $address, $router],
    [0 => ['file', '/dev/null', 'r'], 1 => ['file', '/dev/null', 'w'], 2 => ['file', '/dev/null', 'w']],
    $pipes,
);
for ($try = 0; $try < 100; ++$try) {
    $socket = @fsockopen('127.0.0.1', 8121);
    if (false !== $socket) {
        fclose($socket);
        break;
    }
    usleep(50_000);
}

mt_srand(20261126);
$pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];
$chance = static fn (int $in) => 0 === mt_rand(0, $in - 1);

const SEED_A = '0123456789abcdef0123456789abcdef';
const SEED_B = 'fedcba9876543210fedcba9876543210';

$phpData = ['x|i:1;', '', 'a|s:1:"b";n|N;', '_sf2_attributes|a:1:{s:3:"foo";s:3:"bar";}_sf2_meta|a:3:{s:1:"u";i:1699999000;s:1:"c";i:1699990000;s:1:"l";i:0;}', 'garbage', '_symfony_flashes|a:1:{s:4:"note";a:1:{i:0;s:2:"hi";}}'];
$serializeData = ['a:1:{s:1:"x";i:1;}', '', 'i:5;', 'garbage'];
$keys = ['k', 'x', '5', 'p|q', '_sf2_attributes'];
$values = ['v', 1, null, true, [1, 2], ['k' => 'v'], ''];

$scenarios = [];
for ($n = 0; $n < 1500; ++$n) {
    $ini = [['name', 'sid'], ['gc_probability', '0']];
    foreach ([
        'use_strict_mode' => ['0', '1'],
        'lazy_write' => ['0', '1'],
        'use_cookies' => ['1', '1', '1', '0'],
        'cache_limiter' => ['', 'nocache', 'private', 'public', 'private_no_expire', 'weird'],
        'cache_expire' => ['0', '30', '180'],
        'cookie_lifetime' => ['0', '0', '3600'],
        'cookie_path' => ['/', '/app', ''],
        'cookie_domain' => ['', 'example.com'],
        'cookie_secure' => ['0', '1'],
        'cookie_httponly' => ['0', '1'],
        'cookie_samesite' => ['', 'Lax', 'Strict', 'None'],
        'serialize_handler' => ['php', 'php', 'php', 'php_serialize'],
    ] as $k => $choices) {
        if ($chance(2)) {
            $ini[] = [$k, $pick($choices)];
        }
    }
    if ($chance(8)) {
        $ini[] = ['gc_probability', '1'];
        $ini[] = ['gc_divisor', '1'];
    }
    $serializer = 'php';
    foreach ($ini as [$k, $v]) {
        if ('serialize_handler' === $k) {
            $serializer = $v;
        }
    }
    $data = 'php' === $serializer ? $pick($phpData) : $pick($serializeData);

    $handler = $pick(['test', 'test', 'bare', 'bare_false', 'bare_ts', 'bare_id', 'strict_bare', 'migrating', 'files', 'files', 'null', 'none', 'none']);
    $mode = $pick(['raw', 'raw', 'native', 'native', 'native', 'bridge']);
    $store = [];
    $files = (object) [];
    if (\in_array($handler, ['files', 'none'], true)) {
        $files = [SEED_A => $data];
    } else {
        $store = ['a' => [SEED_A => $data]];
        if ('migrating' === $handler) {
            $store['b'] = [SEED_A => $data];
        }
    }
    $fail = $chance(6) ? [$pick(['open', 'write', 'destroy', 'close', 'doWrite', 'doDestroy'])] : [];

    $options = [];
    if ('native' === $mode) {
        foreach ([
            'cache_limiter' => ['', 'nocache', 'private'],
            'use_strict_mode' => [0, 1, '1', true],
            'lazy_write' => [0, 1],
            'cookie_lifetime' => [0, 3600, '0'],
            'cookie_samesite' => ['lax', ''],
            'cookie_secure' => ['auto', true, false],
            'name' => ['sid', 'other_sid', '1'],
            'gc_probability' => [0],
            'serialize_handler' => ['php', 'php_serialize', 'nope'],
            'bogus' => [1],
        ] as $k => $choices) {
            if ($chance(3)) {
                $options[$k] = $pick($choices);
            }
        }
    }

    $raw = [
        ['start'], ['start'], ['start'], ['start'],
        ['write_close'], ['write_close'],
        ['regenerate', true], ['regenerate', false],
        ['destroy'], ['abort'], ['reset'],
        ['set', 'k', 'v'], ['set', 'x', 1], ['set', 'p|q', 1], ['set', '5', 2], ['set', '_sf2_attributes', 'tampered'],
        ['unset', 'x'], ['unset', 'k'], ['assign', ['k' => 'w']], ['assign', []],
        ['encode'], ['decode', 'y|i:2;'], ['decode', 'bad'], ['decode', 'a:1:{s:1:"z";i:3;}'], ['decode', ''],
        ['id'], ['set_id', 'chosen'], ['set_id', SEED_A], ['set_id', ''], ['set_id', 'bad!id'],
        ['name'], ['set_name', 'sid'], ['set_name', 'other'], ['set_name', '1'],
        ['create_id', ''], ['create_id', 'p-'], ['create_id', 'bad!'],
        ['gc'], ['status'], ['session'], ['session'],
        ['setcookie', 'o', 'v'],
        ['ini', 'use_strict_mode', '1'], ['ini', 'name', 'x'], ['ini', 'cache_limiter', 'private'], ['ini', 'serialize_handler', 'nope'], ['ini', 'lazy_write', '0'],
        ['tick', 30],
    ];
    $stored = [
        ['s_start'], ['s_start'], ['s_start'],
        ['s_save'], ['s_save'],
        ['s_regenerate', false, null], ['s_regenerate', true, null], ['s_regenerate', false, 3600], ['s_regenerate', false, 0],
        ['s_clear'], ['s_set_id', 'chosen'], ['s_set_id', SEED_A], ['s_get_id'], ['s_set_name', 'other'], ['s_get_name'], ['s_is_started'],
        ['s_attr_set', 'k', 'v'], ['s_attr_set', 'n', 1], ['s_attr_set', 'k', null], ['s_attr_get', 'k'], ['s_attr_get', 'foo'], ['s_attr_all'],
        ['s_flash_add', 'note', 'hi'], ['s_flash_get', 'note'],
        ['s_meta'], ['s_set_options', ['cookie_lifetime' => 60, 'cookie_secure' => 'auto']], ['s_register_bag'], ['s_handler_name'],
        ['start'], ['write_close'], ['session'], ['set', 'k', 'v'], ['regenerate', true], ['tick', 30], ['tick', 30],
    ];
    $ops = [];
    for ($i = mt_rand(3, 12); $i > 0; --$i) {
        $ops[] = 'raw' === $mode ? $pick($raw) : $pick($stored);
    }
    if ($chance(30)) {
        array_splice($ops, mt_rand(0, \count($ops)), 0, [['echo']]);
    }

    $scenarios[] = [
        'n' => $n,
        'cookie' => $pick([null, 'sid='.SEED_A, 'sid='.SEED_B, 'sid=abc', 'sid=bad%21chars', 'sid=', 'other=1', 'sid='.SEED_A]),
        'ini' => $ini,
        'handler' => $handler,
        'mode' => $mode,
        'options' => (object) $options,
        'bridge_started' => $chance(2),
        'store' => (object) $store,
        'files' => $files,
        'fail' => $fail,
        'ops' => $ops,
    ];
}

/** Normalizes the strings of a transcript, in the order they are walked. */
final class TranscriptNormalizer
{
    private array $ids = [];

    public function __construct(private string $dir)
    {
    }

    public function text(string $s, bool $file = false): string
    {
        $s = str_replace($this->dir, '{dir}', $s);
        $s = preg_replace_callback('/(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])/', function ($m) use ($file) {
            if (SEED_A === $m[0] || SEED_B === $m[0]) {
                return $m[0];
            }
            if (!isset($this->ids[$m[0]])) {
                if ($file) {
                    return '{file}';
                }
                $this->ids[$m[0]] = '{id'.(\count($this->ids) + 1).'}';
            }

            return $this->ids[$m[0]];
        }, $s);
        $s = preg_replace_callback('/(expires=|Expires: )([^;]+)/', static fn ($m) => str_contains($m[2], '1970') || str_contains($m[2], '1981') ? $m[0] : $m[1].preg_replace(['/[A-Z][a-z]{2}/', '/[0-9]/'], ['Xxx', '9'], $m[2]), $s);
        $s = preg_replace('/by "[^"]*" at line \d+/', 'by "{file}" at line {line}', $s);
        $s = preg_replace('/started from .* on line \d+\)/', 'started from {file} on line {line})', $s);

        return preg_replace('/output started at [^)]*/', 'output started at {output}', $s);
    }

    public function walk(mixed $v, bool $file = false): mixed
    {
        if (\is_string($v)) {
            return $this->text($v, $file);
        }
        if (\is_array($v)) {
            $out = [];
            foreach ($v as $k => $item) {
                $out[\is_string($k) ? $this->text($k, $file) : $k] = $this->walk($item, $file);
            }

            return $out;
        }

        return $v;
    }
}

$out = [];
foreach ($scenarios as $s) {
    $header = "Content-Type: application/json\r\n".(null !== $s['cookie'] ? "Cookie: {$s['cookie']}\r\n" : '');
    $context = stream_context_create(['http' => ['method' => 'POST', 'header' => $header, 'content' => json_encode($s), 'ignore_errors' => true]]);
    $answer = file_get_contents("http://$address/", false, $context);
    $lines = explode("\n", rtrim($answer));
    $result = json_decode(end($lines), true, 512, \JSON_THROW_ON_ERROR);
    $normalizer = new TranscriptNormalizer('/tmp/native-session-'.$s['n']);
    $normalized = [];
    foreach (['log', 'headers', 'status', 'id', 'session', 'store'] as $field) {
        $normalized[$field] = $normalizer->walk($result[$field]);
    }
    $files = $normalizer->walk($result['files'], true);
    sort($files);
    $normalized['files'] = $files;
    $out[] = ['scenario' => $s, 'result' => $normalized];
}
proc_terminate($server);
proc_close($server);
echo json_encode(['scenarios' => $out], \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR), "\n";
