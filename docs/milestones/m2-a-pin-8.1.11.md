# M2-A: version move to v8.1.11 (decision 16)

November 23. The owner chose to move the pins to v8.1.11 (PLAN.md
decision 16) instead of leaving `makeDisposition()`, IpUtils and
`setTrustedProxies()` unported at v8.1.10.

## Scope of the move

Moved: `symfony/http-foundation` (`30fa3ef`), `symfony/http-kernel`
(`4606585`) and `symfony/routing` (`f3fefc6`), from v8.1.10 to v8.1.11.
Their 8.1.11 `composer.json` files need no other package at 8.1.11
(http-kernel asks for `symfony/http-foundation ^7.4|^8.0` and the like), so
nothing else moves. The second app's lock has 38 packages at 8.1.11; moving
them all would be a separate version move, with its own triage.

`parity/packages.tsv` marks the three rows as moved past the demo's lock.
The PHP demo original stays read-only at its lock (v8.1.10), so the oracle
mounts the moved checkouts from `upstream/` over its vendor copy
(`oracle/compose.yaml`, and the converter fixture's `docker run`).
`scripts/fetch-upstream.sh --verify ../symfony-demo/vendor` reports them as
moved, and fails if one is not mounted. The overlay works because 8.1.11
adds no class file and the packages autoload by PSR-4; a probe in the
oracle sees `IpUtils::PRIVATE_RANGES`. The oracle image is unchanged.

## What 8.1.11 changes

| Package | Changed source files | New tests |
|---|---|---|
| http-foundation | `BinaryFileResponse.php`, `HeaderUtils.php` (`/D` anchor), `IpUtils.php` (`PRIVATE_RANGES`, IPv4-mapped check), `Request.php` (`setTrustedProxies()` uses `PRIVATE_RANGES`; `prepareBaseUrl()` refuses a `//host` base URL and partial path segments; `getUrlencodedPrefix()` without a `{n}` quantifier), `RequestMatcher/MethodRequestMatcher.php`, `Session/Storage/Handler/SessionHandlerFactory.php`, `UrlHelper.php` | 13 |
| http-kernel | `Attribute/MapRequestPayload.php`, `RequestPayloadValueResolver.php`, `CacheAttributeListener.php`, `RouterListener.php` (restores the parent's routing context after a sub-request), `Kernel.php`, `FileProfilerStorage.php` | 5 |
| routing | `Matcher/Dumper/CompiledUrlMatcherDumper.php` (host variables can no longer match the path) | 24 (5 per matcher test class) |

Manifests regenerated: the 42 new tests start `pending`; no row was
dropped, and no existing ported row's upstream test changed (UrlMatcherTest
only gains tests). The ported tests and READMEs now name the v8.1.11
commits.

Re-recorded at v8.1.11, all byte-identical: the routing matcher (90 cases),
the http-foundation bags, HeaderUtils, Cookie, header-bag and ServerBag
vectors, php-value's dates, and the converter fixture's 16 responses (only
its package label changes). None of the changed files is reached by them.

The upstream watch after the move lists no advisory affecting the pins and
no new advisory for the three packages. It does list newer patch releases of
other ported packages (config, console, css-selector, finder,
options-resolver, yaml): outside this move.

## Ported at v8.1.11

- `IpUtils.php` (`crates/http-foundation/src/ip_utils.rs`): IpUtilsTest
  18 of 19; `testAnIpv6WithOptionDisabledIpv6` is `n/a` (IPv6 is always
  available; PHP skips it too unless built with `disable-ipv6`).
- `HeaderUtils::makeDisposition()`: HeaderUtilsTest's three
  `makeDisposition` tests, and a test pinning the `/D` anchor (the fix's own
  test is BinaryFileResponseTest's, not ported).
- php-value: `net` (`FILTER_VALIDATE_IP`'s grammar, `inet_pton()`,
  `inet_ntop()`, `ip2long()`) and three numeric helpers for PHP 8's
  string/int rules; http-foundation's error type gains `ValueError`.

Not ported, waiting for their classes: `Request.php`'s fixes
(`setTrustedProxies()`, `prepareBaseUrl()`), UrlHelper, BinaryFileResponse,
MethodRequestMatcher, SessionHandlerFactory, the compiled matcher's host
fix, RouterListener and the other http-kernel listeners.

## Recorded answers

`oracle/recorders/http_foundation_ip_utils.php` (PHP 8.4.27, glibc in the
oracle image):

- 388 strings through `filter_var(FILTER_VALIDATE_IP)` with each flag,
  `inet_pton()` and `ip2long()`. All three accept the same addresses;
  `inet_pton()` and `ip2long()` throw a `ValueError` for a NUL byte, which
  `@` does not silence (it reaches `anonymize()`).
- 629 packed addresses through `inet_ntop()`: glibc writes IPv4-compatible
  addresses dotted (`::1.2.3.4`) besides IPv4-mapped ones.
- 37,802 IpUtils calls: `checkIp4()` and `checkIp6()` over 13 and 14 requests x
  1,242 subnets with 53 odd netmasks, `checkIp()` with four lists,
  `isPrivateIp()` and `anonymize()` with six byte counts. A netmask is a
  string under PHP 8's rules: `"8.5"` acts as 8, `" 8"` and `"8 "` as 8,
  `"8x"` fails the range check (it sorts after `"32"`), `"1x"` passes it
  and is a `TypeError` from `substr_compare()` in `checkIp4()`, while
  `checkIp6()`'s arithmetic reads `"10x"` as 10. `anonymize()` of a string
  that is not an IP is `TypeError: Unsupported operand types: bool &
  string`.

`oracle/recorders/http_foundation_header_utils.php` gains 2,880
`makeDisposition()` calls (5 types x 24 names x 24 fallbacks).

The Rust port matches all of them. Breaking a rule on purpose shows the
replay compares: allowing leading zeros in IPv4 makes 6 built-in and, with
the mapped check dropped, 169 IpUtils answers differ; v8.1.10's unanchored
check makes exactly the 100 recorded calls whose fallback ends in a newline
differ.

## Earlier releases and advisories

`scripts/record.sh http-foundation-releases` records IpUtils and
`makeDisposition()` from v8.0.8, v8.0.13, v8.1.10 and v8.1.11 into
`oracle/out/`:

- GHSA-38cx-cq6f-5755 (`PRIVATE_SUBNETS` without IPv6 transition forms,
  fixed in v8.0.13): v8.0.8 answers `isPrivateIp()` `false` for 13
  addresses (IPv4-compatible, 6to4, Teredo, NAT64) that v8.0.13, v8.1.11
  and the port answer `true`, including rows of IpUtilsTest's
  `testIsPrivateIp`. Verdict: `covered`.
- v8.1.10 → v8.1.11: `checkIp()` differs only for IPv4-mapped requests
  against IPv4 subnets (9 of 1,552 recorded calls); `isPrivateIp()` does
  not differ. `makeDisposition()` differs on the 100 calls whose fallback
  ends in a newline: v8.1.10 wrote the newline into the header value
  (`attachment; filename=foo\n; ...`), v8.1.11 throws.

## symfony-demo-rs comparison

Run against this branch through a temporary `[patch]` in symfony-demo-rs
(at `0ebf52a`; `cargo tree -i symfony-http-foundation` showed the
`symfony-rs-wt-j` checkout; the patch was reverted and nothing committed
there), with the original in the `compare-original` image `ee906322e48c`.
The control is the run made on the pinned revision `e467232` for the
previous stage (`compare/out/run-control-i`, valid for those three
revisions).

| Group | Branch | Control |
|---|---|---|
| Pages | 92 same, 0 differ | 92 / 0 |
| Admin flows | 68 / 0 | 68 / 0 |
| Locale flows | 19 / 0 | 19 / 0 |
| Security flows | 3 / 8 | 3 / 8 |
| Ties flows | 16 / 0 | 16 / 0 |
| Multipart | 92 / 0 | 92 / 0 |
| Live search | 124 / 3 | 124 / 3 |

Every diff file is identical to the control's (apart from the run paths in
its header lines), so the differences are the documented known ones and the
move adds none. That is expected: the PHP original runs v8.1.10 from its
lock, and none of the code 8.1.11 changes is ported in a path the demo
takes (it does not call IpUtils or `makeDisposition()`; Request,
UrlHelper, the compiled matcher and RouterListener are not ported).

## Checks

On the final tree, after rebasing onto `fe46b41` (PR #19):

- `cargo fmt --check`: ok.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test`: 168 suites, 2,156 passed, 0 failed.
- `scripts/sources.sh --check`: ok.
- `scripts/parity-check.sh`: exit 0; http-foundation 959 tests, 193
  ported, 5 `n/a`, 761 pending; http-kernel 1,045 / 10 / 0 / 1,035;
  routing 896 / 18 / 0 / 878.
