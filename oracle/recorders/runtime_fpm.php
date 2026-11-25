<?php
// PHP-FPM's answers for crates/http-foundation/tests/data/sapi.json
// (scripts/record.sh runtime-fpm), in the oracle's FPM image
// (oracle/fpm.Dockerfile: PHP 8.4.27 on Debian bookworm, no php.ini, as
// the CLI oracle). Starts php-fpm, installs runtime_fpm_index.php as the
// front controller /srv/app/public/index.php, and sends it FastCGI
// requests itself: the parameters a web server in front of PHP-FPM passes
// (nginx's fastcgi_params with Symfony's documented location: SCRIPT_NAME
// /index.php, DOCUMENT_ROOT the public directory) and the body. Recorded
// per case: the parameters and body sent; what the front controller saw
// ($_SERVER, $_GET, $_POST, $_COOKIE, $_REQUEST, $_FILES with each
// uploaded file's content, php://input, request_parse_body() for PUT,
// DELETE, PATCH and QUERY); the answers and warnings of its script of
// output calls; and the FastCGI response (stdout, stderr) PHP-FPM sent.
// Request cases are written out; output scripts are generated from a fixed
// seed. Texts are base64; arrays are [key, value] pairs, string keys and
// strings prefixed "s:".

error_reporting(E_ALL);

const DOCROOT = '/srv/app/public';
const FRONT = DOCROOT.'/index.php';

@mkdir(DOCROOT, 0755, true);
copy(__DIR__.'/runtime_fpm_index.php', FRONT);
@mkdir('/tmp/oracle', 0777, true);
chmod('/tmp/oracle', 0777);

// display_errors off, as in production: startup warnings go to the
// FastCGI stderr stream instead of the body.
exec('php-fpm -D -d display_errors=0 > /tmp/oracle/fpm.log 2>&1', $fpmOut, $fpmCode);
if (0 !== $fpmCode) {
    fwrite(STDERR, "php-fpm failed: ".implode("\n", $fpmOut)."\n");
    exit(1);
}
for ($i = 0; $i < 100; ++$i) {
    $probe = @stream_socket_client('tcp://127.0.0.1:9000', $errno, $errstr, 1);
    if ($probe) {
        fclose($probe);
        break;
    }
    usleep(50000);
}

function fcgi_record(int $type, string $content): string
{
    $out = '';
    foreach ('' === $content ? [''] : str_split($content, 65535) as $chunk) {
        $padding = (8 - strlen($chunk) % 8) % 8;
        $out .= pack('CCnnCC', 1, $type, 1, strlen($chunk), $padding, 0).$chunk.str_repeat("\0", $padding);
    }

    return $out;
}

function fcgi_length(int $n): string
{
    return $n < 128 ? chr($n) : pack('N', $n | 0x80000000);
}

/** Sends one request; returns [stdout, stderr, app status]. */
function fcgi_request(array $params, string $stdin): array
{
    $socket = stream_socket_client('tcp://127.0.0.1:9000', $errno, $errstr, 10);
    if (!$socket) {
        throw new RuntimeException("connect: $errstr");
    }
    $pairs = '';
    foreach ($params as [$name, $value]) {
        $pairs .= fcgi_length(strlen($name)).fcgi_length(strlen($value)).$name.$value;
    }
    $request = fcgi_record(1, pack('nCxxxxx', 1, 0));
    if ('' !== $pairs) {
        $request .= fcgi_record(4, $pairs);
    }
    $request .= fcgi_record(4, '');
    if ('' !== $stdin) {
        $request .= fcgi_record(5, $stdin);
    }
    $request .= fcgi_record(5, '');
    fwrite($socket, $request);
    $stdout = $stderr = '';
    $status = null;
    while (null === $status) {
        $header = fread_exact($socket, 8);
        if (null === $header) {
            break;
        }
        $h = unpack('Cversion/Ctype/nid/nlength/Cpadding/Creserved', $header);
        $content = $h['length'] ? fread_exact($socket, $h['length']) : '';
        if ($h['padding']) {
            fread_exact($socket, $h['padding']);
        }
        match ($h['type']) {
            6 => $stdout .= $content,
            7 => $stderr .= $content,
            3 => $status = unpack('Napp/Cprotocol', $content)['app'],
            default => null,
        };
    }
    fclose($socket);

    return [$stdout, $stderr, $status];
}

function fread_exact($socket, int $n): ?string
{
    $data = '';
    while (strlen($data) < $n) {
        $chunk = fread($socket, $n - strlen($data));
        if (false === $chunk || ('' === $chunk && feof($socket))) {
            return null;
        }
        $data .= $chunk;
    }

    return $data;
}

/**
 * The parameters nginx passes for a request through Symfony's location:
 * fastcgi_params in its order, then SCRIPT_FILENAME and DOCUMENT_ROOT as
 * $realpath_root (once: the web server emulation passes each name once),
 * then the request headers as HTTP_*.
 */
