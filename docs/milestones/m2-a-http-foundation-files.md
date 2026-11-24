# M2-A stage 4: File, UploadedFile and FileBag

November 24. Branch `claude/project-thread-7vx3zi`. Crates touched:
`http-foundation`, and a new `mime` (`symfony-mime`). `php-value` is not
changed.

## Scope

Upstream symfony/http-foundation at the pin (v8.1.11,
`30fa3efe6e736b0d06f5331cc6abe813582c0a26`) and symfony/mime at the
demo's lock (v8.1.10, `8d1232052c9cdef170ed9608b3147bca4f61ff69`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `File/File.php` | `crates/http-foundation/src/file.rs` | FileTest 9 of 9 |
| `File/UploadedFile.php` | `crates/http-foundation/src/uploaded_file.rs` | UploadedFileTest 23 of 23 |
| `FileBag.php` | `crates/http-foundation/src/file_bag.rs` | FileBagTest 9 of 9 |
| `File/Exception/*.php` (12 classes) | `crates/http-foundation/src/exception.rs` | through the tests above |
| mime `MimeTypes.php`, `MimeTypesInterface.php`, `MimeTypeGuesserInterface.php` | `crates/mime/src/mime_types.rs`, `map.rs` (generated) | MimeTypesTest 13 of 13 |
| mime `FileBinaryMimeTypeGuesser.php` | `crates/mime/src/file_binary_mime_type_guesser.rs` | FileBinaryMimeTypeGuesserTest 7 of 8 (1 `n/a`) |
| mime `Exception/{ExceptionInterface,LogicException,InvalidArgumentException,RuntimeException}.php` | `crates/mime/src/exception.rs` | through the tests above |

The mime manifest (`parity/symfony/mime.tsv`, 395 tests) is new; the
other 374 rows are `pending`, among them FileinfoMimeTypeGuesserTest's 8
(decision 18).

PHP built-ins (`crates/http-foundation/src/php.rs`, written from the PHP
manual and recorded answers, no PHP source, decision 8): the
`UPLOAD_ERR_*` constants, `is_uploaded_file()` / `move_uploaded_file()`
on a list the upload code fills, `intval()` in bases 16 and 8,
`pathinfo(PATHINFO_EXTENSION)`, `basename()`, the coercions into `new
UploadedFile()`'s typed parameters, and warnings as a web SAPI gives
them. File moves, `mkdir()`, `chmod()`, `umask()` and
`file_get_contents()` come from `symfony-filesystem`'s `php` module;
`is_file()`, `realpath()` and `stat()` from `symfony-finder`'s `fs`.

## Behaviour found by recording

- `parseFilesize()`: `intval('0o17', 8)` is 0 (no `0o` prefix);
  `'  0x10'` is 0 (the `0x` test sees the spaces, and `(int)` stops
  there), but `"\v1M"` and `"\f1M"` are 1 MiB (`(int)` skips them);
  `'1M '` is 1 (the unit must be last); a product past `PHP_INT_MAX`
  becomes a float.
- `getMaxFilesize()`: a negative ini value wins `min()` (it is truthy),
  so `post_max_size=-1` gives -1 and the message says "limit is 0 KiB";
  with both values 0 it is `PHP_INT_MAX`, and the message's `%d` of
  `PHP_INT_MAX / 1024` (a float) is 9007199254740992.
- FileBag: a string `error` that is numeric with a trailing space is
  read (`'1 '` is 1), `'1.5'` and `1.5` are 1, `'0x1'`, `'abc'`, `[]` and
  `1e30` are TypeErrors; `4.0` and `'4'` are not `UPLOAD_ERR_NO_FILE`
  (`===`), so they become UploadedFiles with error 4; a null
  `full_path` falls back to `name`, an empty one does not; a missing
  sibling key (`type` => `[]`) reads as null.
- `move_uploaded_file()` under a web SAPI (`php -S`): a file not
  uploaded gives `false` with no warning; a failed move warns
  `move_uploaded_file(): Unable to move "<from>" to "<to>"` with `&`, `<`,
  `>` and `"` escaped (`html_errors`, on outside the CLI); `rename()`'s
  warning is escaped the same way. So `strip_tags()` finds no tag in
  these messages, and the port does not need it.

## Evidence

| What | Cases | Result |
|---|---|---|
| `parseFilesize()` (`crates/http-foundation/tests/data/files.json`, `oracle/recorders/http_foundation_files.php`) | 108 texts: units, signs, `0x` and `0` prefixes, whitespace, overflow | all match |
| `getMaxFilesize()` and the INI_SIZE message | 126 `post_max_size` / `upload_max_filesize` pairs, each in its own `php -d` process | all match |
| Client names, extensions and paths | 55 names (`..`, `\`, trailing `/`, NUL, non-UTF-8, Cyrillic) | all match |
| Client MIME types, `guessClientExtension()` | 33 types (null, `""`, `"0"`, case, Unicode case) | all match |
| Error codes | 13 codes in and out of test mode: `getError()`, `getErrorMessage()`, `isValid()`, `move()`'s exception | all match |
| FileBag | 71 `$_FILES`-like arrays: coercions, TypeErrors, the array-named layout, list filtering, `full_path` | all match |

Each replay was checked by breaking a rule on purpose: making every int
truthy in FileBag's list filter, and reading a null sibling field as a
refusal, each made the FileBag replay fail.

Not replayed: content guessing. On the 472 files of the http-foundation
and mime checkouts, the oracle's `finfo` and its `file` 5.44 differ on 13
(see decision 18); the cloud container's `file` 5.45 gives the answers
the upstream tests expect for every fixture.

## Advisories

No advisory in the upstream watch lists `File/File.php`,
`File/UploadedFile.php`, `FileBag.php` or symfony/mime's MIME type files.
(`scripts/upstream_watch.py` ran in this cloud session, but every
advisory query got HTTP 403 there, so this rests on the last watch run.)

## Environment of this stage

Done in a cloud session. The oracle image was built without `apt-get`
(its mirror is blocked there), so without `intl` and `pdo_sqlite`, which
these recorders do not load; `/demo/vendor` was an autoload shim over the
pinned upstream checkouts, with symfony/mime copied in. As root, the
Config test `testLoadFile` fails (root reads a `0o000` file); it does not
touch this stage's code. `scripts/linux-test.sh` (for Windows hosts) was
not needed: the checks ran on Linux.

## Demo comparison

Not run: the owner paused the symfony-demo-rs step (November 23).

## Not done in this stage

`FileinfoMimeTypeGuesser` (decision 18), `File/Stream.php` (used by
BinaryFileResponse), and writing multipart file parts to temporary files
so `$_FILES` can be filled (the multipart parser still drops them; that
belongs with Request). Next: Request.
