<?php
// BinaryFileResponse answers for what its tests leave open, for
// crates/http-foundation/tests/data/binary_file.json
// (scripts/record.sh http-foundation-binary-file): 1,500 seeded scripts.
// Each script recreates a directory of files: contents from a pool
// (empty, one byte, text, 35 bytes, 20,000 bytes), names from a pool
// (non-ASCII, '%', a space, no extension), and set modification times.
// It builds a BinaryFileResponse from a path, a File, a File\Stream, an
// SplTempFileObject, a file:// URL or a missing path, with a status,
// headers, public, a disposition, autoEtag and autoLastModified from
// pools; trusts X-Sendfile-Type or not (reset by reflection between
// scripts). Then it runs operations: setContentDisposition() (with
// filenames that are UTF-8, ISO-8859-1, or hold '%' or control
// characters), setChunkSize(), deleteFileAfterSend(), setAutoEtag(),
// setAutoLastModified(), setContent(), getContent(), prepare() with a
// request (method; Range, If-Range taken from the response's ETag or
// Last-Modified or not, X-Sendfile-Type, X-Accel-Mapping), and
// sendContent(), with connection_aborted() true from the start in some
// scripts. Recorded: each operation's result; for sendContent() the
// length of each echo and the CRC32 of what was echoed; then the status,
// the headers, the file's pathname and whether it still exists. The
// directory's path is written as {dir}. Texts are base64 (lib/typed.php).

namespace Symfony\Component\HttpFoundation {
    const RECORDER_NOW = 1700000000;

    function time(): int
    {
        return RECORDER_NOW;
    }

    function gmdate(string $format, ?int $timestamp = null): string
    {
        return \gmdate($format, $timestamp ?? RECORDER_NOW);
    }

    function connection_aborted(): int
    {
        return $GLOBALS['recorder_aborted'] ? 1 : 0;
    }
}

namespace {
    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';
    require __DIR__.'/lib/typed.php';

    use Symfony\Component\HttpFoundation\BinaryFileResponse;
    use Symfony\Component\HttpFoundation\File\File;
    use Symfony\Component\HttpFoundation\File\Stream;
    use Symfony\Component\HttpFoundation\Request;