function nginx_params(string $method, string $target, array $headers, string $body, array $extra = []): array
{
    $query = str_contains($target, '?') ? substr($target, strpos($target, '?') + 1) : '';
    $contentType = '';
    $contentLength = '';
    foreach ($headers as [$name, $value]) {
        if (0 === strcasecmp($name, 'Content-Type')) {
            $contentType = $value;
        }
        if (0 === strcasecmp($name, 'Content-Length')) {
            $contentLength = $value;
        }
    }
    $params = [
        ['QUERY_STRING', $query],
        ['REQUEST_METHOD', $method],
        ['CONTENT_TYPE', $contentType],
        ['CONTENT_LENGTH', $contentLength],
        ['SCRIPT_NAME', '/index.php'],
        ['REQUEST_URI', $target],
        ['DOCUMENT_URI', '/index.php'],
        ['DOCUMENT_ROOT', DOCROOT],
        ['SERVER_PROTOCOL', $extra['protocol'] ?? 'HTTP/1.1'],
        ['REQUEST_SCHEME', isset($extra['https']) ? 'https' : 'http'],
    ];
    if (isset($extra['https'])) {
        $params[] = ['HTTPS', 'on'];
    }
    array_push(
        $params,
        ['GATEWAY_INTERFACE', 'CGI/1.1'],
        ['SERVER_SOFTWARE', 'nginx/1.27.4'],
        ['REMOTE_ADDR', '192.0.2.10'],
        ['REMOTE_PORT', '50432'],
        ['SERVER_ADDR', '192.0.2.1'],
        ['SERVER_PORT', isset($extra['https']) ? '443' : '80'],
        ['SERVER_NAME', 'lab.example'],
        ['REDIRECT_STATUS', '200'],
        ['SCRIPT_FILENAME', FRONT],
    );
    foreach ($headers as [$name, $value]) {
        $params[] = ['HTTP_'.strtoupper(str_replace('-', '_', $name)), $value];
    }

    return array_merge($params, $extra['params'] ?? []);
}

function multipart(string $boundary, array $parts, bool $close = true): string
{
    $body = '';
    foreach ($parts as [$disposition, $headers, $content]) {
        $body .= "--$boundary\r\nContent-Disposition: form-data; $disposition\r\n";
        foreach ($headers as $h) {
            $body .= "$h\r\n";
        }
        $body .= "\r\n$content\r\n";
    }

    return $body.($close ? "--$boundary--\r\n" : '');
}

