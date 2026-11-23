<?php
// Cookie answers for what CookieTest leaves open, for
// crates/http-foundation/tests/data/cookie.json (scripts/record.sh
// http-foundation): Cookie::fromString() over Set-Cookie texts, decoded and
// not, and the constructor over names, values, expiries, paths, domains
// and SameSite values. The clock is fixed as symfony/phpunit-bridge's
// ClockMock fixes it, by functions in Cookie's namespace; strtotime() reads
// the same clock. Texts are base64; a case that throws records its class
// and message.

namespace Symfony\Component\HttpFoundation {
    const RECORDER_NOW = 1700000000;

    function time(): int
    {
        return RECORDER_NOW;
    }

    function strtotime(string $datetime, ?int $baseTimestamp = null): int|false
    {
        return \strtotime($datetime, $baseTimestamp ?? RECORDER_NOW);
    }
}

namespace {
    error_reporting(E_ALL);

    require '/demo/vendor/autoload.php';

    use Symfony\Component\HttpFoundation\Cookie;

    $b = static fn (?string $s) => null === $s ? null : base64_encode($s);

    $describe = static function (callable $make) use ($b): array {
        try {
            $c = $make();
        } catch (\Throwable $e) {
            return ['throws', $e::class, base64_encode($e->getMessage())];
        }

        return ['cookie', $b($c->getName()), $b($c->getValue()), $c->getExpiresTime(), $b($c->getPath()), $b($c->getDomain()), $c->isSecure(), $c->isHttpOnly(), $c->isRaw(), $c->getSameSite(), $c->isPartitioned(), $c->getMaxAge(), $c->isCleared(), $b((string) $c)];
    };

    $attributes = [
        '', 'path=/', 'path=', 'path', 'path=0', 'path="/a"', 'path=/a b', 'path=/a,b', 'Path=/A', 'domain=example.com',
        'domain=', 'domain=0', 'domain=.x.org', 'Domain=a b', 'secure', 'Secure=no', 'SECURE', 'httponly', 'HttpOnly=0',
        'partitioned', 'samesite=lax', 'samesite=Strict', 'SameSite=NONE', 'samesite=', 'samesite=foo', 'samesite="lax"',
        'expires=Fri, 20 May 2011 15:25:52 GMT', 'expires=Tue, 14-Nov-23 22:13:20 GMT', 'expires=Sun Nov  6 08:49:37 1994',
        'expires="Wed, 21 Oct 2015 07:28:00 GMT"', 'expires=1234567890', 'expires=0', 'expires=-5', 'expires=1.5e3',
        'expires= 12', 'expires=0.5', 'expires=invalid', 'expires=', 'expires=bar', 'expires=Thu, 01 Jan 1970 00:00:00 GMT',
        'expires=Wed, 31 Dec 1969 23:59:59 GMT', 'expires=2011-05-20', 'expires=@1700003600', 'expires=+1 day',
        'expires=Fri, 32 May 2011 15:25:52 GMT', 'expires=Fri, 20 May 2011 25:00:00 GMT', 'max-age=0', 'max-age=60',
        'Max-Age=-60', 'max-age=1e3', 'max-age= 30', 'max-age=007', 'max-age=-0', 'max-age=99999999999999999999',
        'raw', 'raw=0', 'unknown=1', '=x', ' ; ', 'expires=Fri, 20 May 2011 15:25:52 GMT; max-age=60',
        'expires=Tue, 14 Nov 2023 23:13:20 GMT; max-age=0', 'max-age=60; max-age=bad', 'samesite=lax; samesite=foo',
        'path=/a; path=/b', 'domain=a.com; domain=', 'expires=1700000500; Max-Age=-1',
    ];
    $pairs = ['foo=bar', 'foo', 'foo=', ' foo = bar ', 'foo=bar=baz', '0=0', '=bar', '', 'a b=c d', 'n%20a=v%2Bx', 'foo="q"', "f\too=x", 'foo=a+b', 'na,me=v', 'x=%zz', "foo=\x00v"];

    $fromString = [];
    foreach ($pairs as $pair) {
        foreach ($attributes as $attribute) {
            foreach ([false, true] as $decode) {
                $text = '' === $attribute ? $pair : $pair.'; '.$attribute;
                $fromString[] = [base64_encode($text), $decode, $describe(static fn () => Cookie::fromString($text, $decode))];
            }
        }
    }

    $construct = [];
    $names = ['foo', '0', 'a=b', 'a b', "a\tb", 'ä', 'a;b'];
    $values = [null, '', '0', 'bar', 'b a+r~/?=&;,%', "\xff\x00"];
    $expires = [0, 1, -1, 1700000000, 1700000001, 1699999999, PHP_INT_MAX, '0', '12', ' 12', '12 ', '1.5e3', '0.5', '-0.5', '1e30', 'Fri, 20 May 2011 15:25:52 GMT', '@5', 'tomorrow', '+1 hour', 'bar', '', 'Tue'];
    foreach ($names as $name) {
        foreach ($values as $value) {
            foreach ([true, false] as $raw) {
                $construct[] = [['name', $b($name), $b($value), $raw], $describe(static fn () => new Cookie($name, $value, 0, '/', null, null, true, $raw))];
            }
        }
    }
    foreach ($expires as $expire) {
        $construct[] = [['expire', \is_int($expire) ? $expire : $b($expire)], $describe(static fn () => new Cookie('foo', 'bar', $expire))];
    }
    foreach (['/', '', '0', null, '/a=b', '/a b', 'x'] as $path) {
        foreach (['', '0', null, 'example.com', 'a,b'] as $domain) {
            $construct[] = [['path', $b($path), $b($domain)], $describe(static fn () => new Cookie('foo', 'bar', 0, $path, $domain))];
        }
    }
    foreach (['lax', 'LAX', 'Strict', 'none', '', null, 'foo', ' lax'] as $sameSite) {
        foreach ([null, true, false] as $secure) {
            $construct[] = [['samesite', $b($sameSite), $secure], $describe(static fn () => new Cookie('foo', 'bar', 0, '/', null, $secure, false, false, $sameSite, true))];
        }
    }

    echo json_encode(['now' => Symfony\Component\HttpFoundation\RECORDER_NOW, 'from_string' => $fromString, 'construct' => $construct], JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR), "\n";
}
