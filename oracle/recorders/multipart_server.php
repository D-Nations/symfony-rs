<?php
function enc($v) {
    if (!is_array($v)) return base64_encode((string) $v);
    $pairs = [];
    foreach ($v as $k => $x) $pairs[] = [(string) $k, enc($x)];
    return $pairs;
}
if ($_SERVER["REQUEST_METHOD"] === "POST") {
    $post = $_POST; $failed = false;
} else {
    try { [$post] = request_parse_body(); $failed = false; }
    catch (\Throwable $e) { $post = []; $failed = true; }
}
header("Content-Type: application/json");
echo json_encode(["fields" => enc($post), "failed" => $failed]);