// The request cases: [name, method, target, headers, body, extra].
function request_cases(): array
{
    $lab = [['Cookie', 'session=first; session=second; a.b=x%2By'], ['Accept-Language', 'fr-CA, en;q=0.5'], ['X-Forwarded-Proto', 'https']];
    $form = ['Content-Type', 'application/x-www-form-urlencoded'];
    $mp = static fn (string $b = 'Lab') => ['Content-Type', "multipart/form-data; boundary=$b"];
    $file = static fn (string $name, string $filename, string $content, ?string $type = 'text/plain') => [
        "name=\"$name\"; filename=\"$filename\"", null === $type ? [] : ["Content-Type: $type"], $content,
    ];
    $field = static fn (string $name, string $value) => ["name=\"$name\"", [], $value];

    $cases = [
        ['lab-inspect', 'GET', '/inspect?a.b=first&a_b=last&list[]=x&list[]=y&scalar=hello+world', array_merge([['Host', 'lab.example']], $lab), ''],
        ['no-host', 'GET', '/inspect', [], ''],
        ['root', 'GET', '/', [['Host', 'localhost:8012']], ''],
        ['head', 'HEAD', '/item/42', [['Host', 'lab.example']], ''],
        ['options', 'OPTIONS', '/item/42', [['Host', 'lab.example']], ''],
        ['query-edges', 'GET', '/?a&b=&c[]=1&c[x]=2&%20d=3&e.f[g.h]=4&&=5&i[=6', [], ''],
        ['query-encoded', 'GET', '/p%20a?x=%41%zz&y=%E2%82%AC', [], ''],
        ['cookies', 'GET', '/', [['Cookie', 'a=1; b[]=2; b[]=3; c=%20+; ; d'], ['Cookie2', 'x']], ''],
        ['headers', 'GET', '/', [['Host', 'lab.example'], ['User-Agent', 'lab'], ['X-Custom-Header', 'v'], ['Authorization', 'Basic dXNlcjpwYXNz'], ['If-None-Match', '"a", "b"']], ''],
        ['https', 'GET', '/secure?q=1', [['Host', 'lab.example']], '', ['https' => true]],
        ['http10', 'GET', '/', [], '', ['protocol' => 'HTTP/1.0']],
        ['duplicate-param', 'GET', '/', [], '', ['params' => [['SERVER_NAME', 'second.example'], ['X_ENV', 'v']]]],
        ['post-form', 'POST', '/inspect', [$form, ['Content-Length', '29']], 'name=one&name=two&tags[]=rust'],
        ['post-form-charset', 'POST', '/inspect', [['Content-Type', 'application/x-www-form-urlencoded; charset=UTF-8'], ['Content-Length', '7']], 'a=1&b=2'],
        ['post-json', 'POST', '/api', [['Content-Type', 'application/json'], ['Content-Length', '13']], '{"a":[1,2,3]}'],
        ['post-no-type', 'POST', '/api', [['Content-Length', '3']], 'a=1'],
        ['get-form', 'GET', '/inspect', [$form, ['Content-Length', '12']], 'name=ignored'],
        ['post-multipart-fields', 'POST', '/inspect', [$mp(), ['Content-Length', '0']], multipart('Lab', [$field('name', 'value'), $field('tags[]', 'a'), $field('tags[]', 'b')])],
        ['post-multipart-nameless', 'POST', '/inspect', [$mp()], "--Lab\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nvalue\r\n--Lab\r\nContent-Disposition: form-data\r\n\r\nignored\r\n--Lab--\r\n"],
        ['post-file', 'POST', '/upload', [$mp()], multipart('Lab', [$field('title', 't'), $file('doc', 'a.txt', "hello\r\nworld")])],
        ['post-file-empty-name', 'POST', '/upload', [$mp()], multipart('Lab', [$file('doc', '', '', 'application/octet-stream')])],
        ['post-file-empty-content', 'POST', '/upload', [$mp()], multipart('Lab', [$file('doc', 'empty.txt', '')])],
        ['post-file-no-type', 'POST', '/upload', [$mp()], multipart('Lab', [$file('doc', 'a.bin', "\x00\x01\x02", null)])],
        ['post-file-type-params', 'POST', '/upload', [$mp()], multipart('Lab', [$file('doc', 'a.txt', 'x', 'text/plain; charset=utf-8')])],
        ['post-file-paths', 'POST', '/upload', [$mp()], multipart('Lab', [$file('a', 'dir/sub/a.txt', '1'), $file('b', 'C:\\Users\\x\\b.txt', '2'), $file('c', '../../etc/c', '3'), $file('d', 'dir/', '4'), $file('e', 'name with spaces.txt', '5'), $file('f', '.hidden', '6')])],
        ['post-file-array', 'POST', '/upload', [$mp()], multipart('Lab', [$file('files[]', 'one.txt', '1'), $file('files[]', 'two.txt', '2'), $file('nested[a][b]', 'n.txt', 'n'), $file('nested[a][c]', '', '')])],
        ['post-file-mixed-names', 'POST', '/upload', [$mp()], multipart('Lab', [$file('f', 'first.txt', '1'), $file('f', 'second.txt', '2'), $file('g[x]', 'x.txt', 'x'), $file('g', 'plain.txt', 'p')])],
        ['post-file-field-same-name', 'POST', '/upload', [$mp()], multipart('Lab', [$file('f', 'x.txt', 'data'), $field('f', 'shadow')])],
        ['post-file-max-file-size', 'POST', '/upload', [$mp()], multipart('Lab', [$field('MAX_FILE_SIZE', '4'), $file('small', 's.txt', '1234'), $file('big', 'b.txt', '12345'), $field('MAX_FILE_SIZE', '100'), $file('later', 'l.txt', '12345')])],
        ['post-file-ini-size', 'POST', '/upload', [$mp()], multipart('Lab', [$file('big', 'big.bin', str_repeat('x', 2 * 1024 * 1024 + 1)), $file('ok', 'ok.txt', 'ok')])],
        ['post-file-exact-ini-size', 'POST', '/upload', [$mp()], multipart('Lab', [$file('big', 'big.bin', str_repeat('y', 2 * 1024 * 1024))])],
        ['post-file-too-many', 'POST', '/upload', [$mp()], multipart('Lab', array_merge(array_map(static fn ($i) => $file("f$i", "f$i.txt", (string) $i), range(1, 21)), [$file('empty', '', ''), $field('after', 'kept')]))],
        ['post-file-unterminated', 'POST', '/upload', [$mp()], "--Lab\r\nContent-Disposition: form-data; name=\"doc\"; filename=\"cut.txt\"\r\nContent-Type: text/plain\r\n\r\nno end"],
        ['post-file-filename-only', 'POST', '/upload', [$mp()], multipart('Lab', [['filename="only.txt"', ['Content-Type: text/plain'], 'x'], $field('after', 'v')])],
        ['post-file-unquoted', 'POST', '/upload', [$mp()], multipart('Lab', [['name=doc; filename=plain.txt', ['Content-Type:   image/png  '], 'png']])],
        ['post-file-quoted-boundary', 'POST', '/upload', [['Content-Type', 'multipart/form-data; boundary="q b"; charset=utf-8']], multipart('q b', [$field('a', '1'), $file('doc', 'q.txt', 'q')])],
        ['post-file-name-brackets', 'POST', '/upload', [$mp()], multipart('Lab', [$file('a.b[c.d]', 'x.txt', 'x'), $file('sp ace', 'y.txt', 'y'), $file('arr[]x', 'z.txt', 'z')])],
        ['post-oversize', 'POST', '/inspect', [$form], 'a=1&b='.str_repeat('z', 8 * 1024 * 1024)],
        ['post-oversize-multipart', 'POST', '/upload', [$mp()], multipart('Lab', [$field('a', '1'), $file('f', 'f.txt', str_repeat('z', 8 * 1024 * 1024))])],
        ['post-exact-max', 'POST', '/inspect', [$form], 'a=1&b='.str_repeat('z', 8 * 1024 * 1024 - 6)],
    ];
    foreach (['PUT', 'PATCH', 'DELETE', 'QUERY'] as $method) {
        $m = strtolower($method);
        $cases[] = ["$m-form", $method, '/inspect', [$form], 'name=one&name=two&tags[]=rust'];
        $cases[] = ["$m-multipart", $method, '/inspect', [$mp()], multipart('Lab', [$field('foo', 'bar'), $file('baz', 'baz.txt', 'qux')])];
        $cases[] = ["$m-multipart-nameless", $method, '/inspect', [$mp()], "--Lab\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nvalue\r\n--Lab\r\nContent-Disposition: form-data\r\n\r\nignored\r\n--Lab--\r\n"];
        $cases[] = ["$m-json", $method, '/api', [['Content-Type', 'application/json']], '{"a":1}'];
        $cases[] = ["$m-empty", $method, '/api', [], ''];
    }
    $cases[] = ['put-multipart-no-boundary', 'PUT', '/inspect', [['Content-Type', 'multipart/form-data']], "--Lab\r\n"];
    $cases[] = ['put-file-ini-size', 'PUT', '/upload', [$mp()], multipart('Lab', [$file('big', 'big.bin', str_repeat('x', 2 * 1024 * 1024 + 1))])];
    $cases[] = ['put-file-array', 'PUT', '/upload', [$mp()], multipart('Lab', [$file('files[]', 'one.txt', '1'), $file('files[]', '', '')])];
    $cases[] = ['put-oversize', 'PUT', '/inspect', [$form], 'a=1&b='.str_repeat('z', 8 * 1024 * 1024)];
    $cases[] = ['post-lowercase-method', 'post', '/inspect', [$form], 'a=1'];
    $cases[] = ['post-file-nameless-twice', 'POST', '/upload', [$mp()], multipart('Lab', [['filename="a.txt"', [], 'a'], ['filename="b.txt"', [], 'b'], ['name=""; filename="c.txt"', [], 'c'], $file('x', 'x.txt', 'x')])];
    foreach (['arr[x]y', 'o[', 'c]', '[x]', 'a[b][c', 'd[]', 'e[][]', 'f[ g ]', 'm[]x[y]', 'n[a]]', '[]', 'p[]]', 'q[x', 'r[a][]', ' s[t]', 'u.v w[x.y]', 'y[0][1]', 'z[-1]'] as $i => $name) {
        $cases[] = ["post-file-name-$i", 'POST', '/upload', [$mp()], multipart('Lab', [$file($name, 'n.txt', 'n'), $file('ok', 'ok.txt', 'ok')])];
    }
    $cases[] = ['post-file-abort-order', 'POST', '/upload', [$mp()], multipart('Lab', [$field('before', 'b'), $file('ok1', '1.txt', '1'), $file('c]', '2.txt', '2'), $file('ok2', '3.txt', '3'), $field('after', 'a'), $file('', '4.txt', '4')])];
    $cases[] = ['post-file-abort-empty-filename', 'POST', '/upload', [$mp()], multipart('Lab', [$file('ok1', '1.txt', '1'), $file('o[', '', ''), $file('ok2', '3.txt', '3')])];
    $cases[] = ['post-file-abort-oversize', 'POST', '/upload', [$mp()], multipart('Lab', [$file('big', 'b.bin', str_repeat('x', 2 * 1024 * 1024 + 1)), $file('o[', 'x.txt', 'x'), $file('ok', 'ok.txt', 'ok')])];
    $cases[] = ['post-file-nameless-then-named', 'POST', '/upload', [$mp()], multipart('Lab', [$file('5', 'a.txt', 'a'), ['filename="b.txt"', [], 'b'], $file('x[]', 'c.txt', 'c'), ['filename="d.txt"', [], 'd']])];
    $cases[] = ['post-field-odd-names', 'POST', '/inspect', [$mp()], multipart('Lab', [$field('o[', '1'), $field('c]', '2'), $field('arr[x]y', '3'), $field('[x]', '4')])];
    $cases[] = ['request-merge', 'POST', '/?a[x]=g&a[]=g0&b=g&c[k]=g', [$form, ['Cookie', 'a[z]=c; b=c; d[]=c']], 'a[y]=p&a[]=p0&b=p&c=p'];
    $cases[] = ['query-max-vars', 'GET', '/?'.implode('&', array_map(static fn ($i) => "v$i=$i", range(1, 1002))), [], ''];
    $cases[] = ['post-file-name-nesting', 'POST', '/upload', [$mp()], multipart('Lab', [$file('h[i]', '9.txt', '9'), $file('h[i][j]', '10.txt', '10'), $file('k[l][m]', '11.txt', '11'), $file('k[l]', '12.txt', '12'), $file('s', 's.txt', 's'), $file('s[t]', 't.txt', 't')])];
    $cases[] = ['post-file-20-then-empty', 'POST', '/upload', [$mp()], multipart('Lab', array_merge(array_map(static fn ($i) => $file("f$i", "f$i.txt", (string) $i), range(1, 20)), [$file('empty', '', ''), $field('after', 'kept')]))];
    $cases[] = ['post-file-19-empty-2', 'POST', '/upload', [$mp()], multipart('Lab', array_merge(array_map(static fn ($i) => $file("f$i", "f$i.txt", (string) $i), range(1, 19)), [$file('empty', '', ''), $file('g1', 'g1.txt', 'g'), $file('g2', 'g2.txt', 'g'), $file('empty2', '', '')]))];
    $cases[] = ['post-file-max-file-size-values', 'POST', '/upload', [$mp()], multipart('Lab', [$field('MAX_FILE_SIZE', 'abc'), $file('a', 'a.txt', '12'), $field('MAX_FILE_SIZE', '0'), $file('b', 'b.txt', '12'), $field('MAX_FILE_SIZE', '-1'), $file('c', 'c.txt', '12'), $field('MAX_FILE_SIZE', '1k'), $file('d', 'd.txt', '12'), $field('MAX_FILE_SIZE', ' 1'), $file('e', 'e.txt', '12'), $field('max_file_size', '1'), $file('f', 'f.txt', '12'), $field('MAX_FILE_SIZE', '1'), $file('g', '', '')])];
    $cases[] = ['post-file-headers', 'POST', '/upload', [$mp()], multipart('Lab', [['name="a"; filename="a.txt"', ['content-type: TEXT/Plain', 'X-Other: 1'], 'a'], ['name="b"; filename="b.txt"', ['Content-Type:'], 'b'], ['name="c"; filename="c.txt"', ['Content-Type: a', 'Content-Type: b'], 'c'], ['name="d"; filename="d.txt"', ['Content-Type: multi', "\tline"], 'd'], ['name="e"; filename="e.txt"', ["Content-Type:\tx/y ;z"], 'e'], ['name="f"; filename="f.txt"', ['Content-Type: x/y,z'], 'f']])];
    $cases[] = ['argv-plus', 'GET', '/?+a++b+&c=d+e', [], ''];
    $cases[] = ['argv-post', 'POST', '/x?a+b', [$form], 'c=d'];
    $cases[] = ['env-override', 'GET', '/', [], '', ['params' => [['HOME', '/override'], ['PATH', '/p'], ['PHP_SELF', '/self'], ['argv', 'x'], ['REQUEST_TIME', '1']]]];
    foreach (['Digest username="u", realm="r"', 'Bearer token', 'Basic !!!', 'basic dXNlcjpwYXNz', 'Basic dXNlcg==', 'Basic dXNlcjo=', 'Basic OnBhc3M=', 'Basic  dXNlcjpwYXNz', 'BASIC dXNlcjpwYXNz', 'Basic dXNlcjpwYXNz extra', 'Digest', 'digest x', 'Basic'] as $i => $auth) {
        $cases[] = ["auth-$i", 'GET', '/', [['Authorization', $auth]], ''];
    }
    $cases[] = ['auth-param', 'GET', '/', [], '', ['params' => [['PHP_AUTH_USER', 'given'], ['HTTP_AUTHORIZATION', 'Basic dXNlcjpwYXNz']]]];
    $cases[] = ['auth-redirect', 'GET', '/', [], '', ['params' => [['REDIRECT_HTTP_AUTHORIZATION', 'Basic dXNlcjpwYXNz']]]];
    $cases[] = ['script-path', 'GET', '/sub/dir/', [], '', ['params' => [['PATH_INFO', '/extra'], ['ORIG_PATH_INFO', '/orig']]]];

    return $cases;
}

