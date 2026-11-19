<?php
// Sends multipart/form-data bodies to PHP's own body parser and prints what
// it made of them. Run by oracle/recorders/multipart.py inside the oracle
// image; reads [{"name", "content_type", "body" (base64)}] on stdin.
//
// PHP parses multipart bodies only for a web request, so this starts the
// built-in server (production display_errors=0, as the recordings always
// used) with multipart_server.php as its index: it answers `$_POST` for
// POST and `request_parse_body()` (what Symfony uses for PUT, DELETE, PATCH
// and QUERY) for PUT. Prints [{"fields", "failed"}], POST then PUT per case.

$cases = json_decode(stream_get_contents(STDIN), true, 512, JSON_THROW_ON_ERROR);

$docroot = sys_get_temp_dir() . '/multipart-server';
@mkdir($docroot);
copy(__DIR__ . '/multipart_server.php', "$docroot/index.php");

$address = '127.0.0.1:8101';
$server = proc_open(
    [PHP_BINARY, '-d', 'display_errors=0', '-d', 'log_errors=0', '-S', $address, '-t', $docroot],
    [0 => ['file', '/dev/null', 'r'], 1 => ['file', '/dev/null', 'w'], 2 => ['file', '/dev/null', 'w']],
    $pipes,
);
for ($try = 0; $try < 100; $try++) {
    $socket = @fsockopen('127.0.0.1', 8101);
    if ($socket !== false) {
        fclose($socket);
        break;
    }
    usleep(50_000);
}

$out = [];
foreach ($cases as $case) {
    $body = base64_decode($case['body'], true);
    foreach (['POST', 'PUT'] as $method) {
        $context = stream_context_create(['http' => [
            'method' => $method,
            'header' => "Content-Type: {$case['content_type']}\r\n",
            'content' => $body,
            'ignore_errors' => true,
        ]]);
        $answer = file_get_contents("http://$address/", false, $context);
        $out[] = json_decode($answer, true, 512, JSON_THROW_ON_ERROR);
    }
}
proc_terminate($server);
proc_close($server);
echo json_encode($out, JSON_THROW_ON_ERROR), "\n";
