<?php
// IpUtils answers for what its tests leave open, for
// crates/http-foundation/tests/data/ip_utils.json (scripts/record.sh
// http-foundation): the PHP built-ins it calls (filter_var() with
// FILTER_VALIDATE_IP, inet_pton(), inet_ntop(), ip2long()) over a string
// corpus, then checkIp4(), checkIp6(), checkIp(), anonymize() and
// isPrivateIp() over requests x subnets, in one process and one order so
// the replay sees the same cache. Strings are base64; a thrown error is
// ["throw", class, message]. Warnings are silenced: the port has none.
//
// With argv[1] = a path to another IpUtils.php (an older release), only the
// isPrivateIp() and checkIp() sections are recorded with that file, for
// checking advisory verdicts (scripts/record.sh ip-utils-prefix).

error_reporting(E_ALL);
set_error_handler(static fn () => true);

if (isset($argv[1])) {
    require $argv[1];
} else {
    require '/demo/vendor/autoload.php';
}

use Symfony\Component\HttpFoundation\IpUtils;

mt_srand(20261122);

function answer(callable $f): mixed
{
    try {
        return $f();
    } catch (\Throwable $e) {
        return ['throw', $e::class, $e->getMessage()];
    }
}

$b = static fn (string $s): string => base64_encode($s);

// Addresses and near-addresses.
$ips = [
    '', ' ', '0', '1', 'x', 'unknown', '*', 'not-an-ip', '2130706433', '0x7f000001',
    '0.0.0.0', '0.0.0.1', '1.2.3.4', '127.0.0.1', '127.1', '127.0.1', '10.0.0.1', '10.0.0.5', '010.0.0.1',
    '01.2.3.4', '1.02.3.4', '1.2.3.04', '00.0.0.0', '1.2.3.4.', '.1.2.3.4', '1.2.3.4.5', '1..2.3', '1.2.3',
    '255.255.255.255', '256.0.0.1', '1.2.3.256', '999.1.1.1', '-1.2.3.4', '+1.2.3.4', '1.2.3.-4',
    ' 1.2.3.4', '1.2.3.4 ', "1.2.3.4\n", "\t1.2.3.4", "1.2.3.4\0", '1.2.3.4/24', '1.2.3.4:80',
    '192.168.1.1', '192.168.52.1', '172.16.0.1', '172.31.255.255', '172.32.0.1', '169.254.0.1',
    '100.64.0.1', '100.127.255.255', '100.128.0.1', '198.18.0.1', '198.20.0.1', '240.0.0.1',
    '8.8.8.8', '104.26.14.6', '192.0.2.1', '203.0.113.1', '198.51.100.1',
    '::', ':::', '::1', '::1:', ':1', '1:', '0::0', '::0', '0:0:0:0:0:0:0:0', '0:0:0:0:0:0:0:1',
    '0000:0000:0000:0000:0000:0000:0000:0001', '00000::1', '1::2::3', '1:2:3:4:5:6:7:8', '1:2:3:4:5:6:7:8:9',
    '1:2:3:4:5:6:7::', '::2:3:4:5:6:7:8', '1:2:3:4:5:6::7:8', '1::8', '1:0:0:0:0:0:0:1', '::1:0:1',
    'fe80::1', 'fe80::1%eth0', 'fe80::1fc4:15d8:78db:2319%enp4s0', 'fc00::1', 'fd00::1', 'fe7f::1', 'febf::1', 'fec0::1',
    '[::1]', '[2a01:198::3]', '[0:0:603:50:396e:4789:8e99:0001]', '[::FFFF:123.234.235.236]', '[1.2.3.4]',
    '::ffff:0:1', '::ffff:10.0.0.5', '::FFFF:10.0.0.5', '::ffff:a00:5', '::ffff:127.0.0.1', '::ffff:8.8.8.8',
    '::ffff:123.234.235.236', '::FFFF:123.234.235.236', '::ffff:7bea:ebec', '0000:0000:0000:0000:0000:ffff:7bea:ebec',
    '0:0:0:0:0:ffff:1.2.3.4', '::ffff:1.2.3', '::ffff:1.2.3.4.5', '::ffff:01.2.3.4', '::ffff:256.1.1.1', "::ffff:10.0.0.5\0",
    '::ffff:10.126.42.2', '::10.0.0.5', '::8.8.8.8', '::7f00:1', '::123.234.235.236', '::7bea:ebec', '1.2.3.4::',
    '1:2:3:4:5:6:1.2.3.4', '1:2:3:4:5:6:7:1.2.3.4', '1:2:3:4:5:1.2.3.4', '::1.2.3.4:5',
    '2002:a00:5::', '2002:7f00:1::', '2002:808:808::1', '2001::1', '2001:0:4136:e378:8000:63bf:3fff:fdd2',
    '2001:db8::1', '2001:0002::1', '2001:2:1::1', '64:ff9b::7f00:1', '64:ff9b::8.8.8.8', '64:ff9b:1::7f00:1',
    '2606:4700:20::681a:e06', '2a01:198:603:0:396e:4789:8e99:890f', '2a00:198:603:0:396e:4789:8e99:890f',
    '2a01:198:603:10:396e:4789:8e99:890f', '0:0:603:0:396e:4789:8e99:0001', '0:0:603:50:396e:4789:8e99:0001',
    '2A01:198:603::1', '12345::1', 'g::1', '::g', '1:2:3:4:5:6:7:8 ', ' ::1', "::1\n", "::1\0",
    '}__test|O:21:&quot;JDatabaseDriverMysqli&quot;:3:{s:2', 'é', '::é',
];
for ($n = 0; $n < 120; ++$n) {
    $ips[] = implode('.', [mt_rand(0, 260), mt_rand(0, 260), mt_rand(0, 260), mt_rand(0, 260)]);
}
for ($n = 0; $n < 120; ++$n) {
    $groups = [];
    for ($i = 0; $i < 8; ++$i) {
        $groups[] = mt_rand(0, 3) ? dechex(mt_rand(0, 0xFFFF)) : '0';
    }
    $text = implode(':', $groups);
    if (mt_rand(0, 1)) {
        $text = preg_replace('/(^|:)0(:0)+(:|$)/', '::', $text, 1);
    }
    $ips[] = $text;
}

