<?php
// UploadedFile and FileBag answers for what UploadedFileTest, FileTest and
// FileBagTest leave open, for crates/http-foundation/tests/data/files.json
// (scripts/record.sh http-foundation-files):
//
// - parse_filesize: UploadedFile::parseFilesize() (private) on ini texts;
// - max_filesize: getMaxFilesize() and the UPLOAD_ERR_INI_SIZE message
//   under post_max_size / upload_max_filesize pairs, each in its own PHP
//   process (`php -d`), since both are PHP_INI_PERDIR;
// - names: getClientOriginalName(), getClientOriginalExtension() and
//   getClientOriginalPath() of client file names;
// - mime: getClientMimeType() and guessClientExtension() of client types;
// - errors: getError(), getErrorMessage(), isValid() and move()'s
//   exception for each error code, in and out of test mode;
// - bags: FileBag conversions of $_FILES-like arrays, an UploadedFile
//   written as ['@uploaded', pathname, name, path, type, error].
//
// Paths under the fixtures directory are written with the @FIX@ prefix.
// Values are typed (lib/typed.php).

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\HttpFoundation\File\UploadedFile;
use Symfony\Component\HttpFoundation\FileBag;

const FIX = '/work/crates/http-foundation/tests/fixtures/file';

if (($argv[1] ?? '') === 'max') {
    // child process: one ini pair
    $file = new UploadedFile('/nonexistent', 'a.gif', null, \UPLOAD_ERR_INI_SIZE);
    echo json_encode([typed_call(static fn () => UploadedFile::getMaxFilesize()), typed_call(static fn () => $file->getErrorMessage())], JSON_THROW_ON_ERROR);
    exit;
}

function unfix(mixed $v): mixed
{
    if (\is_string($v) && str_starts_with($v, FIX)) {
        return '@FIX@'.substr($v, \strlen(FIX));
    }
    if ($v instanceof UploadedFile) {
        return ['@uploaded', unfix($v->getPathname()), $v->getClientOriginalName(), $v->getClientOriginalPath(), $v->getClientMimeType(), $v->getError()];
    }
    if (\is_array($v)) {
        return array_map(unfix(...), $v);
    }

    return $v;
}

function fix(mixed $v): mixed
{
    if (\is_string($v) && str_starts_with($v, '@FIX@')) {
        return FIX.substr($v, 5);
    }
    if (\is_array($v)) {
        return array_map(fix(...), $v);
    }

    return $v;
}

function call(callable $f): array
{
    try {
        return typed_value(unfix($f()));
    } catch (\Throwable $e) {
        return ['throws', $e::class, unfix($e->getMessage())];
    }
}

$out = [];

// parseFilesize
$parse = (static fn (string $s) => UploadedFile::parseFilesize($s))->bindTo(null, UploadedFile::class);
$sizes = ['', '0', '1', '-1', '8M', '8m', '2M', '2G', '1T', '1t', '1K', '1k', '512', '512B', 'M', 'K', 'G', '+1M', '++1M', '+', '-', '-1M', '-1G',
    '0x10', '0X10', '0x10M', '0x1fk', '0xg', '0x', '0x-1', '0x+1', '0x 1', '010', '010M', '08', '09k', '0o17', '0O17', '0b11', '00', '0', '0M', '0k',
    ' 1M', "\t1M", '1M ', '1 M', '1.5M', '1.5', '1e3', '1e3k', '1E3', '.5M', '5.M', 'abc', 'M1', '1MB', '1Mk', '1kM', '9223372036854775807',
    '9223372036854775807k', '9223372036854775808', '9007199254740993k', '9223372036854775807t', '8796093022208t', '8796093022207t', '8796093022208g',
    '0x7fffffffffffffff', '0x8000000000000000', '0xffffffffffffffffffff', '0777777777777777777777', '07777777777777777777777777',
    '1000000000000000000000', '1e100', '-1e100', '1e400', 'inf', 'nan', '0x7fffffffffffffffk', '-9223372036854775808', '-9223372036854775809',
    '  0x10', '+0x10', '0x10 ', '0xAbC', '0XABCM', '1g ', 'g', '0g', '00x10', '0 x10', "1\n", "1M\n", '1m', '1G', '64k', '128M', '-0', '+0', '1_000',
    '1,5M', '٣M', "\v1M", "\f1M", "\x001M", '1M'.\chr(0), '2147483648', '4294967296k'];
