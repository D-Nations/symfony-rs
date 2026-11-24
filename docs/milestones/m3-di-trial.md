# M3 trial: build-time DI container compiler (November 24)

Owner request (November 24): a small trial of the DI compiler design
before M3 starts, because the review of the port found it to be a top
design risk (it must emit Rust at build time, and DI was at 0%). Scope:
take symfony/demo's services and emit Rust container code that compiles
and resolves them. HTTP types are placeholders; the HTTP path (M2-A) is
done in another tree and this trial does not touch it.

Code: [spikes/di-trial/](../../spikes/di-trial/README.md). This trial
marks nothing complete.

## Existing alternatives (searched November 24)

No Rust crate reads a service graph from outside and emits a container.
Runtime containers ([dill](https://docs.rs/dill),
[dependency-injector](https://docs.rs/crate/dependency-injector/2.1.0))
resolve at run time, which AGENTS.md rules out. Compile-time crates
([shaku](https://docs.rs/crate/shaku),
[lockjaw](https://docs.rs/crate/lockjaw/latest/source/userguide/src/setup.md),
[fundle](https://docs.rs/crate/fundle/0.3.5),
[injectable-rs](https://docs.rs/injectable-rs),
[chassis](https://docs.rs/crate/chassis/0.2.0),
[rustyinject](https://docs.rs/crate/rustyinject/latest)) take the graph from
hand-written Rust attributes; Symfony's graph comes from bundles and
compiler passes, so each would repeat it. No Rust port of Symfony's
container was found. The stack row "Converter runtime contract" keeps its
choice (no DI crate).

## Result

**The design holds for this scope.** The demo's compiled container (548
services, 221 inline definitions, 3 aliases, 563 classes) is emitted as
Rust in `build.rs` and every service resolves:

- every service resolves in a request scope, and every application-scoped
  one in the application scope; the 35 public services and the public
  aliases resolve through `get()`, private ones do not;
- shared services are built once, non-shared ones at each use;
- two request scopes share application-scoped services and do not share
  request-scoped ones; two threads build their requests at once on one
  application scope; `Container` is `Send + Sync`, a request future is
  `Send`;
- constructor arguments, `%kernel.enabled_locales%`, `%env()%` and lazy
  closures and iterators come out as the definition says;
- the two cycles through method calls (Twig and `stimulus.helper`) give one
  instance of each, whichever end is asked for first, as in PHP;
- a missing service and a constructor cycle are build errors with
  Symfony's messages, and unsupported input is refused with its line.

Size: about 2,200 lines of compiler and runtime. The generated container
is 39,000 lines (1.9 MB). Regenerating and recompiling it takes about 2
seconds (debug); the first build of the demo crate with its dependencies
about 30 seconds. With placeholders, building every service of one
request takes 1.6 ms (debug), and the services one page needs
(`http_kernel` and a controller) 66 µs; real constructors will cost more.

## What the trial shows (problems for M3)

1. **Most of the graph is request-scoped.** 39 services carry Symfony's
   `kernel.reset` tag (state kept between uses; a good seed list for
   framework services); with `service_container` that makes 40 seeds.
   Everything that reaches one of them becomes
   request-scoped: 349 of 548 services (64%). Counting only eager edges it
   would be 267, only constructor edges 227. Examples: the router, Twig,
   Doctrine, every controller (through `service_container`). Each request
   then builds hundreds of objects. Ways to reduce it, for M3 to choose:
   closures and locators that take the request scope as an argument when
   called (so they do not make their owner request-scoped); a per-class
   "no writes after construction" analysis in the converter instead of
   tags; splitting a service's state into an application part and a
   request part by hand for the few hot framework services.
2. **XmlDumper's file is not a complete interface.** It writes tagged
   iterators and locators as their tag only (the recorder replaces them
   with the resolved services), it keeps `%param%` references and `%%`
   escapes that `PhpDumper::export()` resolves, and it lacks
   `container.build_id` and `container.build_hash`, which `PhpDumper`
   adds. The compiler reproduces these rules. If symfony2rust reads PHP's
   compiled container, it needs its own dumper (or these fixes); if M3
   runs the compiler passes in Rust, the Rust `ContainerBuilder` is the
   input and this does not arise.
3. **The declared class is not always the type.** For 2 of 34 public
   services PHP returns another type than the definition's class
   (`cache.system`: a factory declared as `AdapterInterface` returns
   `PhpFilesAdapter`; `container.env_var_processors_locator`:
   `ServiceLocator` is built as `Argument\ServiceLocator`). Eleven inline
   factory definitions return scalars or untyped values (nine declared
   `bool`, `int` or `string`, two with no class). Typed accessors need return types from the converter or from
   the ported classes, not only from the container.
4. **Cycles through method calls need PHP's order.** PHP stores a service
   before its calls run, so `twig -> addExtension(StimulusTwigExtension) ->
   stimulus.helper -> twig` works. The runtime reproduces this (store,
   then a re-entrant "configure once"; a service reached again while it
   is built returns the stored one). With real types this needs interior
   mutability in Twig's `Environment` (`addExtension` after it is shared),
   and the `Arc` cycle leaks one Twig per request unless one side holds a
   `Weak`. This joins M4 finding 3 (extensions known at build time):
   better to build the Environment with its extensions before it is
   shared.
5. **Withers return a new object.** The HTML sanitizer's config uses
   `returns-clone` calls; such a service is complete only after its calls,
   so it is stored after them, as `PhpDumper` does.
6. **Lazy services need a Rust form.** Six services are `lazy` (among them
   Doctrine's EntityManager). PHP injects a ghost object; the trial
   injects a closure (`Arg::Lazy`). Real types need a proxy type or a
   `Lazy<T>` field in the class that receives it.
7. **`%env()%` needs the env var processors at run time.** Six
   expressions with the `resolve`, `bool`, `int`, `not`, `default`, `key`
   and `query_string` processors. They stay as placeholders here.
8. **Synthetic services.** `kernel` is passed to `Container::new`;
   `service_container` is the request scope itself, so every service that
   receives it (all controller locators) is request-scoped.
9. **Optional references.** `services_resetter` holds 44
   `ignore_uninitialized` references (5 of them to services that do not
   exist in prod); these resolve to nothing until the service is built,
   which needs an "is it built?" accessor per shared service.

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

The oracle image could not be built in this session (deb.debian.org is
not allowed by its network). `data/container.xml` and
`tests/data/php_classes.json` were recorded with the same base image,
`php:8.4.27-cli-bookworm`, without the `intl` extension, which compiling
the container does not use. Composer could not download the demo's
packages either, so `vendor/` was filled with a shallow `git fetch` of
each package at its `composer.lock` reference, then `composer install
--no-dev` without network wrote the autoloader. Re-record in the real
oracle with `scripts/record.sh di-trial`; `git diff` must then show no
change.

## New dependency

None beyond the twig trial's: `quote`, `syn`, `proc-macro2` and
`prettyplease` in the trial compiler (see the stack row "Twig trial
emission"; the DI trial row repeats the choice), `serde_json` in tests.
