<?php
// Sends query strings and Cookie headers to PHP and prints what it made of
// them ($_GET, $_POST, $_COOKIE). Run by oracle/recorders/request_vars.py
// inside the oracle image; reads [{"kind", "raw" (base64), "get"}] on stdin.
//
// PHP fills these arrays only for a web request, so this starts the
// built-in server (display_errors=0, defaults otherwise) with
// request_vars_server.php as its index, and writes each request by hand
// over a socket so the bytes arrive unchanged. Prints one answer per case:
// {"get": ..., "post": ...} for a query, {"cookie": ...} for a cookie, and
// {"post": ...} for a Content-Type sent with a fixed urlencoded body.

$cases = json_decode(stream_get_contents(STDIN), true, 512, JSON_THROW_ON_ERROR);

$docroot = sys_get_temp_dir() . '/request-vars-server';
@mkdir($docroot);
copy(__DIR__ . '/request_vars_server.php', "$docroot/index.php");

$port = 8102;
$server = proc_open(
    [PHP_BINARY, '-d', 'display_errors=0', '-d', 'log_errors=0', '-S', "127.0.0.1:$port", '-t', $docroot],
    [0 => ['file', '/dev/null', 'r'], 1 => ['file', '/dev/null', 'w'], 2 => ['file', '/dev/null', 'w']],
    $pipes,
);
for ($try = 0; $try < 100; $try++) {
    $socket = @fsockopen('127.0.0.1', $port);
    if ($socket !== false) {
        fclose($socket);
        break;
    }
    usleep(50_000);
}

function send(int $port, string $request): ?array
{
    $socket = fsockopen('127.0.0.1', $port);
    fwrite($socket, $request);
    $response = stream_get_contents($socket);
    fclose($socket);
    [$head, $body] = explode("\r\n\r\n", $response, 2) + ['', ''];
    if (!str_starts_with($head, 'HTTP/1.1 200') && !str_starts_with($head, 'HTTP/1.0 200')) {
        return ['status' => strtok($head, "\r\n")];
    }
    return json_decode($body, true, 512, JSON_THROW_ON_ERROR);
}

$out = [];
foreach ($cases as $case) {
    $raw = base64_decode($case['raw'], true);
    if ($case['kind'] === 'type') {
        $body = 'a=1&b[]=2';
        $answer = send($port, "POST / HTTP/1.0\r\nHost: oracle\r\nContent-Type: $raw\r\n"
            . 'Content-Length: ' . strlen($body) . "\r\n\r\n$body");
        $out[] = ['post' => $answer['post'] ?? $answer];
        continue;
    }
    if ($case['kind'] === 'cookie') {
        $answer = send($port, "GET / HTTP/1.0\r\nHost: oracle\r\nCookie: $raw\r\n\r\n");
        $out[] = ['cookie' => $answer['cookie'] ?? $answer];
        continue;
    }
    $get = $case['get']
        ? send($port, "GET /?$raw HTTP/1.0\r\nHost: oracle\r\n\r\n")
        : null;
    $post = send($port, "POST / HTTP/1.0\r\nHost: oracle\r\n"
        . "Content-Type: application/x-www-form-urlencoded\r\n"
        . 'Content-Length: ' . strlen($raw) . "\r\n\r\n$raw");
    $out[] = [
        'get' => $get === null ? null : ($get['get'] ?? $get),
        'post' => $post['post'] ?? $post,
    ];
}
proc_terminate($server);
proc_close($server);
echo json_encode($out, JSON_THROW_ON_ERROR), "\n";
