<?php
// ServerBag::getHeaders() answers for what ServerBagTest leaves open, for
// crates/http-foundation/tests/data/server_bag.json (scripts/record.sh
// http-foundation): every combination of a few $_SERVER entries,
// Authorization texts and odd values, with the headers and the bag's
// parameters afterwards (a Digest header is stored as PHP_AUTH_DIGEST).
// Values are typed (lib/typed.php).

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\HttpFoundation\ServerBag;

$auth = [null, 'Basic '.base64_encode('u:p'), 'basic '.base64_encode('u:p'), 'BASIC '.base64_encode('u'), "Basic\t".base64_encode('u:p'),
    'Basic Zm9v=OmJhcg==', 'Basic !!!', 'Basic ', 'Basic', 'Digest a=1', 'digest x', 'DIGEST', 'Bearer t', 'bearer ', 'Other x', '', '0', 42, ['x'], true];
$extra = [[], ['PHP_AUTH_USER' => 'me'], ['PHP_AUTH_USER' => 'me', 'PHP_AUTH_PW' => null], ['PHP_AUTH_USER' => null], ['PHP_AUTH_DIGEST' => 'd'], ['PHP_AUTH_DIGEST' => '0'],
    ['CONTENT_TYPE' => 'a/b', 'CONTENT_LENGTH' => 0, 'CONTENT_MD5' => ''], ['HTTP_X' => null, 'HTTP_123' => 'n', 'HTTP_' => 'e', 'http_low' => 'l', 7 => 'int'], ['PHP_AUTH_USER' => ['a'], 'PHP_AUTH_PW' => 3]];

$cases = [];
foreach ($auth as $a) {
    foreach ([false, true] as $redirect) {
        foreach ($extra as $e) {
            $server = $e;
            if (null !== $a) {
                $server[$redirect ? 'REDIRECT_HTTP_AUTHORIZATION' : 'HTTP_AUTHORIZATION'] = $a;
            }
            $bag = new ServerBag($server);
            $cases[] = [typed_value($server), typed_call(static fn () => $bag->getHeaders()), typed_value($bag->all())];
        }
    }
}

echo json_encode($cases, JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR), "\n";
