<?php
// symfony/password-hasher and the PHP functions it calls, as vectors for
// crates/password-hasher/tests/data/php.json (scripts/record.sh
// password-hasher). Loads the pinned upstream/symfony/password-hasher
// checkout only (no Security Core class is needed), so it runs on the
// official php:8.4.27-cli-bookworm image, the oracle image's base, which
// has sodium 1.0.18 and libargon2 (PASSWORD_ARGON2I/ID).
//
// Deterministic sections (digests, PBKDF2, crypt() with a given salt,
// sodium_crypto_pwhash() with a given salt) re-record byte for byte.
// password_hash() and sodium_crypto_pwhash_str() salt at random, so the
// "made" hashes change on each recording; tests only verify them.
//
// When crates/password-hasher/tests/data/rust_hashes.json exists (written
// by `cargo test -p symfony-password-hasher --test php -- --ignored
// write_rust_hashes`), PHP verifies those hashes too ("rust_hashes").

spl_autoload_register(static function (string $class): void {
    $prefix = 'Symfony\\Component\\PasswordHasher\\';
    if (str_starts_with($class, $prefix)) {
        $file = '/work/upstream/symfony/password-hasher/'.str_replace('\\', '/', substr($class, strlen($prefix))).'.php';
        if (is_file($file)) {
            require $file;
        }
    }
});

use Symfony\Component\PasswordHasher\Hasher\MessageDigestPasswordHasher;
use Symfony\Component\PasswordHasher\Hasher\MigratingPasswordHasher;
use Symfony\Component\PasswordHasher\Hasher\NativePasswordHasher;
use Symfony\Component\PasswordHasher\Hasher\PasswordHasherFactory;
use Symfony\Component\PasswordHasher\Hasher\Pbkdf2PasswordHasher;
use Symfony\Component\PasswordHasher\Hasher\PlaintextPasswordHasher;
use Symfony\Component\PasswordHasher\Hasher\SodiumPasswordHasher;
use Symfony\Component\PasswordHasher\PasswordHasherInterface;

/// The result of $f, or ['throws' => class, 'message' => text].
function outcome(callable $f): array
{
    try {
        return ['value' => $f()];
    } catch (\Throwable $e) {
        return ['throws' => $e::class, 'message' => $e->getMessage()];
    }
}

/// A hasher's class and settings, read by reflection, for factory cases.
function describe(PasswordHasherInterface $hasher): array
{
    $props = [];
    foreach ((new \ReflectionObject($hasher))->getProperties() as $p) {
        $v = $p->getValue($hasher);
        if ($v instanceof PasswordHasherInterface) {
            $v = describe($v);
        } elseif (is_array($v)) {
            $v = array_map(static fn ($x) => $x instanceof PasswordHasherInterface ? describe($x) : $x, $v);
        }
        $props[$p->getName()] = $v;
    }
    unset($props['hashLength'], $props['encodedLength']);

    return ['class' => (new \ReflectionClass($hasher))->getShortName()] + $props;
}

$passwords = ['password', '', '0', 'foo', 'é', "a\0b", 'Ünïcödé ✓', str_repeat('a', 72), str_repeat('a', 73), str_repeat('x', 100)];
$salts = [null, '', '0', 'salt', 'NaCl é', '{', 'a}b'];
$digestAlgos = ['md5', 'sha1', 'sha224', 'sha256', 'sha384', 'sha512/224', 'sha512/256', 'sha512'];

$out = ['hash_algos' => hash_algos(), 'sodium' => SODIUM_LIBRARY_VERSION];

