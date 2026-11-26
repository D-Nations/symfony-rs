<?php
// Request's and RequestStack's session methods, for what RequestTest
// leaves open, for crates/http-foundation/tests/data/request_session.json
// (scripts/record.sh http-foundation-request-session): 800 seeded scripts.
//
// A script starts with one Request (Request::create() with 0 to 3 cookies
// whose names come from a pool) and a RequestStack. Then 4 to 16
// operations from a pool, on a request picked by index:
// - hasSession() with either argument, getSession(), hasPreviousSession();
// - setSession() with a new Session on a MockArraySessionStorage (its name
//   from the pool) or one made before;
// - setSessionFactory() with a new factory or one made before: a factory
//   returns the same session each time, a new session each call, null, or
//   throws a RuntimeException;
// - duplicate() and clone, which add a request;
// - a cookie set or removed;
// - a session's setName();
// - push() and pop() on the stack, and the stack's getSession().
//
// Recorded: each operation's result, as true/false, a session label
// (s1, s2... in order of creation), a request index, or ["throws", class,
// message]. A TypeError loses its ", called in ... on line N" suffix. At the
// end: how many times each factory was called.
// Plain JSON: every text here is ASCII.

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';

use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\RequestStack;
use Symfony\Component\HttpFoundation\Session\Session;
use Symfony\Component\HttpFoundation\Session\Storage\MockArraySessionStorage;

mt_srand(20261127);
$pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];

$names = ['MOCKSESSID', 'PHPSESSID', 'sid', 'other'];
$kinds = ['same', 'fresh', 'null', 'throw'];

$scripts = [];
for ($n = 0; $n < 800; ++$n) {
    $labels = new \SplObjectStorage();
    $sessions = [];
    $newSession = static function (string $name) use ($labels, &$sessions): Session {
        $session = new Session(new MockArraySessionStorage($name));
        $sessions[] = $session;
        $labels[$session] = 's'.\count($sessions);

        return $session;
    };
    $label = static fn ($s) => $s instanceof Session ? $labels[$s] : get_debug_type($s);

    $cookies = [];
    for ($c = mt_rand(0, 3); $c > 0; --$c) {
        $cookies[$pick($names)] = 'v'.mt_rand(0, 9);
    }
    $requests = [Request::create('/', 'GET', [], $cookies)];
    $stack = new RequestStack();
    $factories = [];
    $calls = [];
    $ops = [];
    $results = [];

    for ($o = mt_rand(4, 16); $o > 0; --$o) {
        $i = mt_rand(0, \count($requests) - 1);
        $r = $requests[$i];
        $op = $pick(['has', 'has', 'get', 'get', 'prev', 'prev', 'set', 'factory', 'factory', 'dup', 'clone', 'cookie_set', 'cookie_remove', 'rename', 'push', 'push', 'pop', 'stack_get', 'stack_get']);
        switch ($op) {
            case 'has':
                $op = [$op, $i, (bool) mt_rand(0, 1)];
                break;
            case 'set':
                $op = $sessions && mt_rand(0, 1) ? [$op, $i, 'old', mt_rand(1, \count($sessions))] : [$op, $i, 'new', $pick($names)];
                break;
            case 'factory':
                $op = $factories && mt_rand(0, 2) === 0 ? [$op, $i, 'old', mt_rand(1, \count($factories))] : [$op, $i, 'new', $pick($kinds), $pick($names)];
                break;
            case 'cookie_set':
            case 'cookie_remove':
                $op = [$op, $i, $pick($names)];
                break;
            case 'rename':
                if (!$sessions) {
                    $op = ['get', $i];
                    break;
                }
                $op = [$op, mt_rand(1, \count($sessions)), $pick($names)];
                break;
            case 'push':
            case 'get':
            case 'prev':
            case 'dup':
            case 'clone':
                $op = [$op, $i];
                break;
            default:
                $op = [$op];
        }
        try {
            $result = match ($op[0]) {
                'has' => $r->hasSession($op[2]),
                'get' => $label($r->getSession()),
                'prev' => $r->hasPreviousSession(),
                'set' => $r->setSession('old' === $op[2] ? $sessions[$op[3] - 1] : $newSession($op[3])),
                'factory' => (function () use ($r, $op, &$factories, &$calls, $newSession) {
                    if ('old' === $op[2]) {
                        $r->setSessionFactory($factories[$op[3] - 1]);

                        return null;
                    }
                    $f = \count($factories);
                    $calls[$f] = 0;
                    $same = null;
                    $kind = $op[3];
                    $name = $op[4];
                    $factories[] = static function () use ($f, $kind, $name, &$calls, &$same, $newSession) {
                        ++$calls[$f];

                        return match ($kind) {
                            'same' => $same ??= $newSession($name),
                            'fresh' => $newSession($name),
                            'null' => null,
                            'throw' => throw new \RuntimeException('factory '.($f + 1).' failed'),
                        };
                    };
                    $r->setSessionFactory($factories[$f]);

                    return null;
                })(),
                'dup' => array_push($requests, $r->duplicate()) - 1,
                'clone' => array_push($requests, clone $r) - 1,
                'cookie_set' => $r->cookies->set($op[2], 'x'),
                'cookie_remove' => $r->cookies->remove($op[2]),
                'rename' => $sessions[$op[1] - 1]->setName($op[2]),
                'push' => $stack->push($r),
                'pop' => null !== $stack->pop() ? true : false,
                'stack_get' => $label($stack->getSession()),
            };
        } catch (\Throwable $e) {
            $message = $e->getMessage();
            if ($e instanceof \TypeError) {
                $message = preg_replace('/, called in .*$/s', '', $message);
            }
            $result = ['throws', $e::class, $message];
        }
        $ops[] = $op;
        $results[] = $result;
    }
    $scripts[] = ['cookies' => $cookies, 'ops' => $ops, 'results' => $results, 'calls' => array_values($calls)];
}

echo json_encode(['scripts' => $scripts], \JSON_PRETTY_PRINT | \JSON_THROW_ON_ERROR), "\n";
