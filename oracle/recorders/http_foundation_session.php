<?php
// Session answers for what its tests leave open, for
// crates/http-foundation/tests/data/session.json
// (scripts/record.sh http-foundation-session): 1,500 seeded scripts.
// Each builds a Session on a MockArraySessionStorage or a
// MockFileSessionStorage (in a fresh directory), with an AttributeBag and
// a FlashBag or AutoExpireFlashBag (storage keys from a pool), a
// MetadataBag (update threshold, cookie lifetime) and a usage reporter
// that counts its calls. The storage may first be given session data,
// tampered or not (strings where bags expect arrays, metadata without a
// creation time). Then 4 to 20 operations from a pool: the session's
// methods, its flash bag's and its metadata bag's, setId() with ids that
// are not usable as file names, the usage index set (as
// SessionListener sets it to PHP_INT_MIN), and "reopen" (a new storage and session
// on the same id and directory, as the next request makes). The clock
// (time() in the Storage namespace) moves on 0 to 40 seconds between
// operations. Recorded: each operation's result or exception, the usage
// index and the reporter's count, and after each save the file's
// unserialized data. A generated session id is recorded as {id}; a
// TypeError loses its ", called in ... on line N" suffix.
// Texts are base64 (lib/typed.php).

namespace Symfony\Component\HttpFoundation\Session\Storage {
    function time(): int
    {
        return $GLOBALS['recorder_now'];
    }
}

namespace {
    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';
    require __DIR__.'/lib/typed.php';

    use Symfony\Component\HttpFoundation\Session\Attribute\AttributeBag;
    use Symfony\Component\HttpFoundation\Session\Flash\AutoExpireFlashBag;
    use Symfony\Component\HttpFoundation\Session\Flash\FlashBag;
    use Symfony\Component\HttpFoundation\Session\Session;
    use Symfony\Component\HttpFoundation\Session\Storage\MetadataBag;
    use Symfony\Component\HttpFoundation\Session\Storage\MockArraySessionStorage;
    use Symfony\Component\HttpFoundation\Session\Storage\MockFileSessionStorage;