// MessageDigestPasswordHasher::hash() and verify()
foreach ($digestAlgos as $algo) {
    foreach ([true, false] as $b64) {
        foreach ([1, 2, 5000, 0, -1] as $it) {
            $h = new MessageDigestPasswordHasher($algo, $b64, $it);
            foreach (['password', '', 'é'] as $pw) {
                foreach ($salts as $salt) {
                    $out['digest'][] = ['algorithm' => $algo, 'base64' => $b64, 'iterations' => $it, 'password' => $pw, 'salt' => $salt,
                        'hash' => outcome(static fn () => $h->hash($pw, $salt))];
                }
            }
        }
    }
}
foreach (['foobar', 'md4', 'sha3-256', 'whirlpool', 'crc32', 'SHA256'] as $algo) {
    $h = new MessageDigestPasswordHasher($algo, true, 1);
    $out['digest_unsupported'][] = ['algorithm' => $algo, 'hash' => outcome(static fn () => $h->hash('password', 'salt')),
        'verify' => outcome(static fn () => $h->verify('', 'password', 'salt'))];
}
$h = new MessageDigestPasswordHasher('sha256', false, 1);
$good = $h->hash('password', 'salt');
foreach ([[$good, 'password', 'salt'], [$good, 'password', null], [$good, 'other', 'salt'], [strtoupper($good), 'password', 'salt'],
    [substr($good, 1), 'password', 'salt'], ['$'.substr($good, 1), 'password', 'salt'], [$good, 'password', '{'],
    [str_repeat('a', 63), 'password', '{'], [$good, str_repeat('a', 4097), 'salt'], [$good, str_repeat('a', 4096), 'salt']] as [$hash, $pw, $salt]) {
    $out['digest_verify'][] = ['hash' => $hash, 'password' => $pw, 'salt' => $salt, 'verify' => outcome(static fn () => $h->verify($hash, $pw, $salt))];
}

// Pbkdf2PasswordHasher
foreach ($digestAlgos as $algo) {
    foreach ([true, false] as $b64) {
        foreach ([1, 2, 1000] as $it) {
            foreach ([40, 0, 1, 20, 64, 100] as $len) {
                $h = new Pbkdf2PasswordHasher($algo, $b64, $it, $len);
                foreach (['password', ''] as $pw) {
                    foreach ([null, '', 'salt', '{x}'] as $salt) {
                        $out['pbkdf2'][] = ['algorithm' => $algo, 'base64' => $b64, 'iterations' => $it, 'length' => $len, 'password' => $pw, 'salt' => $salt,
                            'hash' => outcome(static fn () => $h->hash($pw, $salt))];
                    }
                }
            }
        }
    }
}
foreach ([['sha256', true, 0, 40], ['sha256', true, -5, 40], ['sha256', true, 1, -1], ['foobar', true, 0, -1], ['crc32', true, 1, 40]] as $args) {
    $out['pbkdf2_construct'][] = ['arguments' => $args, 'construct' => outcome(static function () use ($args) {
        $h = new Pbkdf2PasswordHasher(...$args);

        return $h->verify('', 'password', 'salt');
    })];
}
$h = new Pbkdf2PasswordHasher('sha256', true, 1, 40);
$good = $h->hash('password', 'salt');
foreach ([[$good, 'password', 'salt'], [$good, 'password', null], [$good, 'password', '{'], [substr($good, 0, -1).'$', 'password', 'salt'],
    [$good, str_repeat('a', 4097), 'salt']] as [$hash, $pw, $salt]) {
    $out['pbkdf2_verify'][] = ['hash' => $hash, 'password' => $pw, 'salt' => $salt, 'verify' => outcome(static fn () => $h->verify($hash, $pw, $salt))];
}

// PlaintextPasswordHasher
foreach ([false, true] as $ignore) {
    $h = new PlaintextPasswordHasher($ignore);
    foreach ([['foo', 'foo', null], ['FOO', 'foo', ''], ['foo{salt}', 'foo', 'salt'], ['FOO{SALT}', 'foo', 'salt'], ['foo', 'foo', '0'],
        ['ÉTÉ', 'été', null], ['été', 'été', null], ['foo', 'foo', '{'], ['x', str_repeat('a', 4097), null]] as [$hash, $pw, $salt]) {
        $out['plaintext'][] = ['ignore_case' => $ignore, 'hash' => $hash, 'password' => $pw, 'salt' => $salt,
            'hashed' => outcome(static fn () => $h->hash($pw, $salt)),
            'verify' => outcome(static fn () => $h->verify($hash, $pw, $salt))];
    }
}

