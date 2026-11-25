<?php
// The front controller runtime_fpm.php serves through PHP-FPM, copied to
// /srv/app/public/index.php. It loads no package: it records what PHP-FPM
// made of the request (the superglobals, php://input, request_parse_body(),
// the uploaded files), then runs the case's script of output calls
// (header(), header_remove(), http_response_code(), echo, setcookie(),
// output buffers, fastcgi_finish_request(), ...), noting headers_list(),
// headers_sent() and http_response_code() where the script asks and every
// warning. The record goes to /tmp/oracle/record.json at shutdown; the
// response itself is what the FastCGI client receives.

$recorderCase = json_decode(file_get_contents('/tmp/oracle/current.json'), true);
$recorderRecord = ['warnings' => [], 'answers' => []];

set_error_handler(static function (int $level, string $message) use (&$recorderRecord): bool {
    $recorderRecord['warnings'][] = [$level, $message];

    return true;
});

function recorder_value(mixed $v): mixed
{
    if (is_array($v)) {
        $pairs = [];
        foreach ($v as $k => $x) {
            $pairs[] = [is_int($k) ? $k : 's:'.base64_encode($k), recorder_value($x)];
        }

        return $pairs;
    }
    if (is_string($v)) {
        return strlen($v) > 4096 ? ['long' => recorder_bytes($v)] : 's:'.base64_encode($v);
    }

    return $v;
}

// Bytes as base64, or a long file as [before, byte, count, after] around
// its longest run of one byte (as runtime_fpm.php writes long bodies).
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

$recorderTmp = [];
// $_FILES keeps tmp_name under its entry or, for array names, in the
// tmp_name branch: walk the whole tree.
$recorderFiles = $_FILES;
array_walk_recursive($recorderFiles, static function (&$value) use (&$recorderTmp) {
    if (is_string($value) && preg_match('#^/tmp/php#', $value)) {
        $recorderTmp[] = [
            'pattern' => preg_replace('#[A-Za-z0-9]#', 'x', $value),
            'uploaded' => is_uploaded_file($value),
            'content' => recorder_bytes((string) @file_get_contents($value)),
        ];
        $value = '#'.(count($recorderTmp) - 1);
    }
});

$server = $_SERVER;
$recorderRecord['request_time'] = [
    is_int($server['REQUEST_TIME'] ?? null),
    is_float($server['REQUEST_TIME_FLOAT'] ?? null),
];
$server['REQUEST_TIME'] = 0;
$server['REQUEST_TIME_FLOAT'] = 0;

$recorderRecord['server'] = recorder_value($server);
$recorderRecord['get'] = recorder_value($_GET);
$recorderRecord['post'] = recorder_value($_POST);
$recorderRecord['cookie'] = recorder_value($_COOKIE);
$recorderRecord['request'] = recorder_value($_REQUEST);
$recorderRecord['files'] = recorder_value($recorderFiles);

if ($recorderCase['parse_body']) {
    try {
        [$post, $files] = request_parse_body();
        array_walk_recursive($files, static function (&$value) use (&$recorderTmp) {
            if (is_string($value) && preg_match('#^/tmp/php#', $value)) {
                $recorderTmp[] = [
                    'pattern' => preg_replace('#[A-Za-z0-9]#', 'x', $value),
                    'uploaded' => is_uploaded_file($value),
                    'content' => recorder_bytes((string) @file_get_contents($value)),
                ];
                $value = '#'.(count($recorderTmp) - 1);
            }
        });
        $recorderRecord['parsed_body'] = [recorder_value($post), recorder_value($files)];
    } catch (\Throwable $e) {
        $recorderRecord['parsed_body'] = ['throws', $e::class, $e->getMessage()];
    }
}
$recorderRecord['input'] = recorder_bytes(file_get_contents('php://input'));
$recorderRecord['tmp'] = $recorderTmp;
$recorderRecord['headers_sent_at_start'] = headers_sent();

register_shutdown_function(static function () use (&$recorderRecord) {
    $recorderRecord['final_list'] = array_map('base64_encode', headers_list());
    $recorderRecord['final_code'] = http_response_code();
    file_put_contents('/tmp/oracle/record.json', json_encode($recorderRecord));
});

foreach ($recorderCase['script'] as $call) {
    $args = array_map(static fn ($a) => is_string($a) ? base64_decode($a) : $a, array_slice($call, 1));
    switch ($call[0]) {
        case 'header': header($args[0], $args[1], $args[2]); break;
        case 'header_remove': null === $args[0] ? header_remove() : header_remove($args[0]); break;
        case 'http_response_code': $recorderRecord['answers'][] = http_response_code($args[0]); break;
        case 'echo': echo $args[0]; break;
        case 'ini_set': $recorderRecord['answers'][] = ini_set($args[0], $args[1]); break;
        case 'setcookie': $recorderRecord['answers'][] = setcookie($args[0], $args[1], $args[2], $args[3], $args[4], $args[5], $args[6]); break;
        case 'setrawcookie': $recorderRecord['answers'][] = setrawcookie($args[0], $args[1], $args[2], $args[3], $args[4], $args[5], $args[6]); break;
        case 'headers_list': $recorderRecord['answers'][] = array_map('base64_encode', headers_list()); break;
        case 'headers_sent': $recorderRecord['answers'][] = headers_sent(); break;
        case 'response_code': $recorderRecord['answers'][] = http_response_code(); break;
        case 'ob_start': ob_start(); break;
        case 'ob_end_flush': $recorderRecord['answers'][] = @ob_end_flush(); break;
        case 'ob_end_clean': $recorderRecord['answers'][] = @ob_end_clean(); break;
        case 'flush': flush(); break;
        case 'fastcgi_finish_request': $recorderRecord['answers'][] = fastcgi_finish_request(); break;
    }
}
