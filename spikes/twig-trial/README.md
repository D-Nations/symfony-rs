# Twig compiler trial

A small prototype that tests the M4 design (PLAN.md, "M4: Twig (build-time
compiler)") before the real port starts. **This is not the M4 port.** It
does not pass gate T1 or T2, it adds no parity manifest rows, and it is not
in `parity/sources.tsv` (the `//! Upstream:` lines show where the code
comes from, but `scripts/sources.sh` reads only `crates/`, `examples/` and
`fixtures/`). Results and findings: [docs/milestones/m4-twig-trial.md](../../docs/milestones/m4-twig-trial.md).

| Crate | What it is |
|---|---|
| `compiler/` (`twig-trial-compiler`) | Lexer, parser and Rust emitter (quote + prettyplease) for a Twig 3.30 subset, plus the `<twig:...>` pre-lexer of symfony/ux-twig-component. Used from `build.rs`. |
| `runtime/` (`twig-trial-runtime`) | What the emitted code calls: context access, `getAttribute()`, PHP truthiness and `==`, HTML escaping, `include()`, loops. Values are `php-value` values. |
| `demo/` (`twig-trial-demo`) | symfony/demo's `templates/blog/_post.html.twig` and `_post_tags.html.twig`, unchanged, compiled by `build.rs`; typed stand-ins for the entities; stubbed bridges; tests against PHP. |

## Supported subset

Text, `{{ }}`, `{% if/elseif/else %}`, `{% for k, v in x %}...{% else %}`,
comments, `-` and `~` whitespace control, constants, sequences, mappings
(with the `{a}` shortcut), `not`, unary `-`/`+`, `or`, `and`, `==`, `!=`,
`<`, `>`, `<=`, `>=`, `+`, `-`, `*`, `~`, `??`, the ternary, `.` access and
method calls, functions, filters, `include()` with one argument, `raw`, and
the `html` escaping strategy. Everything else is refused at build time
with the template name and line (`compiler/tests/refusals.rs`).

## Stubs

The trial leaves out the bridges whose providers do not exist yet. PHP and
Rust use the same stubs, so the comparison checks Twig itself:

- `path()` (Routing; the HTTP path is M2-A work in another tree) returns
  `/<route>?k=v&...` (`~` for null).
- `app.request.query.get()` reads fixture objects with the same shape as
  AppVariable, Request (public `$query`) and InputBag.
- `component()` (UX TwigComponent + Icons) returns `<stub-component .../>`
  and is safe for HTML, as the real function is.
- `format_datetime` (twig/intl-extra) returns its arguments as text.

## Run

```sh
cargo test -p twig-trial-compiler -p twig-trial-demo
scripts/record.sh twig-trial    # re-record demo/tests/data/php.json in the oracle
```

The templates are copied from symfony/demo (`f42980c`, MIT, see the root
LICENSE). The ported Twig code is under Twig's BSD-3-Clause licence
([LICENSE-Twig](LICENSE-Twig)).
