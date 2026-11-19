# M1-D: request implementation provenance gate

The gate in PLAN.md ("Request implementation provenance gate"): rewrite
`multipart.rs`, `query.rs` and `cookies.rs` of `symfony-http-foundation`
from PHP documentation and recorded observations, with no php-src-derived
implementation (owner decision 8), keeping the existing vectors and the
demo's comparisons passing.

## Method

- **Not read:** the bodies of the three earlier modules, and php-src. Only
  their public signatures and type definitions, their tests, and the
  recorded vectors were read. `upstream/php-src`, still listed in
  `upstream.tsv`, was not opened.
- **Black-box use of the earlier API:** a throwaway test called the old
  public functions to settle two contract questions PHP cannot answer
  (`Query::is_utf8` after overwrites, what `Cookies` returned for `a[b]`).
  Only outputs were observed; the test was deleted.
- **Documentation:** the PHP manual's "Variables From External Sources"
  (dots and spaces become `_`, bracket arrays, trailing text ignored,
  cookie names no longer URL-decoded), "Arrays" (integer key strings) and
  the ini defaults (`max_input_vars`, `max_input_nesting_level`,
  `max_file_uploads`, `arg_separator.input`).
- **Observation:** PHP 8.4.27 in the oracle image, through its built-in
  server, writing requests byte for byte over a socket:
  - `oracle/recorders/request_vars.{py,php}` and `request_vars_server.php`,
    with the cases in `request_vars_cases.py`: 160 query strings (each as
    a query string and as a urlencoded body), 54 Cookie headers, and 12
    form content types (`scripts/record.sh request-vars`);
  - 59 new multipart bodies added to `multipart_cases.py`
    (`rewrite_cases()`), each sent as POST and PUT (118 vectors), after
    the 156 vectors moved from symfony-demo-rs. Re-recording left those
    156 unchanged.
  Every rule in the new modules' documentation is tied to recorded cases;
  where a probe showed a rule was not what was assumed, a case was added
  and recorded before the code was written.
- **Author's note:** the model that wrote this code may have seen php-src
  in its training data. The code was not translated from it: the
  structure (a whole-body state machine with no read buffer, a shared
  name registrar) is new, and each behaviour is pinned by a recorded case.

## Result

| Module | New source | Evidence |
|---|---|---|
| `query.rs` | Query strings, urlencoded bodies (`Query::parse_body`, new), name registration shared with cookies and multipart | `tests/request_vars.rs`: 160 recorded cases as GET and POST; module tests |
| `cookies.rs` | Cookie headers on the shared registrar | `tests/request_vars.rs`: 54 recorded headers; module test |
| `multipart.rs` | Whole-body state machine (seek, headers, value), parameter reader | Module test over all 274 recorded bodies; module tests |
| `lib.rs` (`form_fields`, `BodyFormat`) | Content type trimmed as the HTTP layer does; urlencoded bodies read as bytes | `tests/request_vars.rs`: 12 recorded content types |

A deliberately wrong limit (1,000 pairs for a body) made 6 of 160 query
vectors fail, so the vector tests can fail.

PHP's 5 KiB read buffer, which the earlier code modelled, showed no
effect on any recorded body (the `pad-*`, `long-*` and
`cr-before-boundary-at-fill` cases); the new parser has no buffer.

## Behaviour changes (all toward PHP's recorded answers)

See the crate README, "Differences from the earlier implementation":
`+` in cookie values, array cookies, the urlencoded body's 1,001-pair
limit and byte handling, and the trimmed content type. One existing test
expectation changed with them: `cookies::tests::parses_like_php` asserted
that `x y=+` gives a space, and PHP gives `+` (recorded `cookie-12`).

## Checks

symfony-rs: `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings` clean; `cargo test` 805 passed, 0 failed (22 in
symfony-http-foundation); `scripts/sources.sh --check` and
`scripts/parity-check.sh` clean. The three php-src rows left
`parity/sources.tsv`.

symfony-demo-rs (branch `plan/symfony-runtime`, with its `[patch]` pointing
at this checkout; restored afterwards, nothing committed there): `cargo fmt
--check` and clippy clean, `cargo test` 226 passed. Then the full
comparison on fresh databases, both servers using the original's published
`APP_SECRET`, once with this checkout and once as a control with the
demo's pinned revision `e467232`:

| Comparison | This checkout | Pinned revision |
|---|---|---|
| Pages (`capture.sh`, 4 identities) | 92 same, 0 differ | 92 same, 0 differ |
| Flows admin / locale | 68 / 19 same, 0 differ | 68 / 19 same, 0 differ |
| Flows security / ties | 3 / 14 same, 8 / 2 differ | 3 / 14 same, 8 / 2 differ |
| Multipart login cases | 92 same, 0 differ | 92 same, 0 differ |
| Live search | 124 same, 3 differ | 124 same, 3 differ |

Every diff file is identical between the two runs. The flow differences
are one post's slug (`mqal-jdyd` in the original, `mql-jdyd` in the port),
the demo README's accepted "Slugs of titles in some scripts"; the three
live cases are its documented case-insensitive-mount artifacts.
