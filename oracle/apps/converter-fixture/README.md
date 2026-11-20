# Converter fixture app (M1-D)

A tiny Symfony application, the PHP side of the converter fixture
(PLAN.md, "Early conversion proof and second application";
[docs/converter-contract.md](../../../docs/converter-contract.md)).
Its Rust counterpart is [fixtures/converter-app](../../../fixtures/converter-app).

It exercises, on purpose and nothing more:

- an injected service (`Greeter`) with string, int, array and service
  arguments, the scalars and the array from container parameters;
- a readonly value object (`Greeting`) and a mutable shared service
  (`VisitCounter`) whose state must not leak from one request to the next;
- one successful action (`GET /greet/{name}`), reading a route parameter
  and a query parameter;
- one handled error path: the app's `kernel.exception` subscriber turns
  `GreetingException` and HTTP exceptions (no route, wrong method, a
  non-scalar query value) into `text/plain` responses.

It has no `composer.json`: `public/index.php` loads the packages pinned by
symfony/demo (`/demo/vendor`, as every oracle recorder does;
`parity/packages.tsv`) and registers the `Fixture\` namespace itself. It
runs only in the oracle image, mounted read-only, with its cache in `/tmp`:

    scripts/record.sh converter-fixture

writes [responses.json](responses.json).
