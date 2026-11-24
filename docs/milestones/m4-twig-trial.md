# M4 trial: build-time Twig compiler (November 24)

Owner request (November 24): a small trial of the Twig compiler design
before M4 starts, because the review of the port found it to be a top
design risk. Scope: compile one real template from symfony/demo to Rust at
build time and compare the output with real PHP Twig. Bridges that need
the HTTP path (`path()`, `app.request`) are stubbed; the HTTP path (M2-A)
is done in another tree and this trial does not touch it.

Code: [spikes/twig-trial/](../../spikes/twig-trial/README.md). This trial
does not pass gate T1 or T2 and marks nothing complete.

## Result

**The design holds for this scope.** `blog/_post.html.twig` and the
template it includes, `blog/_post_tags.html.twig`, are compiled unchanged
in `build.rs`. The Rust output is byte-identical to Twig 3.30.0 on PHP
8.4.27 for:

- 5 post cases (tags or no tags, the selected tag, HTML characters, PHP
  numeric-string `==`, non-ASCII text);
- 10 language snippets (whitespace control, precedence, `==` edge cases,
  `for`/`else`, `if`/`elseif`, loop scope, arrays and attributes,
  comments, number printing, escaping and safe analysis, logic and `??`).

The first comparison run found one difference, in loop scope: after a
`for`, Twig unsets the loop variables before it restores the outer
context, so an outer variable with the same name comes back. The fix is in
`restore_after_loop`; the snippet `05-loop-scope.html.twig` keeps it.

Size: about 2,100 lines of compiler and runtime for the subset; the two
templates plus 10 snippets give about 970 lines of generated Rust.

## What the trial shows (problems for M4)

1. **`php-value` objects cannot be read by Twig.** `PhpObject` has no
   method calls, method list or iteration. Twig's `.` lookup needs all of
   them (array key, then public property, then `name()`, `getName()`,
   `isName()`, `hasName()`, with `hasX` losing to `isX`), and so do
   PropertyAccess and ExpressionLanguage. The trial wraps objects in a
   runtime `Obj` type and finds it by downcast. M4 should add this hook to
   `php-value` once, for all three components, and generate the method
   table with a derive. The method list must equal PHP's public methods, or
   lookup can pick another method.
2. **`php-value` has no PHP 8 `==` or `<=>`.** The trial runtime has a
   subset (null, bool, int, float, numeric and non-numeric strings). It
   belongs in `php-value`, checked against oracle vectors.
3. **The extension set must be known at build time.** Twig builds its
   operator list from the registered extensions, and ux-twig-component
   replaces the lexer. The compiler takes the functions, filters and the
   pre-lexer as build-time input, and this works. It means that TwigBundle
   configuration and the registered extensions must reach `build.rs`, so
   M4 depends on the build-time container work of M3 for real apps.
4. **Escaping is decided at build time, so `is_safe_callback` must be a
   compile-time function.** Example: RoutingExtension marks `path()` safe
   only when its parameters are a small constant mapping. The trial has
   only fixed `is_safe` flags (the stubbed `path()` is unsafe, which is
   correct for both calls in these templates).
5. **Every `.` lookup is dynamic.** Twig contexts are not typed, so the
   compiler cannot resolve `post.title` to a Rust method. The cost is a
   string match per lookup. A per-class table from a derive makes it
   cheap; type hints from the converter could make it static later.
6. **Line numbers must be on every node.** The first version used line 0
   for `if` and `for` conditions. Fixed; M4 must keep Twig's line for each
   expression for error messages. The pre-lexer puts a multi-line
   `<twig:...>` tag on one line, which moves later line numbers, as in PHP.
7. **Strict variables are not in the trial.** The demo's `dev` environment
   uses `strict_variables: true`; M4 needs the RuntimeError messages of
   `getAttribute()` for that mode.
8. **Float printing reuses `php-value`.** `0.1 + 0.2` prints `0.3` and
   `2^63` prints `9.2233720368548E+18` (PHP's `precision` 14), and
   `php_value::numeric::float_to_string` already gives that.

## Is MiniJinja a better start? (decision 2)

No new reason for it. The hard parts the trial found (PHP values, `==`,
attribute lookup, escaping decisions, extension bridges) are the same with
MiniJinja, and MiniJinja would add Jinja semantics to undo and parse
templates at run time. Rust emission itself was the easy part.

## Checks

```
cargo fmt --check                                  ok
cargo clippy --all-targets -- -D warnings          ok
cargo test                                         see the PR
scripts/sources.sh --check                         ok (spikes/ is not scanned)
scripts/parity-check.sh                            ok (no manifest change)
```

The symfony-demo-rs comparison was not run (paused by the owner,
November 23).

## Oracle note

The oracle image (`oracle/Dockerfile`) could not be built in this session:
its `apt-get` needs deb.debian.org, which this session's network does not
allow. `demo/tests/data/php.json` was recorded with the same base image,
`php:8.4.27-cli-bookworm`, without the oracle's `intl` and `pdo_sqlite`
extensions, which the recorder does not use. Without `/demo/vendor`, the
recorder loads twig/twig, symfony/ux-twig-component and
symfony/deprecation-contracts from `upstream/` at the pinned commits.
Re-record in the real oracle with `scripts/record.sh twig-trial`; `git
diff` must then show no change.

## New dependency

`prettyplease` 0.2 (with `syn` 2, `quote`, `proc-macro2`, which were
already in Cargo.lock) for the trial compiler only. PLAN.md planned
"quote + prettyplease, as php2rust does"; see the stack row.
