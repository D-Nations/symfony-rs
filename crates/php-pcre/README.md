# php-pcre

Private workspace crate: PHP's `preg_*` patterns translated into
`fancy-regex`, moved out of symfony-finder so that symfony-string can use
it too. Written from PHP's and PCRE2's documentation; no PHP or PCRE
source (PLAN.md decision 8). MIT.

| Contract | Evidence |
|---|---|
| `Pcre::new()`: delimiters, modifiers `imsxUnADu`, PCRE2 syntax translated or refused (`PcreError::Unsupported`) | symfony-finder `tests/php.rs`: 137 patterns x 62 subjects recorded from PHP 8.4.27 |
| `captures_from()`, `replace()`, `replace_callback()`, `preg_quote()` | the same corpus, and the Finder and String tests that use them |

What is not translated (recursion, conditionals, verbs, `\K`, `\X`, `\C`,
branch reset, callouts, `/J`, `/r`, POSIX classes under `/u`) is refused
rather than approximated.
