# M2-A: symfony/mime FileinfoMimeTypeGuesser

November 24. Branch `claude/project-thread-7vx3zi`. Crates touched:
`mime` (`symfony-mime`), and comments and README rows in
`http-foundation`. No path handling is changed.

## Decision

PLAN.md decision 18, the owner's option (c): a pure-Rust libmagic in
place of PHP's `finfo`, in a PR of its own.

## Scope

Upstream symfony/mime at the demo's lock (v8.1.10,
`8d1232052c9cdef170ed9608b3147bca4f61ff69`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `FileinfoMimeTypeGuesser.php` | `crates/mime/src/fileinfo_mime_type_guesser.rs` | FileinfoMimeTypeGuesserTest 8 of 8 |
| `MimeTypes.php` (constructor) | `crates/mime/src/mime_types.rs` | MimeTypesTest 13 of 13, unchanged |

`MimeTypes::new()` now registers `FileinfoMimeTypeGuesser` after
`FileBinaryMimeTypeGuesser`, as upstream, so it is asked first.

## Choice of crate

Each candidate guessed the MIME type of two sets, and the answers were
compared with PHP 8.4.27's `finfo` (libmagic 5.45) in the oracle:

| Candidate | 124 samples | Upstream files |
|---|---|---|
| `pure-magic` 0.5.2 + `magic-db` 0.7.2 | 106 match (107 with the one-byte rule below) | 16,619 of 17,004 (all of `upstream/`), 12,525 of 12,843 (`upstream/symfony`) |
| `pure-magic` 0.5.2 + the `file` 5.45 Magdir sources | 81 match; 6 of 341 sources do not parse, so whole formats (zstd, PNG, GIF, xz, ...) are lost | not continued |
| `libmagic-rs` 0.12.10 | no MIME output (`!:mime` is planned) | not run |

`pure-magic` with `magic-db` was chosen. Both are used under their
BSD-2-Clause option; both are `forbid(unsafe_code)`. The versions are
pinned with `=`, because a new rule set changes answers.

## Behaviour found by recording

- libmagic gives `application/x-empty` for an empty file and
  `application/octet-stream` for a file of one byte ("very short file (no
  magic)"); `pure-magic` gives `application/octet-stream` and
  `text/plain`. The port gives libmagic's answers.
- `pure-magic`'s `*_file()` functions also read the file's extension to
  choose rules first. libmagic reads only the bytes, so the port passes no
  extension.

## Evidence

`oracle/recorders/mime_fileinfo.php` (`scripts/record.sh mime-fileinfo`)
runs upstream's own `FileinfoMimeTypeGuesser` on:

- the 124 files of `crates/mime/tests/fixtures/fileinfo`: 71 test files of
  the `file` project at FILE5_45 (BSD licence,
  `crates/mime/tests/fixtures/fileinfo-COPYING`) and 53 small files of
  common types written for this crate;
- the 12,843 files of `upstream/symfony` (the pinned checkouts).

`crates/mime/tests/fileinfo_recorded.rs` compares the port with each
answer. 107 samples and 12,525 upstream files match. The 335 others are
listed in `crates/mime/tests/data/fileinfo_differences.tsv`, and the test
fails when a file differs that is not listed, or a listed file no longer
gives the listed answer. The largest groups:

| PHP finfo | Port | Files |
|---|---|---:|
| `text/x-c++` | `text/plain` | 120 |
| `application/javascript` | `text/javascript` | 99 |
| `application/json` | `text/plain` | 55 |
| `application/javascript` | `text/plain` | 10 |
| `text/plain` | `application/x-pem-file` | 9 |
| `application/pgp-keys` | `application/octet-stream` | 8 |
| `text/plain` | `text/x-ssh-private-key` | 7 |

Also: UTF-16 and Latin-1 text read as binary (2), RAR, WASM, HWP, AMR
(`audio/AMR`) and Gentoo GPKG named differently. The FileinfoMimeTypeGuesserTest
fixtures (GIF, the `.unknownextension` file, the DOCX) all give
upstream's expected answers.

## Not ported

The `$magicFile` constructor parameter (a compiled `.mgc` file, which
`pure-magic` does not read) and paths through a stream wrapper (both
`Class::NotPorted`).

## Advisories

No advisory in `parity/advisories.tsv` or the upstream watch lists
`FileinfoMimeTypeGuesser.php`. (`scripts/upstream_watch.py` gets HTTP
403 from the advisory API in the cloud session, so this rests on the last
watch run.)

## Checks

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`scripts/sources.sh --check` and `scripts/parity-check.sh` pass. `cargo
test --workspace --no-fail-fast`: 2,527 passed, 1 failed: Config's
`testLoadFile`, which fails only as root (root reads a `0o000` file), as
in stage 4. The replay takes about 20 seconds in a debug build.

## Environment of this stage

Done in a cloud session, with the oracle image and `/demo/vendor` shim of
stage 4 (symfony/mime copied in). The symfony-demo-rs comparison was not
run: the owner paused it (November 23).
