<?php
// Index of the built-in server started by request_vars.php: answers what
// PHP put in $_GET, $_POST and $_COOKIE. Arrays become ordered
// [key, value] pairs and strings base64, so order and bytes survive JSON.
function enc($v) {
    if (!is_array($v)) return base64_encode((string) $v);
    $pairs = [];
    foreach ($v as $k => $x) $pairs[] = [is_int($k) ? $k : base64_encode($k), enc($x)];
    return $pairs;
}
header('Content-Type: application/json');
echo json_encode(['get' => enc($_GET), 'post' => enc($_POST), 'cookie' => enc($_COOKIE)]);