// crypt() for the formats password_verify() hands to it
$settings = ['$5$abc', '$5$abc$', '$5$rounds=5000$abc', '$5$rounds=1000$abc$', '$5$rounds=999$abc', '$5$rounds=10$abc',
    '$5$rounds=100000$x', '$5$rounds=1000000000$abc', '$5$rounds=01000$abc', '$5$rounds=abc$def', '$5$rounds=$abc', '$5$rounds=1000',
    '$5$abcdefghijklmnopqrstu$', '$5$abcdefghijklmnop', '$5$', '$5$$', '$5$ab$cd', '$5$é',
    '$6$abc', '$6$rounds=5000$abc', '$6$rounds=1234$saltsaltsaltsalt$', '$6$abcdefghijklmnopq', '$6$rounds=10$abc',
    '$1$abc$', '$1$abcdefghij', '$1$', '$1$$', '$1$ab$cd',
    '$2y$04$abcdefghijklmnopqrstuu', '$2y$04$abcdefghijklmnopqrstuv', '$2a$04$abcdefghijklmnopqrstuu', '$2b$04$abcdefghijklmnopqrstuu',
    '$2x$04$abcdefghijklmnopqrstuu', '$2y$4$abcdefghijklmnopqrstuu', '$2y$03$abcdefghijklmnopqrstuu', '$2y$32$abcdefghijklmnopqrstuu',
    '$2y$04$abcdefghijklmnopqrstu', '$2y$04$abcdefghijklmnopqrstuuXYZ', '$2c$04$abcdefghijklmnopqrstuu', '$2y$04$abcdefghijklmnopqrst!u',
    'ab', '_J9..rasm', '$3$abc', '$7$abc', '*0', ''];
foreach ($settings as $s) {
    foreach (['password', '', 'é', 'Ünïcödé ✓', "a\0b", 'x', str_repeat('a', 72), str_repeat('a', 73)] as $pw) {
        $out['crypt'][] = ['password' => $pw, 'setting' => $s, 'crypt' => crypt($pw, $s)];
    }
}
// password_verify() on crypt() results, including passwords cut at a NUL
foreach (['$5$abc', '$6$abc', '$1$abc$', '$2y$04$abcdefghijklmnopqrstuu', '$2a$04$abcdefghijklmnopqrstuu', 'ab'] as $s) {
    foreach (['password', 'a', 'é'] as $made) {
        $hash = crypt($made, $s);
        foreach (['password', 'a', "a\0b", 'é', ''] as $pw) {
            $out['password_verify'][] = ['hash' => $hash, 'password' => $pw, 'verify' => password_verify($pw, $hash)];
        }
    }
}

// sodium_crypto_pwhash() with a given salt: Argon2 outputs
foreach ([[SODIUM_CRYPTO_PWHASH_ALG_ARGON2ID13, 'argon2id'], [SODIUM_CRYPTO_PWHASH_ALG_ARGON2I13, 'argon2i']] as [$alg, $name]) {
    foreach ([[3, 10240], [4, 11000], [3, 65536], [4, 1048576]] as [$ops, $mem]) {
        foreach (['password', 'é', "a\0b", ''] as $pw) {
            $salt = substr(hash('sha256', $name.$ops.$mem.$pw, true), 0, 16);
            $out['argon2_raw'][] = ['algorithm' => $name, 'ops' => $ops, 'mem' => $mem, 'password' => $pw, 'salt' => bin2hex($salt),
                'out' => bin2hex(sodium_crypto_pwhash(32, $pw, $salt, $ops, $mem, $alg))];
        }
    }
}

// Hashes made by PHP: Symfony's hashers and password_hash()
$made = [];
foreach (['password', '', 'é', "a\0b", str_repeat('a', 72), str_repeat('a', 73), str_repeat('x', 100), str_repeat('z', 4096)] as $pw) {
    $made[] = ['by' => 'NativePasswordHasher(cost 4)', 'password' => $pw, 'hash' => (new NativePasswordHasher(null, null, 4))->hash($pw)];
    $made[] = ['by' => 'NativePasswordHasher(3, 10240, 4, argon2i)', 'password' => $pw, 'hash' => (new NativePasswordHasher(3, 10240, 4, 'argon2i'))->hash($pw)];
    $made[] = ['by' => 'NativePasswordHasher(3, 10240, 4, argon2id)', 'password' => $pw, 'hash' => (new NativePasswordHasher(3, 10240, 4, 'argon2id'))->hash($pw)];
    $made[] = ['by' => 'SodiumPasswordHasher(3, 10240)', 'password' => $pw, 'hash' => (new SodiumPasswordHasher(3, 10240))->hash($pw)];
}
foreach (['password', 'é'] as $pw) {
    $made[] = ['by' => 'password_hash(bcrypt, cost 4)', 'password' => $pw, 'hash' => password_hash($pw, PASSWORD_BCRYPT, ['cost' => 4])];
    $made[] = ['by' => 'password_hash(argon2id, p=2)', 'password' => $pw, 'hash' => password_hash($pw, PASSWORD_ARGON2ID, ['time_cost' => 3, 'memory_cost' => 16, 'threads' => 2])];
    $made[] = ['by' => 'password_hash(argon2i, p=3)', 'password' => $pw, 'hash' => password_hash($pw, PASSWORD_ARGON2I, ['time_cost' => 1, 'memory_cost' => 24, 'threads' => 3])];
}
$made[] = ['by' => 'password_hash(bcrypt, cost 4)', 'password' => str_repeat('a', 100), 'hash' => password_hash(str_repeat('a', 100), PASSWORD_BCRYPT, ['cost' => 4])];
$native = new NativePasswordHasher();
$sodium = new SodiumPasswordHasher(3, 10240);
foreach ($made as $i => $m) {
    $made[$i]['native_verify'] = $native->verify($m['hash'], $m['password']);
    $made[$i]['sodium_verify'] = $sodium->verify($m['hash'], $m['password']);
    $made[$i]['native_verify_other'] = $native->verify($m['hash'], $m['password'].'x');
}
$out['made'] = $made;