mt_srand(20261125);

function pick(array $a): mixed
{
    return $a[mt_rand(0, count($a) - 1)];
}

/** A generated script of output calls (texts base64, as the front controller decodes them). */
function script(): array
{
    $b = static fn (string $s) => base64_encode($s);
    $names = ['X-A', 'x-a', 'X-B', 'Content-Type', 'content-type', 'Location', 'Set-Cookie', 'Cache-Control', 'Status', 'X-Powered-By', 'Content-Length', 'Date', 'Vary', 'WWW-Authenticate'];
    $values = ['1', 'two', 'text/plain', 'application/json', '/next', 'https://example.com/x?y=1', 'a=b; path=/', 'no-cache, private', '404 Gone', '0', '', ' spaced ', 'é'];
    $codes = [0, 0, 0, 200, 201, 204, 299, 301, 302, 303, 304, 307, 404, 418, 500, 503, 600, 99];
    $lines = [
        'HTTP/1.0 404 Not Found', 'HTTP/1.1 200 OK', 'HTTP/1.1 201 Created', 'HTTP/1.0 302 Found', 'HTTP/1.1 303 See Other',
        'HTTP/1.1 418 I\'m a teapot', 'HTTP/1.1 299 Custom Text', 'HTTP/1.1 500', 'HTTP/2 204 No Content', 'HTTP/1.0 301 Moved Permanently',
        'HTTP/1.1 304 Not Modified', 'HTTP/1.1  200  OK', 'HTTP/1.1 abc Weird', 'http/1.1 404 lower', 'HTTP/1.1 999 Big',
    ];
    $bad = ["X-Bad: a\r\nInjected: b", "X-Bad: a\nb", "X-Nul: a\0b", 'NoColon', ': empty name', 'X-Empty:', 'X-Space :v', ' Leading: space', 'X-Tab:	tab', "X-Trailing: v\r\n", 'X-Colon: a:b', ''];
    $script = [];
    $n = mt_rand(1, 9);
    for ($i = 0; $i < $n; ++$i) {
        $r = mt_rand(0, 99);
        if ($r < 40) {
            $script[] = ['header', $b(pick($names).': '.pick($values)), (bool) mt_rand(0, 1), pick($codes)];
        } elseif ($r < 52) {
            $script[] = ['header', $b(pick($lines)), (bool) mt_rand(0, 1), pick($codes)];
        } elseif ($r < 58) {
            $script[] = ['header', $b(pick($bad)), (bool) mt_rand(0, 1), pick($codes)];
        } elseif ($r < 64) {
            $script[] = ['header_remove', mt_rand(0, 5) ? $b(pick($names)) : null];
        } elseif ($r < 68) {
            $script[] = ['http_response_code', pick([200, 201, 302, 404, 500, 299])];
        } elseif ($r < 76) {
            $script[] = ['echo', $b(pick(['body', '', "line\n", '<p>é</p>']))];
        } elseif ($r < 80) {
            $script[] = ['ini_set', $b(pick(['default_mimetype', 'default_charset'])), $b(pick(['', 'text/plain', 'application/json', 'ISO-8859-1', 'UTF-8']))];
        } elseif ($r < 84) {
            $script[] = [pick(['setcookie', 'setrawcookie']), $b(pick(['a', 'b c', 'x=y', 'name'])), $b(pick(['v', 'a b', 'é;,', ''])), 0, $b(pick(['', '/', '/p'])), $b(pick(['', 'example.com'])), (bool) mt_rand(0, 1), (bool) mt_rand(0, 1)];
        } elseif ($r < 90) {
            $script[] = [pick(['headers_list', 'headers_sent', 'response_code'])];
        } elseif ($r < 94) {
            $script[] = [pick(['ob_start', 'ob_end_flush', 'ob_end_clean', 'flush'])];
        } elseif ($r < 96) {
            $script[] = ['fastcgi_finish_request'];
        } else {
            $script[] = ['headers_list'];
        }
    }
    $script[] = ['headers_list'];

    return $script;
}