foreach ($sizes as $s) {
    $out['parse_filesize'][] = [typed_value($s), call(static fn () => $parse($s))];
}

// getMaxFilesize() per ini pair
$ini = ['8M', '2M', '0', '', '-1', '1G', '1T', '0x10M', '010k', '9223372036854775807', '9223372036854775807k', '8796093022208t', '1.5M', 'abc', '100', '1023', '1025', '3k'];
$pairs = [];
foreach (['8M', '0', '-1', '1T', '100', ''] as $post) {
    foreach ($ini as $upload) {
        $pairs[] = [$post, $upload];
    }
}
foreach ($ini as $post) {
    $pairs[] = [$post, '0'];
}
foreach ($pairs as [$post, $upload]) {
    $cmd = escapeshellarg(PHP_BINARY).' -d display_errors=0 -d post_max_size='.escapeshellarg($post).' -d upload_max_filesize='.escapeshellarg($upload).' '.escapeshellarg(__FILE__).' max';
    $answer = json_decode(shell_exec($cmd), true, 512, JSON_THROW_ON_ERROR);
    $out['max_filesize'][] = [typed_value($post), typed_value($upload), $answer[0], $answer[1]];
}

// client names
$names = ['original.gif', '../../original.gif', '..\\..\\original.gif', 'a/b/c.txt', 'a\\b\\c.txt', 'webkitdirectory/test.txt', 'webkitdirectory\\nested\\test.txt',
    '', '.', '..', '/', '\\', 'a/', 'a\\', '.htaccess', 'a.', 'a..b', 'a.b.c', 'archive.tar.gz', 'no_extension', 'UPPER.GIF', 'файлfile.gif', '../../файлfile.gif',
    'файл.ГИФ', "a\0b.gif", "a.g\0if", 'a b.gif', ' a.gif ', 'a.gif ', '.gif', '..gif', 'a/.gif', 'C:\\fakepath\\photo.jpg', 'C:photo.jpg', '//server/share/x.png',
    'a//b.png', 'a/b/', 'a/b/.', 'a.<b>', '<i>x</i>.gif', 'a%2Fb.gif', 'a;b.gif', "a\nb.gif", "a\rb.txt", 'a.b/c', 'a.b\\c', '~/x.gif', 'x.GiF', 'x.jpeg', 'x.123',
    "\xff\xfe.gif", "x.\xff", 'a'.str_repeat('b', 300).'.gif', 'dir.d/file', 'dir.d\\file'];
foreach ($names as $name) {
    $file = new UploadedFile('/nonexistent', $name, null, \UPLOAD_ERR_NO_FILE);
    $out['names'][] = [typed_value($name), typed_value($file->getClientOriginalName()), typed_value($file->getClientOriginalExtension()), typed_value($file->getClientOriginalPath())];
}

// client MIME types
$types = [null, '', '0', '00', ' ', 'image/gif', 'IMAGE/GIF', 'Image/Gif', 'image/png', 'image/svg', 'image/svg+xml', 'application/vnd.ms-excel.sheet.macroEnabled.12',
    'application/vnd.ms-excel.sheet.macroenabled.12', 'APPLICATION/VND.MS-EXCEL.SHEET.MACROENABLED.12', 'text/plain', 'text/plain; charset=utf-8', ' image/gif', 'image/gif ',
    'x/unknown', 'application/octet-stream', 'application/postscript', 'application/x-empty', 'inode/x-empty', 'text/csv', 'application/csv', 'video/x-anim',
    "image/gif\0", 'ımage/gif', 'İMAGE/GIF', 'application/x-sharedlib', 'application/x-troff-man', 'audio/mpeg', 'AUDIO/MPEG'];
foreach ($types as $type) {
    $file = new UploadedFile('/nonexistent', 'a', $type, \UPLOAD_ERR_NO_FILE);
    $out['mime'][] = [typed_value($type), typed_value($file->getClientMimeType()), call(static fn () => $file->guessClientExtension())];
}