// Argon2 strings as sodium_crypto_pwhash_str_verify() and password_verify() read them
$base = sodium_crypto_pwhash_str('password', 3, 10240);
[, $alg, $v, $params, $salt, $hash] = explode('$', $base);
$i = password_hash('password', PASSWORD_ARGON2I, ['time_cost' => 3, 'memory_cost' => 10, 'threads' => 1]);
[, $ialg, $iv, $iparams, $isalt, $ihash] = explode('$', $i);
$variants = [
    'base' => $base,
    'argon2i' => $i,
    'no_version' => "\$$alg\$$params\$$salt\$$hash",
    'v16' => "\$$alg\$v=16\$$params\$$salt\$$hash",
    'v20' => "\$$alg\$v=20\$$params\$$salt\$$hash",
    'v019' => "\$$alg\$v=019\$$params\$$salt\$$hash",
    'order_tm' => "\$$alg\$$v\$t=3,m=10,p=1\$$salt\$$hash",
    'no_p' => "\$$alg\$$v\$m=10,t=3\$$salt\$$hash",
    'p2' => "\$$alg\$$v\$m=10,t=3,p=2\$$salt\$$hash",
    'm010' => "\$$alg\$$v\$m=010,t=3,p=1\$$salt\$$hash",
    'm_plus' => "\$$alg\$$v\$m=+10,t=3,p=1\$$salt\$$hash",
    'extra_param' => "\$$alg\$$v\$m=10,t=3,p=1,data=abc\$$salt\$$hash",
    'space' => "\$$alg\$$v\$m=10, t=3,p=1\$$salt\$$hash",
    'salt_padded' => "\$$alg\$$v\$$params\$$salt==\$$hash",
    'hash_padded' => "\$$alg\$$v\$$params\$$salt\$$hash=",
    'salt_noncanonical' => "\$$alg\$$v\$$params\$".substr($salt, 0, -1).chr(ord(substr($salt, -1)) + 1)."\$$hash",
    'hash_truncated' => "\$$alg\$$v\$$params\$$salt\$".substr($hash, 0, 22),
    'hash_short' => "\$$alg\$$v\$$params\$$salt\$".substr($hash, 0, 20),
    'salt_short' => "\$$alg\$$v\$$params\$".substr($salt, 0, 8)."\$$hash",
    'trailing' => "$base\$",
    'trailing_text' => $base.'x',
    'argon2d' => "\$argon2d\$$v\$$params\$$salt\$$hash",
    'upper' => "\$ARGON2ID\$$v\$$params\$$salt\$$hash",
    'i_as_id' => "\$argon2id\$$iv\$$iparams\$$isalt\$$ihash",
    'empty' => '$argon2id$',
    't0' => "\$$alg\$$v\$m=10,t=0,p=1\$$salt\$$hash",
    'm0' => "\$$alg\$$v\$m=0,t=3,p=1\$$salt\$$hash",
    'huge_m' => "\$$alg\$$v\$m=99999999999,t=3,p=1\$$salt\$$hash",
];
foreach ($variants as $name => $s) {
    $out['argon2_strings'][] = ['name' => $name, 'hash' => $s,
        'sodium_verify' => outcome(static fn () => sodium_crypto_pwhash_str_verify($s, 'password')),
        'password_verify' => password_verify('password', $s),
        'sodium_needs_rehash_3_10240' => outcome(static fn () => sodium_crypto_pwhash_str_needs_rehash($s, 3, 10240)),
        'native_needs_rehash' => [
            'argon2id_3_10' => password_needs_rehash($s, PASSWORD_ARGON2ID, ['time_cost' => 3, 'memory_cost' => 10, 'threads' => 1, 'cost' => 13]),
            'argon2i_3_10' => password_needs_rehash($s, PASSWORD_ARGON2I, ['time_cost' => 3, 'memory_cost' => 10, 'threads' => 1, 'cost' => 13]),
        ],
        'info' => password_get_info($s)];
}