/** What Response::send() asks of the SAPI for a typical response (Symfony's order). */
function response_scripts(): array
{
    $b = static fn (string $s) => base64_encode($s);

    $scripts = [
        ['symfony-json', [['header', $b('Cache-Control: no-cache, private'), false, 200], ['header', $b('Date: Sat, 12 Nov 1955 20:04:00 GMT'), false, 200], ['header', $b('Content-Type: application/json'), true, 200], ['header', $b('HTTP/1.1 200 OK'), true, 200], ['echo', $b('{"a":1}')], ['fastcgi_finish_request']]],
        ['symfony-empty-404', [['header', $b('Cache-Control: no-cache, private'), false, 404], ['header', $b('Content-Type: text/html; charset=UTF-8'), true, 404], ['header', $b('HTTP/1.1 404 Not Found'), true, 404], ['echo', $b('')], ['fastcgi_finish_request']]],
        ['symfony-405-allow', [['header', $b('Allow: GET'), false, 405], ['header', $b('Cache-Control: no-cache, private'), false, 405], ['header', $b('HTTP/1.1 405 Method Not Allowed'), true, 405], ['echo', $b('')]]],
        ['symfony-redirect', [['header', $b('Location: /inspect'), false, 303], ['header', $b('Cache-Control: no-cache, private'), false, 303], ['header', $b('HTTP/1.1 303 See Other'), true, 303]]],
        ['symfony-redirect-201', [['header', $b('Location: /new'), false, 201], ['header', $b('HTTP/1.1 201 Created'), true, 201]]],
        ['symfony-204', [['ini_set', $b('default_mimetype'), $b('')], ['header', $b('Cache-Control: no-cache, private'), false, 204], ['header', $b('HTTP/1.1 204 No Content'), true, 204], ['echo', $b('')]]],
        ['symfony-304', [['ini_set', $b('default_mimetype'), $b('')], ['header', $b('HTTP/1.1 304 Not Modified'), true, 304]]],
        ['symfony-cookies', [['header', $b('Cache-Control: no-cache, private'), false, 200], ['header', $b('Set-Cookie: a=1; path=/; httponly; samesite=lax'), false, 200], ['header', $b('Set-Cookie: b=2; path=/'), false, 200], ['header', $b('HTTP/1.1 200 OK'), true, 200], ['echo', $b('ok')]]],
        ['symfony-multi-values', [['header', $b('Vary: Accept'), false, 200], ['header', $b('Vary: Accept-Encoding'), false, 200], ['header', $b('X-Same: 1'), false, 200], ['header', $b('x-same: 2'), false, 200], ['header', $b('HTTP/1.0 200 OK'), true, 200]]],
        ['location-no-code', [['header', $b('Location: /x'), true, 0]]],
        ['location-after-201', [['http_response_code', 201], ['header', $b('Location: /x'), true, 0]]],
        ['location-after-404', [['http_response_code', 404], ['header', $b('Location: /x'), true, 0]]],
        ['location-then-status', [['header', $b('Location: /x'), true, 0], ['header', $b('HTTP/1.1 200 OK'), true, 200]]],
        ['location-with-code-200', [['header', $b('Location: /x'), false, 200]]],
        ['www-authenticate', [['header', $b('WWW-Authenticate: Basic'), true, 0]]],
        ['www-authenticate-200', [['http_response_code', 200], ['header', $b('WWW-Authenticate: Basic'), true, 0]]],
        ['echo-then-header', [['echo', $b('early')], ['header', $b('X-Late: 1'), true, 0], ['headers_sent'], ['headers_list']]],
        ['ob-echo-then-header', [['ob_start'], ['echo', $b('buffered')], ['header', $b('X-Late: 1'), true, 0], ['headers_sent'], ['ob_end_flush'], ['headers_sent']]],
        ['ob-clean', [['ob_start'], ['echo', $b('dropped')], ['ob_end_clean'], ['echo', $b('kept')]]],
        ['finish-then-echo', [['echo', $b('first')], ['fastcgi_finish_request'], ['echo', $b('after')], ['header', $b('X-After: 1'), true, 0]]],
        ['remove-all', [['header', $b('X-A: 1'), true, 0], ['header', $b('X-B: 2'), true, 0], ['header_remove', null], ['headers_list']]],
        ['remove-powered-by', [['header_remove', $b('X-Powered-By')], ['headers_list']]],
        ['default-charset-empty', [['ini_set', $b('default_charset'), $b('')]]],
        ['default-mimetype-json', [['ini_set', $b('default_mimetype'), $b('application/json')]]],
        ['default-mimetype-text-charset', [['ini_set', $b('default_mimetype'), $b('text/plain; charset=x')]]],
        ['content-type-no-charset', [['header', $b('Content-Type: text/plain'), true, 0]]],
        ['content-type-twice-append', [['header', $b('Content-Type: text/plain'), false, 0], ['header', $b('Content-Type: text/csv'), false, 0]]],
        ['status-header', [['header', $b('Status: 404 Gone'), true, 0]]],
        ['status-header-lower', [['header', $b('status: 201'), true, 0]]],
        ['response-code-only', [['http_response_code', 404]]],
        ['response-code-unknown', [['http_response_code', 299]]],
        ['header-code-unknown', [['header', $b('X-A: 1'), true, 299]]],
        ['status-line-then-code', [['header', $b('HTTP/1.1 299 Custom'), true, 0], ['http_response_code', 404], ['response_code']]],
        ['code-then-status-line', [['http_response_code', 404], ['header', $b('HTTP/1.1 299 Custom'), true, 0], ['response_code']]],
        ['setcookie-attrs', [['setcookie', $b('n'), $b('v w'), 0, $b('/p'), $b('example.com'), true, true], ['setrawcookie', $b('r'), $b('v%20w'), 0, $b(''), $b(''), false, false], ['headers_list']]],
        ['setcookie-invalid-name', [['setcookie', $b('a=b'), $b('v'), 0, $b(''), $b(''), false, false], ['setrawcookie', $b('c'), $b('v w'), 0, $b(''), $b(''), false, false], ['setcookie', $b(''), $b('v'), 0, $b(''), $b(''), false, false], ['headers_list']]],
        ['replace-order', [['header', $b('X-A: 1'), true, 0], ['header', $b('X-B: 2'), true, 0], ['header', $b('x-a: 3'), true, 0], ['headers_list']]],
        ['replace-false-then-true', [['header', $b('X-A: 1'), false, 0], ['header', $b('X-A: 2'), false, 0], ['header', $b('X-A: 3'), true, 0], ['headers_list']]],
        ['status-line-200-then-location', [['header', $b('HTTP/1.1 200 OK'), true, 0], ['header', $b('Location: /x'), true, 0], ['response_code']]],
        ['status-line-404-then-code-200', [['header', $b('HTTP/1.1 404 Not Found'), true, 0], ['http_response_code', 200]]],
        ['status-line-301-then-location', [['header', $b('HTTP/1.1 301 Moved Permanently'), true, 0], ['header', $b('location: /x'), true, 0]]],
        ['location-empty', [['header', $b('Location:'), true, 0]]],
        ['location-after-307-line', [['header', $b('HTTP/1.1 307 Temporary Redirect'), true, 0], ['header', $b('Location: /x'), true, 0]]],
        ['www-authenticate-after-201', [['http_response_code', 201], ['header', $b('WWW-Authenticate: Basic'), true, 0]]],
        ['www-authenticate-after-404', [['http_response_code', 404], ['header', $b('www-authenticate: Basic'), true, 0]]],
        ['www-authenticate-code', [['header', $b('WWW-Authenticate: Basic'), true, 403]]],
        ['remove-case', [['header', $b('X-A: 1'), true, 0], ['header_remove', $b('x-a')], ['header', $b('X-B: 1'), true, 0], ['header_remove', $b('X-B ')], ['header_remove', $b('Content-Type')], ['headers_list']]],
        ['remove-all-then-echo', [['header_remove', null], ['echo', $b('x')], ['headers_list']]],
        ['remove-content-type', [['header', $b('Content-Type: application/json'), true, 0], ['header_remove', $b('content-type')]]],
        ['powered-by-replace', [['header', $b('X-Powered-By: Rust'), true, 0], ['headers_list']]],
    ];
    $more = [];
    foreach (['HTTP/1.1  200  OK', 'HTTP/1.1 abc Weird', 'http/1.1 404 lower', 'HTTP/1.1 999 Big', 'HTTP/2 204 No Content', 'HTTP/1.1 600', 'HTTP/1.1 99 Low', 'HTTP/1.1 200', 'HTTP/1.1 404', 'HTTP/1.1 302', 'HTTP/1.1 404 Not Found extra words', 'HTTP/1.1 404Not', 'HTTP/ 404 x', 'HTTP 404 x', 'HTTP/1.1', 'HTTP/1.1 ', 'HTTP/1.1 0 Zero', 'HTTP/1.1 -5 Neg', 'HTTP/1.1 4040 Long', 'HTTP/1.1 +404 Plus', 'HTTPS/1.1 404 x', 'HTTP/1.1 404 x', 'HTTP/1.1 201 Created', 'xHTTP/1.1 404 x', "HTTP/1.1\t404 tab", 'HTTP/1.1 0404 Lead', 'HTTP/1.1 404 ', 'HTTP/1.1 100 Continue', 'HTTP/1.1 101'] as $i => $line) {
        $more[] = ["status-line-$i", [['header', $b($line), true, 0], ['response_code']]];
        $more[] = ["status-line-$i-code", [['header', $b($line), true, 418], ['response_code']]];
    }
    foreach (['Content-Type: TEXT/PLAIN', 'Content-Type: application/xml', 'Content-Type: text/plain; charset=x', 'Content-Type: text/plain;Charset=x', 'content-type:text/plain', 'Content-Type: image/svg+xml', 'Content-Type: application/javascript', 'Content-Type:  text/csv ', 'Content-Type: text', 'Content-Type: text/', 'Content-Type: ', 'Content-Type:', 'Content-type: text/xml; x=charset', 'CONTENT-TYPE: text/plain', 'Content-Type: multipart/x; charset=y', 'Content-Type: text/plain; Charset', 'Content-Type : text/plain', 'Content-Type:text/html'] as $i => $line) {
        $more[] = ["content-type-$i", [['header', $b($line), true, 0], ['headers_list']]];
        $more[] = ["content-type-$i-no-charset", [['ini_set', $b('default_charset'), $b('')], ['header', $b($line), true, 0], ['headers_list']]];
    }
    foreach (["X-Bad: a\r\nInjected: b", "X-Bad: a\nb", "X-Bad: a\rb", "X-Nul: a\0b", 'NoColon', ': empty name', 'X-Empty:', 'X-Space :v', ' Leading: space', "X-Tab:\ttab", "X-Trailing: v\r\n", "X-Trailing2: v\n", "X-Trailing3: v\r", 'X-Colon: a:b', '', ' ', "\r\n", 'X-A:    many', "X-A:\t\tTabs", 'X-É: v', 'X-A: é', "X-A: v \t", "\tX-A: v"] as $i => $line) {
        $more[] = ["bad-$i", [['header', $b($line), true, 0], ['headers_list']]];
    }
    foreach (["X-A\r\nB", 'X-Nope', '', 'x-powered-by', 'X-A: 1', 'X-A:'] as $i => $name) {
        $more[] = ["remove-$i", [['header', $b('X-A: 1'), true, 0], ['header_remove', $b($name)], ['headers_list']]];
    }

    return array_merge($scripts, $more);
}

