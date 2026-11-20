# Symfony Console (M1-B subset)

Private workspace crate: the parts of `symfony/console` v8.1.9 (`630e7419`)
that the Yaml and Dotenv commands use (PLAN.md, M1-B). MIT; Symfony source
and tests copyright Fabien Potencier. Unofficial, not affiliated with or
endorsed by Symfony. The rest of Console is M7 work; its manifest rows stay
`pending`.

| Contract | Source | Evidence |
|---|---|---|
| Style tags (`<info>`, inline `fg`/`bg`/`href`/`options`, `</>`), escaping, nesting, decoration on and off, wrapping to a width by graphemes | `Formatter/OutputFormatter.php` | OutputFormatterTest (22 of 22) |
| Styles: colors, bright colors, hex colors by color mode, options, hyperlinks (not on JetBrains JediTerm, old Konsole or IDEA) | `Formatter/OutputFormatterStyle.php`, `Color.php`, `Output/AnsiColorMode.php` | OutputFormatterStyleTest (5 of 5) |
| Style stack: push, pop, pop to a matching style, nesting errors | `Formatter/OutputFormatterStyleStack.php` | OutputFormatterStyleStackTest (5 of 5) |
| Output: verbosity levels and per-message verbosity, `OUTPUT_NORMAL` / `RAW` / `PLAIN` (PHP's `strip_tags()` written from the manual), decoration, buffered and stream outputs, color support detection (`NO_COLOR`, `FORCE_COLOR`, TTY, `TERM`) | `Output/Output.php`, `Output/StreamOutput.php`, `Output/BufferedOutput.php` | OutputTest (11 of 11), StreamOutputTest (4 of 6, 2 `n/a`); `strip_tags()` on 6,137 calls recorded from PHP |
| Input: arguments and options (modes, defaults, shortcuts, negation, suggested values, deprecations), definitions and synopsis, ArrayInput parsing, `getFirstArgument()`, `hasParameterOption()` / `getParameterOption()`, `__toString()`, `unparse()`; ArgvInput and CompletionInput parsing are ported, their tests not yet | `Input/*.php`, `Completion/CompletionInput.php`, `Completion/CompletionSuggestions.php`, `Completion/Suggestion.php` | InputArgumentTest (15 of 15), InputOptionTest (24 of 24), InputDefinitionTest (32 of 32), InputTest (11 of 12), ArrayInputTest (8 of 8) |
| Color mode from `COLORTERM` / `TERM`; width and height from `COLUMNS` / `LINES`, else `stty -a` | `Terminal.php` | Formatter tests; `stty` parsing unit test |
| `Helper::width()`, `length()`, `substr()`, `removeDecoration()` | `Helper/Helper.php` | Through the formatter and style tests |

## Rust shape

- PHP's environment is read through `env`: `getenv()` and `$_SERVER`, with
  `putenv()` and `$_SERVER` writes kept in a per-thread overlay (changing
  the process environment is `unsafe`). Terminal's static caches are per
  thread.
- Exceptions are `ConsoleError`, one variant per class, with upstream's
  messages.
- Like symfony-demo-rs's console, this behaves as Symfony does on Linux on
  every system (`stty -a` for the terminal size).

## Differences

- The truecolor row of `testInlineStyleOptions` runs only when `COLORTERM`
  is `truecolor`, as upstream's does; it is skipped elsewhere.
- Graphemes, NFC and widths come from `symfony-string`; see its README.
