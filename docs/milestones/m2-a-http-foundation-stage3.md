# M2-A stage 3: AcceptHeader and parseQuery

November 23. Branch `m2/http-foundation-accept`.

## Scope

Upstream symfony/http-foundation at the pin (v8.1.11,
`30fa3efe6e736b0d06f5331cc6abe813582c0a26`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `AcceptHeader.php`, `AcceptHeaderItem.php` | `crates/http-foundation/src/accept_header.rs` | AcceptHeaderTest 6 of 6, AcceptHeaderItemTest 5 of 5 |
| `HeaderUtils.php` (`parseQuery()`) | `crates/http-foundation/src/header_utils.rs` | HeaderUtilsTest 13 of 13 (the four `parseQuery` tests added) |

PHP built-ins added to `php-value` (written from the PHP manual and
recorded answers, no PHP source, decision 8):

- `vars::{register, parse_str, top_level_key}`: PHP's input-variable rules,
  moved unchanged from http-foundation's `query.rs` (written at the M1-D
  request rewrite) so `parse_str()` shares them. `query.rs` keeps its API
  and builds on them; its 160 query, 54 cookie and 274 multipart vectors
  pass unchanged. `Query::vars()` now also gives PHP's binary-safe array.
- `url::{http_build_query, bin2hex, hex2bin}`.
- `PhpArray::next_index()`.

## Behaviour found by recording

- `parse_str()` stops at the first raw NUL byte of its input:
  `x=1&a\0b=1&y=2` gives `x` and `a` (empty), and no `y`. An encoded `%00`
  is kept. (`$_GET`'s reading, `query.rs`, is unchanged.)
- `parse_str()` reads at most `max_input_vars` (1,000) non-empty pairs.
- `parseQuery()` really reaches `hex2bin()` with odd-length text (a name
  whose `[` has no `]`, after `parse_str()` turns it into `_`): PHP warns
  and uses the key `0` for `false`; the port does the same, and breaking
  that rule makes 6 of the 3,000 recorded queries differ.
- `AcceptHeaderItem::fromString('')` (and `has('')`) is PHP's `TypeError`
  for a null value; `get('')` is not, because `get()` appends `;q=1`, which
  makes `q` the value.
- A bare attribute (`flowed`) is the string `"1"`: `combine()` gives `true`
  and `setAttribute()` takes a string.

## Evidence

| What | Cases | Result |
|---|---|---|
| `parse_str()` (`crates/php-value/tests/data/parse_str.json`, `oracle/recorders/php_value_parse_str.php`) | 254 inputs: every request-vector query string, names, keys, nesting 63 to 100, 999 to 1,500 pairs, raw NULs | all match |
| `parseQuery()` (`header_utils.json`, `query` section) | 3,000 generated queries, both modes, `&` and `;` | all match |
| AcceptHeader (`accept_header.json`) | 1,500 headers through `fromString()`, `__toString()`, `all()`, with 6,000 `has()`/`get()` queries; 2,000 `get()` calls among overlapping ranges; 21 `filter()` calls; 500 `AcceptHeaderItem::fromString()` | all match |

Each replay was checked by breaking a rule on purpose: removing the NUL
cut (1 `parse_str()` case differs), the `hex2bin()` false key (6
`parseQuery()` cases), the specificity's parameter count (38 overlapping
`get()` calls; the upstream tests and the generated headers alone did not
catch it, so the overlapping-range section was added).

`filter()` runs its pattern on php-pcre (an in-workspace crate; its
`fancy-regex` is already in the lock for Finder and String): a pattern PHP
rejects matches nothing, as `preg_match()`'s `false` does; a pattern
php-pcre does not translate is refused (`Class::NotPorted`).

`accept_language.rs` (`Request::getLanguages()`, `getPreferredLanguage()`)
now reads the header through `AcceptHeader` instead of its own copy of the
parsing. One rule changes with it, to upstream's: items are keyed by value
and attributes other than `q`, so `fr;q=0.5, fr;level=1` are two items.

## Advisories

No advisory in the upstream watch lists `AcceptHeader.php`,
`AcceptHeaderItem.php` or `HeaderUtils::parseQuery()`.

## Not done in this stage

`FileBag.php`, `File/File.php`, `File/UploadedFile.php` and the File
exceptions (FileBagTest, FileTest, UploadedFileTest): `File`'s MIME
guessing (`getMimeType()`, `guessExtension()`) uses symfony/mime, and
`UploadedFile::move()` / `isValid()` need PHP's upload state
(`is_uploaded_file()`, `move_uploaded_file()`). Next stage, then Request.

## Demo comparison

symfony-demo-rs built against this worktree through a temporary
`[patch]` (`cargo tree`: `symfony-http-foundation` from
`../symfony-rs-wt-k`), reverted afterwards, nothing committed there. Its
checks: `cargo fmt --check` ok, clippy clean, 226 tests passed.

No new control run: `run-control-i` stays valid (demo `0ebf52a`, pinned
symfony-rs `e467232`, `compare-original` image `ee906322e48c`).

| Comparison | This branch | Control (`run-control-i`) |
|---|---|---|
| Pages | 92 same, 0 differ | 92, 0 |
| Flows: admin | 68, 0 | 68, 0 |
| Flows: locale | 19, 0 | 19, 0 |
| Flows: security | 2, 9 | 3, 8 |
| Flows: ties | 16, 0 | 16, 0 |
| Multipart | 92, 0 | 92, 0 |
| Live search | 124, 3 | 124, 3 |

Every diff file is identical to the control's except security, where
`array_indexes/11-POST` differs only in the `REMEMBERME=deleted` cookie's
`expires` second (original `13:35:50`, port `13:35:51`): the two servers
answered a second apart. Re-running the security flows twice against this
branch gave 11 same, 0 differ, with both `expires` in the same second.
(In that isolated run the control's 8 known security differences do not
appear either: they depend on the state the earlier flows leave.)

This is the last symfony-demo-rs comparison before the owner paused that
step (November 23, PR #26); later stages do not run it until it resumes.