$out = ['requests' => [], 'scripts' => []];

/**
 * Bytes as base64, or a long body as [before, byte, count, after] around
 * its longest run of one byte, so the vectors stay small.
 */
function recorder_bytes(string $s): string|array
{
    if (strlen($s) <= 4096) {
        return base64_encode($s);
    }
    $best = [0, 0];
    $at = 0;
    while ($at < strlen($s)) {
        $run = strspn($s, $s[$at], $at);
        if ($run > $best[1]) {
            $best = [$at, $run];
        }
        $at += $run;
    }

    return [base64_encode(substr($s, 0, $best[0])), ord($s[$best[0]]), $best[1], base64_encode(substr($s, $best[0] + $best[1]))];
}

function run_case(string $name, array $params, string $stdin, array $script, bool $parseBody): array
{
    @unlink('/tmp/oracle/record.json');
    file_put_contents('/tmp/oracle/current.json', json_encode(['script' => $script, 'parse_body' => $parseBody]));
    [$stdout, $stderr, $status] = fcgi_request($params, $stdin);
    // After fastcgi_finish_request() the worker runs on: wait for its record.
    for ($i = 0; $i < 300 && !file_exists('/tmp/oracle/record.json'); ++$i) {
        usleep(10000);
    }
    usleep(20000);
    $record = json_decode((string) @file_get_contents('/tmp/oracle/record.json'), true);

    return [
        'name' => $name,
        'params' => array_map(static fn ($p) => [base64_encode($p[0]), base64_encode($p[1])], $params),
        'stdin' => recorder_bytes($stdin),
        'script' => $script,
        'record' => $record,
        'stdout' => base64_encode($stdout),
        'stderr' => base64_encode($stderr),
        'status' => $status,
    ];
}