    mt_srand(20261129);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];

    $dir = sys_get_temp_dir().'/bfr-recorder';
    $big = '';
    for ($i = 0; $i < 20000; ++$i) {
        $big .= \chr(($i * 7 + 3) % 256);
    }
    $contents = ['', 'A', "hello world\n", '0123456789ABCDEFGHIJKLMNOPQRSTUVWXY', $big];
    $names = ['a.txt', 'data.bin', 'fööö.html', 'per%cent.txt', 'sp ace.csv', 'noext'];
    $mtimes = [1600000000, 1699999000, 1700000000];

    $normalize = static function ($v) use (&$normalize, $dir) {
        if (\is_string($v)) {
            return str_replace([$dir, rawurlencode($dir)], ['{dir}', '{dir}'], $v);
        }
        if (\is_array($v)) {
            return array_map($normalize, $v);
        }

        return $v;
    };
    $thrown = static fn (\Throwable $e) => ['throws', $e::class, base64_encode($normalize($e->getMessage()))];

    $ranges = ['bytes=0-', 'bytes=-0', 'bytes=0-0', 'bytes=5-', 'bytes=-5', 'bytes=3-9', 'bytes=10-20', 'bytes=34-34', 'bytes=35-', 'bytes=40-50', 'bytes=0-99999999999999999999', 'bytes=99999999999999999999-', 'BYTES=1-2', "bytes=1-2\n", 'bytes=-', 'bytes=1-2,3-4', 'items=1-2', 'bytes= 1-2', 'bytes=16383-16385', 'bytes=19990-'];
    $sendfileTypes = ['X-Sendfile', 'x-accel-redirect', 'X-Accel-Redirect', 'X-LIGHTTPD-send-file', 'X-Leak-Path', ''];
    $mappings = ['{dir}/=/files/', '/nowhere/=/x/,{dir}=/y', '"{dir}/"="/q/"', '/nowhere/=/x/', '{dir}', ''];
    $dispositions = [null, null, null, null, 'attachment', 'inline', '0', null, null, 'attachment', 'inline', 'bogus'];
    $filenames = ['', '', 'föö.html', "f\xf6\xf6.html", "foo\n", 'a%b.txt', 'plain.txt', "tab\there", 'é'];
    $fallbacks = ['', '', 'fallback.txt', "foo\n", 'f%b', 'é', 'a/b'];

    $scripts = [];
    for ($n = 0; $n < 1500; ++$n) {
        // A fresh directory of files.
        if (is_dir($dir)) {
            foreach (scandir($dir) as $f) {
                if ('.' !== $f && '..' !== $f) {
                    unlink("$dir/$f");
                }
            }
        } else {
            mkdir($dir);
        }
        $files = [];
        foreach ($names as $i => $name) {
            $c = mt_rand(0, \count($contents) - 1);
            $m = $pick($mtimes);
            file_put_contents("$dir/$name", $contents[$c]);
            touch("$dir/$name", $m);
            $files[] = [base64_encode($name), $c, $m];
        }

        $kind = $pick(['path', 'path', 'path', 'file', 'stream', 'temp', 'url', 'missing']);
        $name = $pick($names);
        $tempContent = mt_rand(0, \count($contents) - 1);
        $status = $pick([200, 200, 200, 200, 200, 200, 200, 201, 204, 304, 404, 500]);
        $headersIn = [];
        if (0 === mt_rand(0, 1)) {
            $headersIn[] = ['Content-Type', $pick(['application/octet-stream', 'text/plain', 'text/csv'])];
        }
        if (0 === mt_rand(0, 5)) {
            $headersIn[] = ['Accept-Ranges', 'foo'];
        }
        if (0 === mt_rand(0, 6)) {
            $headersIn[] = ['Transfer-Encoding', 'chunked'];
        }
        $public = 0 !== mt_rand(0, 3);
        $disposition = $pick($dispositions);
        $autoEtag = 0 === mt_rand(0, 1);
        $autoLastModified = 0 !== mt_rand(0, 3);
        $trust = 0 === mt_rand(0, 2);
        $aborted = 0 === mt_rand(0, 9);

        $ops = [];
        for ($i = mt_rand(0, 3); $i > 0; --$i) {
            $ops[] = match ($pick(['disposition', 'disposition', 'chunk', 'delete', 'etag', 'lm', 'set_content', 'get_content'])) {
                'disposition' => ['disposition', $pick(['attachment', 'inline', 'bogus']), base64_encode($pick($filenames)), base64_encode($pick($fallbacks))],
                'chunk' => ['chunk', $pick([0, 1, 7, 100, 16384])],
                'delete' => ['delete', 0 !== mt_rand(0, 3)],
                'etag' => ['etag'],
                'lm' => ['lm'],
                'set_content' => ['set_content', $pick([null, 'x'])],
                'get_content' => ['get_content'],
            };
        }
        $method = $pick(['GET', 'GET', 'GET', 'HEAD', 'POST']);
        $requestHeaders = [];
        if (0 !== mt_rand(0, 2)) {
            $requestHeaders[] = ['Range', base64_encode($pick($ranges))];
        }
        if (0 === mt_rand(0, 2)) {
            $requestHeaders[] = ['If-Range', base64_encode($pick(['{etag}', '{etag}', '{lm}', '{lm}', '"nope"', 'Sat, 01 Jan 2000 00:00:00 GMT']))];
        }
        if (0 === mt_rand(0, 2)) {
            $requestHeaders[] = ['X-Sendfile-Type', base64_encode($pick($sendfileTypes))];
            if (0 !== mt_rand(0, 3)) {
                $requestHeaders[] = ['X-Accel-Mapping', base64_encode($pick($mappings))];
            }
        }
        $sends = mt_rand(1, 2);
        if (0 === mt_rand(0, 7)) {
            $sends = 0; // sendContent() without prepare()
        }

        $script = [
            'files' => $files,
            'kind' => $kind,
            'name' => base64_encode($name),
            'temp' => $tempContent,
            'status' => $status,
            'headers_in' => $headersIn,
            'public' => $public,
            'disposition' => $disposition,
            'auto_etag' => $autoEtag,
            'auto_last_modified' => $autoLastModified,
            'trust' => $trust,
            'aborted' => $aborted,
            'ops' => $ops,
            'method' => $method,
            'request_headers' => $requestHeaders,
            'sends' => $sends,
        ];

        \Closure::bind(static function () use ($trust) {
            self::$trustXSendfileTypeHeader = $trust;
        }, null, BinaryFileResponse::class)();
        $GLOBALS['recorder_aborted'] = $aborted;

        $path = "$dir/$name";
        $file = match ($kind) {
            'path' => $path,
            'file' => new File($path),
            'stream' => new Stream($path),
            'temp' => (static function () use ($contents, $tempContent) {
                $t = new \SplTempFileObject();
                $t->fwrite($contents[$tempContent]);

                return $t;
            })(),
            'url' => 'file://'.$path,
            'missing' => "$dir/missing.txt",
        };
        $array = [];
        foreach ($headersIn as [$k, $v]) {
            $array[$k] = $v;
        }
        try {
            $response = new BinaryFileResponse($file, $status, $array, $public, $disposition, $autoEtag, $autoLastModified);
        } catch (\Throwable $e) {
            $script['new'] = $thrown($e);
            $scripts[] = $script;
            continue;
        }
        $script['new'] = null;

        $results = [];
        foreach ($ops as $op) {
            try {
                $results[] = match ($op[0]) {
                    'disposition' => ($response->setContentDisposition($op[1], base64_decode($op[2]), base64_decode($op[3])) && false) ?: ['self'],
                    'chunk' => ($response->setChunkSize($op[1]) && false) ?: ['self'],
                    'delete' => ($response->deleteFileAfterSend($op[1]) && false) ?: typed_value($response->shouldDeleteFileAfterSend()),
                    'etag' => ($response->setAutoEtag() && false) ?: ['self'],
                    'lm' => ($response->setAutoLastModified() && false) ?: ['self'],
                    'set_content' => ($response->setContent($op[1]) && false) ?: ['self'],
                    'get_content' => typed_value($response->getContent()),
                };
            } catch (\Throwable $e) {
                $results[] = $thrown($e);
            }
        }

        $request = Request::create('/', $method);
        foreach ($requestHeaders as [$k, $v]) {
            $v = str_replace(['{etag}', '{lm}', '{dir}'], [(string) $response->getEtag(), (string) $response->headers->get('Last-Modified'), $dir], base64_decode($v));
            $request->headers->set($k, $v);
        }
        $prepared = null;
        if ($sends > 0) {
            try {
                $response->prepare($request);
                $prepared = ['self'];
            } catch (\Throwable $e) {
                $prepared = $thrown($e);
            }
        }
        $sent = [];
        for ($i = 0; $i < max(1, $sends); ++$i) {
            $chunks = [];
            ob_start(static function (string $buffer) use (&$chunks) {
                if ('' !== $buffer) {
                    $chunks[] = $buffer;
                }

                return '';
            }, 1);
            try {
                $response->sendContent();
                $result = ['self'];
            } catch (\Throwable $e) {
                $result = $thrown($e);
            }
            ob_end_flush();
            $sent[] = [$result, array_map('strlen', $chunks), \sprintf('%08x', crc32(implode('', $chunks)))];
        }

        $script['results'] = $results;
        $script['prepared'] = $prepared;
        $script['sent'] = $sent;
        $script['status_out'] = $response->getStatusCode();
        $script['headers'] = typed_value($normalize($response->headers->allPreserveCase()));
        $script['pathname'] = base64_encode($normalize($response->getFile()->getPathname()));
        $script['exists'] = 'temp' === $kind ? null : is_file($response->getFile()->getPathname());
        $scripts[] = $script;
    }

    echo json_encode(['scripts' => $scripts], \JSON_THROW_ON_ERROR), "\n";
}