    mt_srand(20261201);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];
    $b64 = static fn (string $s) => base64_encode($s);

    $keys = ['foo', 'bar', '12', '0', 'a.b', '', 'x y'];
    $types = ['notice', 'error', '0', 'warning'];
    $values = [['s', 'v'], ['s', ''], ['s', '0'], ['i', 0], ['i', 42], ['n'], ['b', true], ['b', false], ['a', [['s', 'x'], ['i', 1]]], ['a', []]];
    $ids = ['0123456789abcdef', 'abc,def-1', '../planted', '', 'a b', str_repeat('z', 246), str_repeat('z', 247)];
    $value = static function (array $spec) use (&$value) {
        return match ($spec[0]) {
            's' => base64_decode($spec[1]),
            'i' => $spec[1],
            'n' => null,
            'b' => $spec[1],
            'a' => array_map($value, $spec[1]),
        };
    };
    $specOut = static function (array $spec) use (&$specOut, $b64) {
        return match ($spec[0]) {
            's' => ['s', $b64($spec[1])],
            'a' => ['a', array_map($specOut, $spec[1])],
            default => $spec,
        };
    };

    $dir = sys_get_temp_dir().'/session-recorder';
    $normalize = static function ($v) use (&$normalize) {
        if (\is_string($v) && preg_match('/^[0-9a-f]{32}$/', $v)) {
            return '{id}';
        }
        if (\is_array($v)) {
            return array_map($normalize, $v);
        }

        return $v;
    };

    $typed = static fn ($v) => typed_value($normalize($v));

    $scripts = [];
    for ($n = 0; $n < 1500; ++$n) {
        if (is_dir($dir)) {
            array_map('unlink', glob("$dir/*") ?: []);
        }
        $GLOBALS['recorder_now'] = 1700000000;
        $kind = $pick(['array', 'array', 'file']);
        $attributeKey = $pick(['_sf2_attributes', '_sf2_attributes', 'attrs']);
        $flashKind = $pick(['flash', 'auto']);
        $flashKey = $pick(['_symfony_flashes', '_symfony_flashes', 'fl']);
        $threshold = $pick([0, 0, 30]);
        $cookieLifetime = $pick([null, null, 60]);
        $data = null;
        if ('array' === $kind && 0 === mt_rand(0, 2)) {
            $data = [];
            if (0 !== mt_rand(0, 3)) {
                $data[] = [$attributeKey, $specOut($pick([['a', [['s', 'pre']]], ['s', 'tampered'], ['n'], ['a', []]]))];
            }
            if (0 !== mt_rand(0, 3)) {
                $data[] = [$flashKey, $specOut($pick([['a', [['s', 'old']]], ['s', 'tampered'], ['a', []]]))];
            }
            if (0 === mt_rand(0, 2)) {
                $data[] = ['_sf2_meta', $pick([['meta', 1699999000, 1699999990, 0], ['meta', null, 1699999990, 0], ['meta', 1699999000, null, 5]])];
            }
            if (0 === mt_rand(0, 3)) {
                $data[] = ['other', $specOut(['s', 'kept'])];
            }
        }

        $ops = [];
        for ($i = mt_rand(4, 20); $i > 0; --$i) {
            $ops[] = match ($pick(['start', 'set', 'set', 'set', 'get', 'has', 'remove', 'all', 'replace', 'clear', 'invalidate', 'migrate', 'save', 'save', 'id', 'set_id', 'name', 'is_started', 'is_empty', 'count', 'f_add', 'f_add', 'f_set', 'f_peek', 'f_peek_all', 'f_get', 'f_all', 'f_set_all', 'f_has', 'f_keys', 'm_created', 'm_last_used', 'm_lifetime', 'reopen', 'reopen', 'usage'])) {
                'start' => ['start'],
                'set' => ['set', $b64($pick($keys)), $specOut($pick($values))],
                'get' => ['get', $b64($pick($keys)), $specOut($pick([['n'], ['s', 'dflt']]))],
                'has' => ['has', $b64($pick($keys))],
                'remove' => ['remove', $b64($pick($keys))],
                'all' => ['all'],
                'replace' => ['replace', array_map(static fn () => [$b64($pick($keys)), $specOut($pick($values))], range(1, mt_rand(0, 3)))],
                'clear' => ['clear'],
                'invalidate' => ['invalidate', $pick([null, 0, 100])],
                'migrate' => ['migrate', 0 === mt_rand(0, 1), $pick([null, 0, 100])],
                'save' => ['save'],
                'id' => ['id'],
                'set_id' => ['set_id', $b64($pick($ids))],
                'name' => ['name'],
                'is_started' => ['is_started'],
                'is_empty' => ['is_empty'],
                'count' => ['count'],
                'f_add' => ['f_add', $b64($pick($types)), $specOut($pick($values))],
                'f_set' => ['f_set', $b64($pick($types)), $specOut($pick([['s', 'm'], ['a', [['s', 'a'], ['s', 'b']]], ['i', 3], ['a', []]]))],
                'f_peek' => ['f_peek', $b64($pick($types))],
                'f_peek_all' => ['f_peek_all'],
                'f_get' => ['f_get', $b64($pick($types))],
                'f_all' => ['f_all'],
                'f_set_all' => ['f_set_all', array_map(static fn () => [$b64($pick($types)), $specOut($pick([['s', 'str'], ['a', [['s', 'one']]]]))], range(0, mt_rand(0, 2)))],
                'f_has' => ['f_has', $b64($pick($types))],
                'f_keys' => ['f_keys'],
                'm_created' => ['m_created'],
                'm_last_used' => ['m_last_used'],
                'm_lifetime' => ['m_lifetime'],
                'reopen' => ['reopen'],
                'usage' => ['usage', $pick([\PHP_INT_MIN, -2, 0])],
            };
            $ops[count($ops) - 1][] = mt_rand(0, 3) * mt_rand(0, 13); // seconds before
        }

        $script = compact('kind', 'attributeKey', 'flashKind', 'flashKey', 'threshold', 'cookieLifetime', 'data', 'ops');

        $reported = 0;
        $make = static function () use ($kind, $dir, $attributeKey, $flashKind, $flashKey, $threshold, $cookieLifetime, &$reported) {
            $meta = new MetadataBag('_sf2_meta', $threshold, $cookieLifetime);
            $storage = 'file' === $kind ? new MockFileSessionStorage($dir, 'MOCKSESSID', $meta) : new MockArraySessionStorage('MOCKSESSID', $meta);
            $flashes = 'auto' === $flashKind ? new AutoExpireFlashBag($flashKey) : new FlashBag($flashKey);
            $session = new Session($storage, new AttributeBag($attributeKey), $flashes, static function () use (&$reported) { ++$reported; });

            return [$storage, $session];
        };
        [$storage, $session] = $make();
        if (null !== $data) {
            $array = [];
            foreach ($data as [$k, $v]) {
                $array[$k] = 'meta' === $v[0] ? array_filter(['c' => $v[1], 'u' => $v[2], 'l' => $v[3]], static fn ($x) => null !== $x) : $value($v);
            }
            $storage->setSessionData($array);
        }

        $results = [];
        foreach ($ops as $op) {
            $GLOBALS['recorder_now'] += $op[\count($op) - 1];
            $arg = static fn (int $i) => base64_decode($op[$i]);
            $file = null;
            try {
                $result = match ($op[0]) {
                    'start' => $typed($session->start()),
                    'set' => $typed($session->set($arg(1), $value($op[2]))),
                    'get' => $typed($session->get($arg(1), $value($op[2]))),
                    'has' => $typed($session->has($arg(1))),
                    'remove' => $typed($session->remove($arg(1))),
                    'all' => $typed($session->all()),
                    'replace' => $typed($session->replace(array_combine(array_map('base64_decode', array_column($op[1], 0)), array_map($value, array_column($op[1], 1))) ?: [])),
                    'clear' => $typed($session->clear()),
                    'invalidate' => $typed($session->invalidate($op[1])),
                    'migrate' => $typed($session->migrate($op[1], $op[2])),
                    'save' => (static function () use ($session, $storage, $kind, $dir, $typed) {
                        $session->save();
                        if ('file' === $kind) {
                            $path = $dir.'/'.$storage->getId().'.mocksess';

                            return ['saved', is_file($path) ? $typed(unserialize(file_get_contents($path))) : null];
                        }

                        return $typed(null);
                    })(),
                    'id' => $typed($session->getId()),
                    'set_id' => $typed($session->setId($arg(1))),
                    'name' => $typed($session->getName()),
                    'is_started' => $typed($session->isStarted()),
                    'is_empty' => $typed($session->isEmpty()),
                    'count' => $typed(\count($session)),
                    'f_add' => $typed($session->getFlashBag()->add($arg(1), $value($op[2]))),
                    'f_set' => $typed($session->getFlashBag()->set($arg(1), $value($op[2]))),
                    'f_peek' => $typed($session->getFlashBag()->peek($arg(1))),
                    'f_peek_all' => $typed($session->getFlashBag()->peekAll()),
                    'f_get' => $typed($session->getFlashBag()->get($arg(1))),
                    'f_all' => $typed($session->getFlashBag()->all()),
                    'f_set_all' => $typed($session->getFlashBag()->setAll(array_combine(array_map('base64_decode', array_column($op[1], 0)), array_map($value, array_column($op[1], 1))) ?: [])),
                    'f_has' => $typed($session->getFlashBag()->has($arg(1))),
                    'f_keys' => $typed($session->getFlashBag()->keys()),
                    'm_created' => $typed($session->getMetadataBag()->getCreated()),
                    'm_last_used' => $typed($session->getMetadataBag()->getLastUsed()),
                    'm_lifetime' => $typed($session->getMetadataBag()->getLifetime()),
                    'usage' => (static function () use ($session, $op, $typed) {
                        // as SessionListener does, through &getUsageIndex()
                        $index = &$session->getUsageIndex();
                        $index = $op[1];

                        return $typed(null);
                    })(),
                    'reopen' => (static function () use (&$storage, &$session, $make, $typed) {
                        $id = $storage->getId();
                        [$storage, $session] = $make();
                        $storage->setId($id);

                        return $typed(null);
                    })(),
                };
            } catch (\Throwable $e) {
                $result = ['throws', $e::class, base64_encode(preg_replace('/, called in .* on line \d+$/', '', $e->getMessage()))];
            }
            $results[] = [$result, $session->getUsageIndex(), $reported];
        }
        $script['results'] = $results;
        $scripts[] = $script;
    }

    echo json_encode(['scripts' => $scripts], \JSON_THROW_ON_ERROR), "\n";
}
