# Symfony DI emitter

Private workspace crate (`symfony-di-emitter`): the build-time emitter that
turns a compiled Symfony service graph into a Rust container, for
`build.rs`. It is the part of PLAN.md's M3 that replaces `PhpDumper`, and
it ports no PHP file. It grew from the November 24 DI trial's compiler
([docs/milestones/m3-di-trial.md](../../docs/milestones/m3-di-trial.md)).
MIT. Unofficial, not affiliated with or endorsed by Symfony.

```rust,ignore
use symfony_di_emitter::{compile, Config, DumperParameters, Graph};

// container: a compiled symfony_dependency_injection::ContainerBuilder
let graph = Graph::from_container_builder(&container, &DumperParameters::from_seed(b"app"))?;
let output = compile(&graph, &Config { request_scoped_tags: vec!["kernel.reset".into()], ..Config::default() })?;
std::fs::write(out_dir.join("container.rs"), output.code)?;
```

## Inputs

- `Graph::from_container_builder(&ContainerBuilder, &DumperParameters)`
  reads a **compiled** `ContainerBuilder` of `symfony-dependency-injection`
  directly: definitions, aliases, the parameter bag and the argument
  objects. This is the M3 path: loaders and compiler passes run in Rust.
- `Graph::from_xml(&[u8], &DumperParameters)` reads `XmlDumper`'s file of a
  container compiled by PHP (`spikes/di-trial/demo` until the bundles are
  ported). `compile_xml()` does both steps for such a file.

Both give the same `Graph` for the same container (graphs compare equal
whatever their refusal locations). `DumperParameters` are
`container.build_hash` and `container.build_id`, which `PhpDumper` adds and
arguments may name; they come from a seed the caller chooses.

The emitted code still builds the DI trial's placeholders
(`spikes/di-trial/runtime`, `di-trial-runtime`): a recorded `Instance` per
service where the real container will call each ported class's
constructor.

## Contract

| Contract | Source | Evidence |
|---|---|---|
| A compiled `ContainerBuilder` is read as `XmlDumper` writes it and the XML reader reads it back: the same definitions (inline ones numbered in dump order), aliases (the end of each alias chain), parameters and arguments; env placeholders as `%env(NAME)%`; `%%`, `%env()%` and `%name%` in strings as `PhpDumper::export()` reads them | `Dumper/XmlDumper.php` (the format), `ParameterBag/ParameterBag.php` (`resolveString()`'s pattern) | tests/equivalence.rs: 44 upstream YAML fixtures (`YamlFileLoader`) and 16 of the DI crate's rebuilt upstream test containers give equal graphs and the same emitted Rust both ways; 10 more are refused by both |
| A tagged iterator is read as the services it resolved to (the recorder's `untag()` for the XML input) | `oracle/recorders/di_container.php` | `tagged_iterators_are_read_as_their_services` |
| Scalars keep their PHP type where the XML text loses it (a float without a fractional part becomes an int, `"NULL"`/`"True"` become null/bool through `XmlUtils::phpize()`) | | `scalars_the_xml_text_loses` |
| Missing services (`on-invalid="exception"`) and constructor cycles are build errors with Symfony's messages; optional references to missing services resolve to null or are dropped; cycles through calls, properties and configurators are allowed | `CheckExceptionOnInvalidReferenceBehaviorPass.php`, `Exception/ServiceCircularReferenceException.php` (messages) | tests/refusals.rs (XML and builder cases) |
| Scopes: seeds by tag (`Config::request_scoped_tags`), id and `service_container`, propagated to every service that reaches one, eagerly or lazily | converter contract 0.2, section 4 | tests/refusals.rs; spikes/di-trial/demo tests |
| Emission: one accessor per service, shared services in cells, non-shared and inline ones built at each use, withers stored after their calls, lazy services and closures as closures, `get()` for public services and public aliases, `parameter()` for the compiled parameters (unescaped; `%env()%` unresolved) | `PhpDumper` (order of properties, calls, configurator) | spikes/di-trial/demo tests (symfony/demo's 548 services) |

A refusal has a `Location`: the line of the XML file, or the service id
(for an inline definition, the named service that holds it) or parameter
name of a `ContainerBuilder`.

## Refusals

Anything outside the table is refused at build time, never approximated.
The messages name the construct as `XmlDumper` writes it, so both readers
refuse it with the same text:

| Input | Message |
|---|---|
| A reference to a missing service | `The service "a" has a dependency on a non-existent service "b".` |
| A constructor (or factory) cycle | `Circular reference detected for service "a", path: "a -> b -> a".` |
| A `%name%` of a parameter that does not exist | `You have requested a non-existent parameter "name".` |
| A decorated (unresolved) or abstract definition | `unsupported attribute decorates="b" on <service>`, `unsupported attribute abstract="true" on <service>` |
| A definition with a file | `unsupported element <file> in a service` |
| A definition without a class and without a factory | `service "a" has no class` |
| Expressions, binary strings, abstract arguments, env closures, service locators (tagged or not) | `unsupported argument type="expression"` (`binary`, `abstract`, `env_closure`, `service_locator`, `tagged_locator`) |
| A service closure of something other than a reference | `unsupported argument type="service_closure" of a value that is not a reference` |
| A parameter holding a reference, definition, argument object, enum case, expression or binary string | `unsupported parameter type "service"` (`constant`, `expression`, `binary`, ...) |
| Any other object | Symfony's `Unable to dump a service container if a parameter is an object or a resource, got "stdClass".` |
| An uncompiled `ContainerBuilder` | `the container is not compiled: ...` |
| A closure, inline service or iterator inside an iterator; a synthetic service that becomes request-scoped | as worded in `emit.rs` |

The XML reader also refuses every element and attribute it does not know,
with its line (`unsupported attribute tag="..." on <argument>` for a
tagged iterator, for example).

## Checks

```sh
cargo test -p symfony-di-emitter -p di-trial-demo
```

Tests that read `upstream/` (the YAML fixtures) skip, with a note, when it
is not fetched.