foreach (request_cases() as $case) {
    [$name, $method, $target, $headers, $body] = $case;
    $extra = $case[5] ?? [];
    if (!array_filter($headers, static fn ($h) => 0 === strcasecmp($h[0], 'Content-Length')) && '' !== $body) {
        $headers[] = ['Content-Length', (string) strlen($body)];
    }
    // A Content-Length the case gives as 0 is the body's own length.
    $headers = array_map(static fn ($h) => 0 === strcasecmp($h[0], 'Content-Length') ? [$h[0], (string) strlen($body)] : $h, $headers);
    $params = nginx_params($method, $target, $headers, $body, $extra);
    $parse = in_array($method, ['PUT', 'DELETE', 'PATCH', 'QUERY'], true);
    $out['requests'][] = run_case($name, $params, $body, [['headers_list']], $parse);
}

$get = nginx_params('GET', '/', [['Host', 'lab.example']], '');
$head = nginx_params('HEAD', '/', [['Host', 'lab.example']], '');
foreach (response_scripts() as [$name, $script]) {
    $out['scripts'][] = run_case($name, $get, '', $script, false);
    $out['scripts'][] = run_case("$name-head", $head, '', $script, false);
}
for ($i = 0; $i < 600; ++$i) {
    $out['scripts'][] = run_case("generated-$i", $i % 7 ? $get : $head, '', script(), false);
}

exec('kill $(cat /usr/local/var/run/php-fpm.pid 2>/dev/null) 2>/dev/null; pkill php-fpm 2>/dev/null');
echo json_encode($out, JSON_UNESCAPED_SLASHES), "\n";
