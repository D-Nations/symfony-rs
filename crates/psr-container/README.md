# PSR-11 container interface

Private workspace crate: a port of `psr/container` 2.0.2 (`c71ecc56`), as
installed by symfony/demo. MIT; psr/container copyright container-interop
and PHP Framework Interoperability Group. Unofficial.

| Contract | Source | Evidence |
|---|---|---|
| `ContainerInterface::get(id)` returns an entry (a php-value `Value`) or an error; `has(id)` says whether there is one | `ContainerInterface.php` | Used by http-kernel's `ContainerControllerResolver` and its upstream tests |
| The names of `ContainerExceptionInterface` and `NotFoundExceptionInterface`, for implementations' `instanceof` | `ContainerExceptionInterface.php`, `NotFoundExceptionInterface.php` | Constants only |

The package has no PHPUnit tests upstream (`parity/psr/container.tsv` is
empty). Identifiers are PHP strings (bytes). The exception interfaces add
nothing to `\Throwable`, so there is no Rust type for them: a container
returns any shared error, and says through its own `instanceof` answer
which interface that error implements.
