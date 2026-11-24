# M2-A routing stage 2: RequestContext and the routing exceptions

November 24. Branch `m2/routing-request-context`. Crates touched:
`symfony-routing`, `php-value`.

The matcher and the generator both read RequestContext and throw the
routing exceptions. A matcher session started at the same time as the
generator stage, so the owner chose to port these shared pieces first, as
one small PR. Both stages then build on it.

## Scope

Upstream symfony/routing at the pin (v8.1.11,
`f3fefc6cad3748d538be970ca81b71f06d7aa8b3`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `RequestContext.php` (without `fromRequest()`) | `crates/routing/src/request_context.rs` | RequestContextTest 16 of 18: `testConstructParametersBcLayer` `n/a` (a PHP subclass calling `setParameters()` before the parent constructor), `testFromRequest` pending (needs HttpFoundation's Request) |
| `Exception/*.php` (all 11) | `crates/routing/src/exception.rs` | none upstream |

`php-value` gains `url::parse_url()` (`Url`, `Url::to_array()`), written
from the PHP manual and recorded answers (no PHP source, decision 8).
symfony-filesystem's scheme-only `parse_url_scheme()` (in its `php.rs`,
reworked by PR #33 while this stage ran) is left unchanged here. Having it
call `php_value::url::parse_url()` is a follow-up for that crate.

Prior art for `parse_url()` (PLAN.md stack row): `url` follows the WHATWG
standard, and `http::Uri` and `fluent-uri` follow RFC 3986 strictly.
None returns PHP's components for the malformed input `fromUri()` must
read. No new dependency.

## Behaviour found by recording

`parse_url()` (all fixed by the corpus, with one rule tried per round
until the model matched every case):

- `a:80`, `a:80/x` and `x/y:1` (a colon followed by 1 to 5 digits and
  then the end or `/`, before any `?` or `#`) are host and port without
  `//`. For `x/y:1` the host is `x` and the path is `/y:1`. With 6 or
  more digits it is a scheme and path again.
- `''` is `['path' => '']`, while `?q` has no path. `:`, `//`, `///a`,
  `http://`, `//:80`, `//@` and `/x:1` are `false`.
- `file:///etc` keeps `/etc` as the path, but `http:///x` is `false`.
- The port is the text after the authority's last colon unless the host
  is bracketed and ends with `]`, so `[::1` has host `[:` and port 1. It
  is at most 5 bytes, read like `strtol()` (leading spaces and a sign,
  trailing text ignored), and must be 0 to 65535.
- User info splits at the last `@`, and the password at the first colon.
- Control bytes (below 0x20, and 0x7f) become `_` in every component.

`fromUri()` compares the URI's scheme with `http`/`https` before
lowercasing, so `HTTP://h:81` keeps the default HTTP port. Breaking that
rule on purpose fails the replay.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | RequestContextTest 16 ported, 1 `n/a`, 1 pending | all pass |
| `parse_url()` (`crates/php-value/tests/data/parse_url.json`, `oracle/recorders/php_value_parse_url.php`) | 119 hand-picked, 6,000 generated strings over its special characters, 6,000 generated scheme/authority/path/query/fragment combinations | all match |
| `fromUri()` (`crates/routing/tests/data/request_context.json`, `oracle/recorders/routing_request_context.php`) | 3,000 generated URIs (both scheme cases, ports, backslashes before and after `?`/`#`, control bytes and spaces at the ends) with varied default hosts, schemes and ports; 10 `setBaseUrl()` | all match |

## Checks

`cargo fmt --check` ok, `cargo clippy --all-targets -- -D warnings`
clean, `scripts/sources.sh --check` ok, `scripts/parity-check.sh` ok
(routing 94 ported, 1 n/a, 801 pending). `cargo test` on the Windows
host: 2,526 passed, 1 failed, the known Windows-only
`symfony-filesystem` `testReadNonExistentFile` (OS error text), which
this branch does not touch (before the rebase onto 99caa76). After it: see
below. `scripts/linux-test.sh -p symfony-routing -p
php-value` on the committed branch (fmt, clippy, tests): 125 passed, 0
failed.

## Advisories

No advisory in the upstream watch names RequestContext or the exception
classes. GHSA-72xp-p242-47p9 stays with the generator stage.

## Not done

`fromRequest()` (with HttpFoundation's Request). The matcher and
generator stages follow.

## Demo comparison

Not run: paused by the owner (November 23). The demo does not use these
classes.
