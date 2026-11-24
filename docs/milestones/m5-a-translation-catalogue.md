# M5-A: Translation catalogue operations (November 24, 2026)

Branch `m5/translation-catalogue`, isolated worktree
`../symfony-rs-wt-translation-catalogue`, based on `origin/main` at
`1b1994d`. Routing and HttpFoundation are being developed separately.
Only symfony-translation behaviour changes; php-value and Cargo.lock are
unchanged. No dependency added.

## Scope and upstream

Pinned symfony/translation v8.1.5, commit
`d9e1caba0d6b6f9a26710af8a2f88d37f001215a`. Read the complete files and
tests before implementing:

- Existing MessageCatalogue.php and its three interfaces: audit and retain
  the existing 18 MessageCatalogueTest cases. Add the separate
  Tests/Catalogue/MessageCatalogueTest.php::testIcuMetadataKept regression.
- Catalogue/OperationInterface.php and AbstractOperation.php: lazy domain
  union, per-domain cached message batches, result identity, locale/domain
  exceptions and ICU migration.
- Catalogue/MergeOperation.php: 12 upstream tests, including seven
  inherited from AbstractOperationTestCase.
- Catalogue/TargetOperation.php: 13 upstream tests, including the same
  seven inherited tests.

Regenerated the package manifest with oracle/manifest.php via
scripts/manifest.sh. All 26 Catalogue/ rows become ported. Package totals:
345 tracked methods, 81 ported, 2 n/a, 262 pending. The package remains
partial and is not added to parity/done.txt.

GitHub and crates.io search on November 24 compared Serenade, rust-i18n
and fluent-bundle. None demonstrated Symfony operation semantics on vectors.
Use the existing MessageCatalogue and php-value; the PLAN.md stack row
records this choice. No PHP VM or bridge added (rust-anything's PHP bridge
is unsupported; this repository's oracle supplies differential evidence).

## Behaviour pinned by the oracle

scripts/record.sh translation-catalogue records 524 scenarios with intl
(7,770 calls), and four scenarios without MessageFormatter (12 calls).
All 528 scenarios / 7,782 calls match exactly: ordered and typed arrays,
exception class, message and code, and raw result snapshots. No tolerated
mismatches. Oracle PHP 8.4.27, image
`sha256:6593774ab6aeeefe7addc469fd2c2c111de80a5b11c5ec25e3425dcbbc82c6bb`.
Recorders run in removed containers with repositories mounted read-only.

The no-formatter run uses php -n and directly requires the pinned package
files. Composer otherwise autoloads a MessageFormatter polyfill even with
intl disabled; merely checking extension_loaded('intl') would not exercise
the upstream early return.

The corpus crosses plain/ICU/empty/repeated-suffix/numeric domains,
integer and string ids, null messages, message and catalogue metadata,
source and target fallbacks, every migration batch and invalid batches.
It checks partial processing order, mutations before/after domains and
messages are cached, and mutation through the shared result handle.

Notable upstream behaviours retained:

- Source values win shared ids, even for TargetOperation. Fallback-aware
  has() excludes target-only messages supplied by the source fallback;
  target fallbacks can retain source messages.
- Processing ICU domains separately can subsequently replace values in
  the result. Cached message batches remain as originally computed.
- Per-message metadata follows its originating catalogue; catalogue
  metadata comes from the target, filling only null/missing values.
  Input resources and fallback links are not copied to the result.
- getMessages() uses strict domain membership: integer domain values
  cannot be requested as strings, though getResult() can process them.
- Invalid batch values raise the native InvalidArgumentException, not
  Translation's subclass. The check occurs inside the domain loop:
  empty catalogues or a missing formatter return without throwing.
- Migration preserves the upstream array_diff_key/replace/union order,
  including removal and insertion order and catalogue metadata handling.

API scope: constructors accept the existing MessageCatalogue handles;
custom implementations of PHP's MessageCatalogueInterface are not claimed.
OperationInterface is a Rust trait. Batch is a closed enum; the additional
byte-string batch boundary preserves dynamic validation order. The
formatter capability is supplied from build-time configuration.
The existing weak parent reference limitation remains documented.

## Validation

- cargo fmt --check: passed on Windows.
- cargo clippy --all-targets -- -D warnings: passed on Windows.
- cargo test: attempted on Windows; stopped at the unchanged
  symfony-filesystem testReadNonExistentFile. It asserts Linux's
  "No such file or directory" whereas Windows emits
  "The system cannot find the file specified." No test or rule changed.
- New operation tests: 12 MergeOperationTest + 13 TargetOperationTest pass.
- Oracle replay: all 528 scenarios / 7,782 calls pass.
- Final host Translation checks: 82 tests passed (the two filesystem
  recorder replays are Unix-only and run in Linux below).
- scripts/sources.sh --check: passed (exit 0).
- scripts/parity-check.sh: passed (exit 0), including all 26 new claims.
- scripts/fetch-upstream.sh --verify ../symfony-demo/vendor symfony/translation:
  "same symfony/translation v8.1.5".
- Standard scripts/linux-test.sh -p symfony-translation at 088f500: passed
  formatting, Clippy and all 84 Translation tests, including recorded XLIFF
  and Qt replays.
- Final full Linux workspace at 8765e49: formatting, Clippy all targets
  with warnings denied, and cargo test --locked --no-fail-fast passed:
  2408 passed, 0 failed, 0 ignored across 206 suite results.
  This includes all 84 Translation tests on the final implementation.

The default full scripts/linux-test.sh run initially failed Config's
unchanged XmlUtilsTest::testLoadFile: root bypasses chmod 000 and reads the
fixture. The full run therefore uses the same pinned Rust image and fresh
archive setup with Docker --cap-drop=DAC_OVERRIDE and
--cap-drop=DAC_READ_SEARCH. This removes privilege without changing or
skipping tests. The first restricted-capability run exposed missing Finder
upstream fixtures; fetching symfony/finder resolved those setup failures.
The final full run is green, including Config and Finder.

Reproduce the full Linux setup from the repository root (Git Bash on this
Windows host), with upstream/symfony/finder fetched:

```sh
export MSYS_NO_PATHCONV=1
host=$(cd upstream && pwd -W)
git archive --format=tar HEAD | docker run --rm -i \
  --cap-drop=DAC_OVERRIDE --cap-drop=DAC_READ_SEARCH \
  -v "$host:/upstream:ro" \
  rust@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7 \
  bash -euo pipefail -c '
    rustup component add rustfmt clippy >/dev/null 2>&1
    mkdir /work && cd /work && tar -x
    test -z "$(ls -A target 2>/dev/null)"
    ln -s /upstream upstream
    cargo fmt --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked --no-fail-fast
  '
```

No checks, tests or tracked runner scripts were weakened. All builds were
debug builds; all oracle and Rust containers exit with --rm.

The upstream-watch issue #23 (November 23) lists no translation advisory.
A direct GitHub advisory query found only GHSA-f75p-x5vm-83qp, published
May 30, 2024 (outside the watch's 400-day window), for translation
>=2.0.0,<2.0.17; fixed long before the v8.1.5 pin, in XML loading rather
than catalogue operations. No new watch triage row is needed for this stage.

## Remaining integration

The owner-paused symfony-demo-rs checks and page comparisons were not run;
all acceptance requiring them remains open. Other loaders, Translator,
formatters, extraction, commands and later application-provider integrations
stay pending in M5-A. No production build, deployment or merge performed.