// needsRehash(): each hasher on each hash
$hashes = [
    'dummyhash', '', '$2y$04$abcdefghijklmnopqrstuu',
    crypt('foo', '$2y$04$abcdefghijklmnopqrstuu'), crypt('foo', '$2y$05$abcdefghijklmnopqrstuu'), crypt('foo', '$2y$13$abcdefghijklmnopqrstuu'),
    crypt('foo', '$2a$04$abcdefghijklmnopqrstuu'), crypt('foo', '$2b$04$abcdefghijklmnopqrstuu'), crypt('foo', '$2y$04$abcdefghijklmnopqrstuu').'x',
    crypt('foo', '$5$abc'), '$argon2id$v=19$m=10,t=3,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA',
    '$argon2id$v=19$m=11,t=4,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA', '$argon2id$v=19$m=65536,t=4,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA',
    '$argon2i$v=19$m=10,t=3,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA', '$argon2i$v=19$m=65536,t=4,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA',
    '$argon2id$v=19$m=10,t=3,p=2$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA', '$argon2id$v=16$m=10,t=3,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA',
    '$argon2id$m=10,t=3,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA', '$argon2id$v=19$m=10,t=3,p=1', '$argon2id$v=19$m=10,t=3,p=1$$',
    '$argon2id$v=19$m=10,t=3,p=1$c2FsdHNhbHQ$aGFzaA', '$argon2id$v=19$m=10,t=3,p=1$c2Fs$aGFzaGhhc2hoYXNoaGFzaA',
    '$argon2id$v=19$t=3,m=10,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA', '$argon2id$v=19$m=10,t=3,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA$x',
    '$argon2d$v=19$m=10,t=3,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA',
];
$natives = [
    'native(4, 11000, 4)' => [4, 11000, 4, null], 'native(5, 11000, 5)' => [5, 11000, 5, null], 'native(null, null, 13)' => [null, null, 13, null],
    'native(3, 10240, 4, argon2id)' => [3, 10240, 4, 'argon2id'], 'native(3, 10240, 4, argon2i)' => [3, 10240, 4, 'argon2i'],
    'native(4, 11264, 4, argon2id)' => [4, 11264, 4, 'argon2id'], 'native(4, 11000, 4, 3)' => [4, 11000, 4, '3'],
    'native(4, 11000, 4, foo)' => [4, 11000, 4, 'foo'], 'native(null, null, null, 2y)' => [null, null, null, '2y'],
];
foreach ($hashes as $hash) {
    $row = ['hash' => $hash];
    foreach ($natives as $name => $args) {
        $row['native'][$name] = (new NativePasswordHasher(...$args))->needsRehash($hash);
    }
    foreach ([[3, 10240], [4, 11000], [4, 11264], [4, 10240], [4, 65536 * 1024]] as [$ops, $mem]) {
        $row['sodium']["sodium($ops, $mem)"] = (new SodiumPasswordHasher($ops, $mem))->needsRehash($hash);
    }
    $out['needs_rehash'][] = $row;
}

