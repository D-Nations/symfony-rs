<?php
// FileinfoMimeTypeGuesser answers (PHP's finfo, its bundled libmagic) for
// crates/mime/tests/data/fileinfo.json (scripts/record.sh mime-fileinfo):
//
// - samples: the files in crates/mime/tests/fixtures/fileinfo (test files
//   of the `file` project at FILE5_45, and small files of common types);
// - upstream: every file under upstream/symfony (the pinned checkouts),
//   without .git, keyed by its path from the repository root.
//
// A guess that throws is written as {"throws": class}.

error_reporting(E_ALL);
set_error_handler(static fn () => true);

require '/demo/vendor/autoload.php';

use Symfony\Component\Mime\FileinfoMimeTypeGuesser;

$guesser = new FileinfoMimeTypeGuesser();

function guess(FileinfoMimeTypeGuesser $guesser, string $path): mixed
{
    try {
        return $guesser->guessMimeType($path);
    } catch (Throwable $e) {
        return ['throws' => $e::class];
    }
}

function files(string $dir): array
{
    $it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($dir, FilesystemIterator::SKIP_DOTS));
    $out = [];
    foreach ($it as $file) {
        $path = $file->getPathname();
        if (str_contains($path, '/.git/') || !is_file($path) || is_link($path)) {
            continue;
        }
        $out[] = $path;
    }
    sort($out, SORT_STRING);

    return $out;
}

$samples = [];
$dir = '/work/crates/mime/tests/fixtures/fileinfo';
foreach (files($dir) as $path) {
    $samples[substr($path, strlen($dir) + 1)] = guess($guesser, $path);
}

$upstream = [];
foreach (files('/work/upstream/symfony') as $path) {
    $upstream[substr($path, strlen('/work/'))] = guess($guesser, $path);
}

echo json_encode([
    'php' => PHP_VERSION,
    'samples' => $samples,
    'upstream' => $upstream,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_INVALID_UTF8_SUBSTITUTE | JSON_THROW_ON_ERROR)."\n";