$builtins = [];
foreach ($ips as $s) {
    $builtins[] = [
        $b($s),
        false !== filter_var($s, \FILTER_VALIDATE_IP, \FILTER_FLAG_IPV4),
        false !== filter_var($s, \FILTER_VALIDATE_IP, \FILTER_FLAG_IPV6),
        false !== filter_var($s, \FILTER_VALIDATE_IP),
        answer(static function () use ($s) {
            $packed = @inet_pton($s);

            return false === $packed ? false : bin2hex($packed);
        }),
        answer(static fn () => ip2long($s)),
    ];
}

$packedCorpus = ['', '00', '00000000', '7f000001', 'ffffffff', '0102030405', str_repeat('00', 15), str_repeat('00', 17)];
foreach (['::', '::1', '::ffff:1.2.3.4', '::1.2.3.4', '::102:304', '::ffff:0:1', '1::', '1:0:0:0:0:0:0:1', '1:0:0:1:0:0:0:1',
    '1:0:1:0:1:0:1:0', '0:1:0:0:0:1:0:0', '0:0:1::', '::1:0:0:0:0', 'ffff::', '0:0:0:0:0:ffff:0:0', '0:0:0:0:1:ffff:1:1',
    '::ffff:0.0.0.0', '::0.0.0.1', '::0.1.0.0', '64:ff9b::7f00:1', '2a01:198:603::'] as $text) {
    $packedCorpus[] = bin2hex(inet_pton($text));
}
for ($n = 0; $n < 300; ++$n) {
    $bytes = '';
    for ($i = 0; $i < 8; ++$i) {
        $bytes .= mt_rand(0, 2) ? "\0\0" : pack('n', mt_rand(0, 2) ? mt_rand(0, 0xFFFF) : 0xFFFF);
    }
    $packedCorpus[] = bin2hex($bytes);
    $packedCorpus[] = bin2hex(substr($bytes, 0, 4));
}
$ntop = [];
foreach ($packedCorpus as $hex) {
    $ntop[] = [$hex, @inet_ntop(hex2bin($hex))];
}

