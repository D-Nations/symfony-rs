# Symfony OptionsResolver

Private workspace crate: a port of `symfony/options-resolver` v8.1.0
(`88f9c561`), as installed by symfony/demo. MIT; Symfony source and tests
copyright Fabien Potencier. No PHP source code is used: the PHP built-ins
in `src/php.rs` are written from PHP's documented and observed behaviour.
Unofficial, not affiliated with or endorsed by Symfony or PHP.

```rust
use php_value::{php_array, Value};
use symfony_options_resolver::{DefaultValue, OptionClosure, OptionsResolver};

let resolver = OptionsResolver::new();
resolver
    .set_default("host", "localhost").unwrap()
    .set_default("port", DefaultValue::lazy(|o| {
        Ok(if o.offset_get("host")? == Value::from("localhost") { 8080.into() } else { 80.into() })
    })).unwrap()
    .set_allowed_types("port", "int").unwrap()
    .set_normalizer("host", OptionClosure::new(|_, v| Ok(v))).unwrap();
let options = resolver.resolve(&php_array!["host" => "localhost"]).unwrap();
assert_eq!(options, php_array!["host" => "localhost", "port" => 8080]);
```

| Contract | Source | Evidence |
|---|---|---|
| Defaults, lazy defaults (with or without the previous value), required, defined, missing, remove, clear, ignore-undefined; resolved arrays in PHP's key order, numeric option names as int keys | `OptionsResolver.php` | OptionsResolverTest; recorded edge cases |
| Allowed types: the `is_*()` names, `null`, class names (`instanceof`, leading `\`, any case), `T[]` at any depth, unions and parenthesised groups, `trim()`; the "but is of type" / "one of the elements is of type" messages | `OptionsResolver.php` | OptionsResolverTest (its three type providers replayed); 345 recorded type cases |
| Allowed values (`===` or closures), `addAllowedValues()` with `array_merge()` key rules, `formatValue()` (PHP's `(string)` of floats), Info suffix | `OptionsResolver.php` | OptionsResolverTest; recorded cases |
| Normalizers (set, add, prepend), deprecations (strings with `%name%`, closures, `''` to skip, `offsetGet($o, false)`), triggered through `trigger_deprecation()` | `OptionsResolver.php` | OptionsResolverTest::testDeprecationMessages (11 rows) |
| Nested and prototype options, parent access, cyclic dependency detection, `name[child]` / `name[index][child]` prefixes, including upstream's shift of `$parentsOptions` after each message | `OptionsResolver.php` | OptionsResolverTest; recorded cases |
| Array access (`offsetGet`, `offsetExists`, `count`) only while resolving; every mutator throws `AccessException` while resolving | `OptionsResolver.php`, `Options.php` | OptionsResolverTest |
| `define()` and the option configurator | `OptionConfigurator.php` | OptionsResolverTest |
| Introspection of defaults, lazy closures, types, values, normalizers, deprecations and nested options | `Debug/OptionsResolverIntrospector.php` | OptionsResolverIntrospectorTest (24) |
| Exception classes and their hierarchy (`Class::is_a`) | `Exception/*.php` | Both test classes |

Vectors: `scripts/record.sh options-resolver` writes
`tests/data/options_resolver_test.json` (OptionsResolverTest's data
providers, run on the oracle) and `tests/data/edges.json`
(`oracle/recorders/options_resolver_edges.php`, PHP 8.4.27).

## Rust shape

- `OptionsResolver` is a handle to shared state (`Arc<Mutex<_>>`, `Send +
  Sync`); methods take `&self`, and no lock is held while a user closure
  runs, so closures re-enter the resolver as PHP's do. `handle()` is the
  same object; `resolve()` copies the state, as upstream's `clone $this`.
- Upstream decides by reflection whether a default closure is lazy (first
  parameter typed `Options`) and whether it takes the previous value. Here
  the caller says so: `DefaultValue::Lazy { closure, previous }`, or a
  plain value (a closure that is not lazy is a `php::ClosureValue`).
- Closures are `OptionClosure`, `NestedClosure` and `AllowedClosure`; their
  clones are the same closure (`same()` is `===`). A closure fails with an
  `Error`, which may be of any class (`Error::other`).
- Option names are `php_value::Key`s, so `"0"` is the int key `0`, as in
  PHP. `setRequired()`/`setDefined()`/`remove()`/`setAllowedTypes()` take
  one name or a list (`Names`). `ksort()` of option names in messages
  compares as PHP 8 does (numeric strings as numbers).
- Resources (`fopen()`) have no `php_value` variant: `php::Resource` stands
  for one, and is neither an object nor `instanceof` anything.

## Differences

- `callable`: only closures count as callable. PHP also accepts function
  names (`'strlen'`) and `[object, 'method']` arrays (PLAN.md decision
  12).
- `instanceof` knows the parents of the built-in classes used here
  (`DateTime`, `DateTimeImmutable`, `ArrayObject`, enum cases,
  `OptionsResolver`); other classes match their own name only.
- `ksort()` of option names whose PHP 8 comparison is not transitive
  (mixing numeric and non-numeric strings) may order them differently from
  PHP's sort; none of the recorded cases does.