// NativePasswordHasher and SodiumPasswordHasher constructors and hash() errors
foreach ([[2, null, null, null], [3, 10239, null, null], [3, 10240, 3, null], [3, 10240, 32, null], [null, null, 4, 'foo'], [null, null, 4, '7'],
    [null, null, 4, 'argon2d'], [null, null, 4, '2'], [3, 10240, 4, 'argon2i'], [null, null, null, null]] as $args) {
    $out['native_construct'][] = ['arguments' => $args, 'outcome' => outcome(static function () use ($args) {
        $h = new NativePasswordHasher(...$args);
        $r = new \ReflectionObject($h);

        return ['algorithm' => $r->getProperty('algorithm')->getValue($h), 'options' => $r->getProperty('options')->getValue($h),
            'hash_prefix' => outcome(static fn () => substr($h->hash('x'), 0, 7))];
    })];
}
foreach ([[2, null], [3, 10239], [3, 10240], [null, null]] as $args) {
    $out['sodium_construct'][] = ['arguments' => $args, 'outcome' => outcome(static function () use ($args) {
        $h = new SodiumPasswordHasher(...$args);
        $r = new \ReflectionObject($h);

        return ['opsLimit' => $r->getProperty('opsLimit')->getValue($h), 'memLimit' => $r->getProperty('memLimit')->getValue($h)];
    })];
}
foreach ([str_repeat('a', 4096), str_repeat('a', 4097)] as $pw) {
    $out['too_long'][] = ['length' => strlen($pw),
        'native_hash' => outcome(static fn () => strlen((new NativePasswordHasher(null, null, 4))->hash($pw))),
        'sodium_hash' => outcome(static fn () => strlen((new SodiumPasswordHasher(3, 10240))->hash($pw))),
        'digest_hash' => outcome(static fn () => (new MessageDigestPasswordHasher('sha256', false, 1))->hash($pw)),
        'pbkdf2_hash' => outcome(static fn () => (new Pbkdf2PasswordHasher('sha256', false, 1, 8))->hash($pw)),
        'plaintext_hash' => outcome(static fn () => strlen((new PlaintextPasswordHasher())->hash($pw)))];
}

// PasswordHasherFactory: configurations and the hashers they give
$factoryCases = [
    'md_class' => [['class' => MessageDigestPasswordHasher::class, 'arguments' => ['sha512', true, 5]]],
    'md_class_named_args' => [['class' => MessageDigestPasswordHasher::class, 'arguments' => ['iterations' => 3, 'algorithm' => 'sha1']]],
    'md_class_coerced' => [['class' => MessageDigestPasswordHasher::class, 'arguments' => ['sha1', 0, '7']]],
    'native_class' => [['class' => NativePasswordHasher::class, 'arguments' => []]],
    'native_class_args' => [['class' => NativePasswordHasher::class, 'arguments' => [4, 20000, 5, 'argon2id']]],
    'sodium_class' => [['class' => SodiumPasswordHasher::class, 'arguments' => []]],
    'pbkdf2_class' => [['class' => Pbkdf2PasswordHasher::class, 'arguments' => ['sha256', false, 3, 10]]],
    'plaintext_class' => [['class' => PlaintextPasswordHasher::class, 'arguments' => [true]]],
    'missing_class' => [['arguments' => []]],
    'missing_arguments' => [['class' => \stdClass::class]],
    'missing_class_options' => [['cost' => 4, 'iterations' => '5', 'key' => "a/b\u{e9}"]],
    'unknown_class' => [['class' => 'App\\NoSuchHasher', 'arguments' => []]],
    'auto' => [['algorithm' => 'auto']],
    'auto_options' => [['algorithm' => 'auto', 'cost' => 5, 'time_cost' => 3, 'memory_cost' => 10, 'iterations' => 7, 'key_length' => 20, 'encode_as_base64' => false]],
    'auto_hash_algorithm' => [['algorithm' => 'auto', 'hash_algorithm' => 'sha256']],
    'bcrypt' => [['algorithm' => 'bcrypt', 'cost' => 11]],
    'bcrypt_hash_algorithm' => [['algorithm' => 'bcrypt', 'hash_algorithm' => 'sha1', 'iterations' => 3]],
    'native' => [['algorithm' => 'native', 'time_cost' => 3, 'memory_cost' => 10, 'cost' => 5]],
    'native_memory_string' => [['algorithm' => 'native', 'memory_cost' => '12']],
    'sodium' => [['algorithm' => 'sodium', 'time_cost' => 3, 'memory_cost' => 10]],
    'argon2i' => [['algorithm' => 'argon2i', 'time_cost' => 3, 'memory_cost' => 10]],
    'argon2id' => [['algorithm' => 'argon2id', 'time_cost' => 3, 'memory_cost' => 10]],
    'pbkdf2' => [['algorithm' => 'pbkdf2']],
    'pbkdf2_options' => [['algorithm' => 'pbkdf2', 'hash_algorithm' => 'sha1', 'encode_as_base64' => false, 'iterations' => 2, 'key_length' => 8]],
    'plaintext' => [['algorithm' => 'plaintext', 'ignore_case' => true]],
    'sha1' => [['algorithm' => 'sha1', 'iterations' => 1]],
    'md5_options' => [['algorithm' => 'md5', 'encode_as_base64' => false, 'iterations' => 2]],
    'unknown_algorithm' => [['algorithm' => 'foobar']],
    'algorithm_null' => [['algorithm' => null, 'class' => PlaintextPasswordHasher::class, 'arguments' => []]],
    'algorithm_true' => [['algorithm' => true]],
    'migrate_from' => [['algorithm' => 'sodium', 'migrate_from' => ['bcrypt', 'digest_hasher', 'named_config']]],
    'migrate_from_native' => [['algorithm' => 'native', 'cost' => 4, 'migrate_from' => ['sha1']]],
    'migrate_from_empty' => [['algorithm' => 'bcrypt', 'migrate_from' => []]],
    'instance' => [['instance' => 'digest_hasher']],
    'instance_migrate_from' => [['instance' => 'digest_hasher', 'migrate_from' => ['bcrypt', 'named_config', 'md5']]],
    'service' => ['digest_hasher'],
];
foreach ($factoryCases as $name => [$config]) {
    $make = static function ($config) {
        $digest = new MessageDigestPasswordHasher('sha256');
        if ('digest_hasher' === $config) {
            $config = $digest;
        } elseif (isset($config['instance'])) {
            $config['instance'] = $digest;
        }

        return new PasswordHasherFactory([
            'digest_hasher' => $digest,
            'named_config' => ['algorithm' => 'pbkdf2', 'iterations' => 9],
            'App\\User' => $config,
        ]);
    };
    $factory = $make($config);
    $out['factory'][] = ['name' => $name, 'config' => $config,
        'hasher' => outcome(static fn () => describe($factory->getPasswordHasher('App\\User'))),
        'again' => outcome(static fn () => describe($factory->getPasswordHasher('App\\User')))];
}
foreach (['No\\Such', 'app\\user', 'digest_hasher'] as $subject) {
    $factory = new PasswordHasherFactory(['digest_hasher' => new MessageDigestPasswordHasher('sha256'), 'App\\User' => new PlaintextPasswordHasher()]);
    $out['factory_subjects'][] = ['subject' => $subject, 'hasher' => outcome(static fn () => describe($factory->getPasswordHasher($subject)))];
}