// Subnets: addresses with and without netmasks, odd netmasks included.
$netmasks = ['0', '00', '-0', '+0', '0.0', '0.5', '.5', '1', '1.', '1x', '8', '8.5', '8x', ' 8', '8 ', "\t8", "8\n", ' 8x',
    '+8', '1e1', '0x8', '010', '16', '24', '31', '31.9', '32', '32.0', '32.9', '33', '-1', '', 'x', '8/8',
    '10x', '100abc', '64', '64.5', '65', '96', '104', '120', '127', '128', '128.0', '129', '1e2', ' 64', '64 ', ' 64x', '2abc', '30x', '3x'];
$v4Bases = ['0.0.0.0', '1.2.3.4', '10.0.0.0', '127.0.0.1', '192.168.1.0', '192.168.0.0', '256.256.256', '1.2.3', '010.0.0.0', 'x', '', '::1', '*'];
$v6Bases = ['::', '::1', '::ffff:10.0.0.0', '::ffff:0:0', '2a01:198:603:0::', '1a01:198:603:0::', 'fe80::', 'fc00::', '2002::', '2001::', 'x', '', '10.0.0.0', '[::1]'];
$subnets = [];
foreach ($v4Bases as $base) {
    $subnets[] = $base;
    foreach ($netmasks as $mask) {
        $subnets[] = $base . '/' . $mask;
    }
}
foreach ($v6Bases as $base) {
    $subnets[] = $base;
    foreach ($netmasks as $mask) {
        $subnets[] = $base . '/' . $mask;
    }
}
$subnets = array_values(array_unique($subnets));

$requests4 = ['1.2.3.4', '10.0.0.1', '127.0.0.1', '192.168.1.1', '192.168.52.1', '0.0.0.0', '255.255.255.255', '1.2.3', '::1', 'x', '', '1.2.3.4 ', '::ffff:10.0.0.5'];
$requests6 = ['::1', '::', '::ffff:10.0.0.5', '::ffff:10.126.42.2', '2a01:198:603:0:396e:4789:8e99:890f', '2a00:198:603:0:396e:4789:8e99:890f', 'fe80::1', 'fc00::1', '2002:a00:5::', '1.2.3.4', 'x', '', '::1%lo', '[::1]'];

$check4 = [];
foreach ($requests4 as $r) {
    foreach ($subnets as $s) {
        $check4[] = [$b($r), $b($s), answer(static fn () => IpUtils::checkIp4($r, $s))];
    }
}
$check6 = [];
foreach ($requests6 as $r) {
    foreach ($subnets as $s) {
        $check6[] = [$b($r), $b($s), answer(static fn () => IpUtils::checkIp6($r, $s))];
    }
}

// Releases before v8.1.11 have no PRIVATE_RANGES: its v8.1.11 value is
// used there, so a difference comes from checkIp() itself.
$privateRanges = \defined(IpUtils::class . '::PRIVATE_RANGES') ? \constant(IpUtils::class . '::PRIVATE_RANGES')
    : ['127.0.0.0/8', '10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16', '169.254.0.0/16', '100.64.0.0/10', '::1/128', 'fc00::/7', 'fe80::/10'];
$lists = ['PRIVATE_RANGES' => $privateRanges,'PRIVATE_SUBNETS' => IpUtils::PRIVATE_SUBNETS, 'v4' => ['10.0.0.0/8'], 'mixed' => ['::1', '192.168.0.0/16', 'x', '::ffff:0:0/96']];
$check = [];
foreach ($ips as $r) {
    foreach ($lists as $name => $list) {
        $check[] = [$b($r), $name, answer(static fn () => IpUtils::checkIp($r, $list))];
    }
}

$private = [];
foreach ($ips as $r) {
    $private[] = [$b($r), answer(static fn () => IpUtils::isPrivateIp($r))];
}

if (isset($argv[1])) {
    echo json_encode(['check' => $check, 'private' => $private], \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
    exit;
}

$anonymize = [];
foreach ($ips as $ip) {
    foreach ([[1, 8], [0, 0], [2, 3], [4, 16], [3, 12], [1, 15]] as [$v4, $v6]) {
        $anonymize[] = [$b($ip), $v4, $v6, answer(static fn () => IpUtils::anonymize($ip, $v4, $v6))];
    }
}

echo json_encode([
    'lists' => $lists,
    'builtins' => $builtins,
    'ntop' => $ntop,
    'check4' => $check4,
    'check6' => $check6,
    'check' => $check,
    'private' => $private,
    'anonymize' => $anonymize,
], \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