// error codes
foreach ([null, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, -1, 100] as $error) {
    foreach ([false, true] as $test) {
        $path = FIX.'/test.gif';
        $file = new UploadedFile($path, '../x/<b>original</b>.gif', 'image/gif', $error, $test);
        $row = [typed_value($error), typed_value($test), typed_value($file->getError()), typed_value($file->getErrorMessage()), typed_value($file->isValid())];
        // a valid test-mode file would really move: not called
        $row[] = $file->isValid() ? ['skipped'] : call(static fn () => $file->move(FIX.'/directory'));
        $out['errors'][] = $row;
    }
}

// FileBag conversions
$gif = '@FIX@/test.gif';
$txt = '@FIX@/webkitdirectory/test.txt';
$one = static fn ($error = 0, $tmp = null, $extra = []) => ['name' => 'a.gif', 'type' => 'image/gif', 'tmp_name' => $tmp ?? $gif, 'error' => $error, 'size' => 35] + $extra;
$bags = [
    [],
    ['file' => $one()],
    ['file' => $one(4)],
    ['file' => $one(4, '')],
    ['file' => $one(1)],
    ['file' => $one('0')],
    ['file' => $one('4')],
    ['file' => $one(4.0)],
    ['file' => $one(null)],
    ['file' => $one(true)],
    ['file' => $one(false)],
    ['file' => $one(0, '@FIX@/not_here')],
    ['file' => $one(0, '@FIX@/directory')],
    ['file' => $one(2, '@FIX@/not_here')],
    ['file' => $one(0, null, ['full_path' => 'dir/a.gif'])],
    ['file' => $one(0, null, ['full_path' => null])],
    ['file' => $one(0, null, ['full_path' => ''])],
    ['file' => $one(0, null, ['extra' => 1])],
    ['file' => ['name' => 'a', 'type' => 'b', 'tmp_name' => $gif, 'error' => 0]],
    ['file' => ['name' => 'a', 'type' => null, 'tmp_name' => $gif, 'error' => 0, 'size' => null]],
    ['file' => ['name' => 'a', 'type' => '', 'tmp_name' => $gif, 'error' => 0, 'size' => 'x']],
    ['file' => ['name' => 'a', 'type' => '0', 'tmp_name' => $gif, 'error' => 0, 'size' => 0]],
    ['file' => ['name' => 7, 'type' => 8, 'tmp_name' => $gif, 'error' => 0, 'size' => 0]],
    ['file' => ['name' => true, 'type' => false, 'tmp_name' => $gif, 'error' => 0, 'size' => 0]],
    ['file' => ['name' => null, 'type' => 'x', 'tmp_name' => $gif, 'error' => 0, 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => null, 'error' => 4, 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => null, 'error' => 1, 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => 5, 'error' => 1, 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => ['x'], 'tmp_name' => $gif, 'error' => 0, 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => $gif, 'error' => 'abc', 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => $gif, 'error' => '1.5', 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => $gif, 'error' => 1.5, 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => $gif, 'error' => ' 1', 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => $gif, 'error' => '1 ', 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => $gif, 'error' => '0x1', 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => $gif, 'error' => [], 'size' => 0]],
    ['file' => ['name' => 'a', 'type' => 'x', 'tmp_name' => $gif, 'error' => 1e30, 'size' => 0]],
    ['file' => 'foo'],
    ['file' => 1],
    ['file' => null],
    ['file' => true],
    ['file' => []],
    ['file' => [[]]],
    ['file' => ['a' => []]],
    ['file' => [null, 0, '', '0', 'x', [], false, 1, [null]]],
    ['file' => ['a' => null, 'b' => 0, 'c' => '']],
    ['file' => [1 => null, 2 => 'x']],
    ['file' => [0 => null, 2 => 'x']],
    ['file' => ['name' => ['a.gif', 'b.gif'], 'type' => ['image/gif', 'image/gif'], 'tmp_name' => [$gif, $gif], 'error' => [0, 0], 'size' => [35, 35]]],
    ['file' => ['name' => ['a.gif', ''], 'type' => ['image/gif', ''], 'tmp_name' => [$gif, ''], 'error' => [0, 4], 'size' => [35, 0]]],
    ['file' => ['name' => ['', 'b.gif'], 'type' => ['', 'image/gif'], 'tmp_name' => ['', $gif], 'error' => [4, 0], 'size' => [0, 35]]],
    ['file' => ['name' => ['x' => 'a.gif', 'y' => ''], 'type' => ['x' => 'image/gif', 'y' => ''], 'tmp_name' => ['x' => $gif, 'y' => ''], 'error' => ['x' => 0, 'y' => 4], 'size' => ['x' => 35, 'y' => 0]]],
    ['file' => ['name' => ['a' => ['b' => ['c' => 'a.gif']]], 'type' => ['a' => ['b' => ['c' => 't']]], 'tmp_name' => ['a' => ['b' => ['c' => $gif]]], 'error' => ['a' => ['b' => ['c' => 0]]], 'size' => ['a' => ['b' => ['c' => 1]]]]],
    ['file' => ['name' => [['a.gif'], ['']], 'type' => [['t'], ['']], 'tmp_name' => [[$gif], ['']], 'error' => [[0], [4]], 'size' => [[1], [0]]]],
    ['file' => ['name' => ['a.gif'], 'type' => ['t'], 'tmp_name' => [$gif], 'error' => [0], 'size' => [1], 'full_path' => ['d/a.gif']]],
    ['file' => ['name' => ['a.gif', 'b.txt'], 'type' => ['t', 't'], 'tmp_name' => [$gif, $txt], 'error' => [0, 0], 'size' => [1, 1], 'full_path' => [1 => 'd/b.txt']]],
    ['file' => ['name' => ['a.gif'], 'type' => ['t'], 'tmp_name' => [$gif], 'error' => [0], 'size' => [1], 'full_path' => [null]]],
    ['file' => ['name' => ['a.gif'], 'type' => ['t'], 'tmp_name' => [$gif], 'error' => [0], 'size' => [1], 'full_path' => null]],
    ['file' => ['name' => ['a.gif'], 'type' => [], 'tmp_name' => [$gif], 'error' => [0], 'size' => [1]]],
    ['file' => ['name' => ['a.gif'], 'type' => null, 'tmp_name' => [$gif], 'error' => [0], 'size' => [1]]],
    ['file' => ['name' => ['a.gif'], 'type' => 5, 'tmp_name' => [$gif], 'error' => [4], 'size' => [1]]],
    ['file' => ['name' => ['a.gif'], 'type' => ['t'], 'tmp_name' => [$gif], 'error' => [], 'size' => [1]]],
    ['file' => ['name' => ['a.gif'], 'type' => ['t'], 'tmp_name' => [$gif], 'error' => null, 'size' => [1]]],
    ['file' => ['name' => [], 'type' => [], 'tmp_name' => [], 'error' => [], 'size' => []]],
    ['file' => ['name' => ['k' => 'a.gif'], 'type' => ['k' => 't'], 'tmp_name' => ['k' => $gif], 'error' => ['k' => 0], 'size' => ['k' => 1], 'extra' => 1]],
    ['file' => ['name' => ['error' => 'a', 'name' => 'b', 'size' => 'c', 'tmp_name' => 'd', 'type' => 'e'], 'type' => ['error' => 'a', 'name' => 'b', 'size' => 'c', 'tmp_name' => 'd', 'type' => 'e'], 'tmp_name' => ['error' => 1, 'name' => 2, 'size' => 3, 'tmp_name' => 4, 'type' => 5], 'error' => ['error' => 4, 'name' => 4, 'size' => 4, 'tmp_name' => 4, 'type' => 4], 'size' => ['error' => 0, 'name' => 0, 'size' => 0, 'tmp_name' => 0, 'type' => 0]]],
    ['file' => ['name' => 'a.gif', 'type' => 'image/gif', 'tmp_name' => [$gif], 'error' => [0], 'size' => [1]]],
    ['file' => ['x' => $one(), 'y' => $one(4), 'z' => 'keep', 'w' => [$one(4)]]],
    ['file' => [$one(), $one(4), 'keep', [$one(4)]]],
    ['a' => $one(), 'b' => $one(4), 7 => $one(), '8' => $one(4)],
    ['file' => $one(0, $txt, ['name' => 'test.txt', 'full_path' => 'webkitdirectory/test.txt'])],
];
foreach ($bags as $files) {
    $out['bags'][] = [typed_value($files), call(static function () use ($files) {
        $bag = new FileBag(fix($files));

        return $bag->all();
    })];
}

echo json_encode($out, JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR), "\n";