// MigratingPasswordHasher built by the factory: verify() of old hashes
$factory = new PasswordHasherFactory(['u' => ['algorithm' => 'auto', 'time_cost' => 3, 'memory_cost' => 10, 'cost' => 4]]);
$auto = $factory->getPasswordHasher('u');
foreach ([
    ['bcrypt', (new NativePasswordHasher(null, null, 4))->hash('pw')],
    ['sodium', (new SodiumPasswordHasher(3, 10240))->hash('pw')],
    ['pbkdf2', (new Pbkdf2PasswordHasher())->hash('pw')],
    ['digest', (new MessageDigestPasswordHasher())->hash('pw')],
    ['plaintext', 'pw'],
] as [$kind, $hash]) {
    $out['auto_verify'][] = ['kind' => $kind, 'hash' => $hash, 'verify' => $auto->verify($hash, 'pw'), 'needs_rehash' => $auto->needsRehash($hash)];
}

// Hashes made by the Rust port, verified here
$rustFile = '/work/crates/password-hasher/tests/data/rust_hashes.json';
if (is_file($rustFile)) {
    foreach (json_decode(file_get_contents($rustFile), true) as $r) {
        $r['php_native_verify'] = (new NativePasswordHasher())->verify($r['hash'], $r['password']);
        $r['php_sodium_verify'] = (new SodiumPasswordHasher())->verify($r['hash'], $r['password']);
        $r['php_password_verify'] = password_verify($r['password'], $r['hash']);
        $r['php_native_verify_other'] = (new NativePasswordHasher())->verify($r['hash'], $r['password'].'x');
        $r['php_info'] = password_get_info($r['hash']);
        $out['rust_hashes'][] = $r;
    }
}

echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
