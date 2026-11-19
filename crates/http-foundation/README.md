# Symfony request compatibility subset

Private workspace crate of symfony-rs, extracted from
[symfony-demo-rs](https://github.com/D-Nations/symfony-demo-rs). It implements the
request semantics already exercised by the demo, with the PHP source named
in each module. It is not a complete port of Symfony HttpFoundation.

| Module/API | Source behavior | Evidence | Limits |
|---|---|---|---|
| `query` | PHP `php_default_treat_data`, `php_register_variable_ex`; Symfony `InputBag::get()` | Existing query tests, demo form tests, request-lab HTTP tests | Fixed PHP defaults: 1,000 variables, nesting 64; invalid UTF-8 is replaced and tracked for scalar values |
| `cookies` | PHP `PARSE_COOKIE` | Existing cookie test, request-lab HTTP test | The existing demo subset; not a full Symfony Cookie/ResponseHeaderBag API |
| `multipart` | PHP 8.4.27 `main/rfc1867.c` | 156 PHP-recorded vectors plus parser tests | Text fields only; file parts are consumed and discarded; fixed PHP upload limits |
| `form_fields` | Symfony `Request::createFromGlobals()` method/format selection | Demo write tests, request-lab HTTP tests, multipart comparison | URL-encoded and multipart bodies; no JSON payload API |
| `accept_language` | Symfony `Request`, `AcceptHeader`, `AcceptHeaderItem`, `HeaderUtils` | Original 77-header test retained in the demo integration suite; float parsing test here | Caller supplies supported locale strings; no demo locale enum dependency |
| `scheme` | Symfony `Request` trusted HTTPS proxy subset | Demo proxy/security regression tests, request-lab untrusted-header test | Axum adapter; exact trusted peer IPs and one `X-Forwarded-Proto` value only |

HTTP adapters bound body reads using `POST_MAX_SIZE` (8 MiB), and supply an
empty body after a read failure. No socket I/O or body buffering is done by
the parsers. The only runtime dependency is the already-used Axum, for
`scheme`'s request-parts adapter. JSON, serde and base64 are test dependencies.

Run this crate independently with:

```sh
cargo test -p symfony-http-foundation
```

symfony-demo-rs's `src/request/mod.rs` re-exports this API. The second consumer is
[`request-lab`](../../examples/request-lab/README.md); it has no dependency on
the demo crate, its database, templates, translations or built assets.

The multipart test snapshot is copied from symfony-demo-rs's
`data/request/multipart_vectors.json`; its `compare/multipart_vectors.py` updates both when regenerating PHP evidence.
The application-specific language test remains in symfony-demo-rs at
`tests/request_compatibility.rs`, so it continues to verify the demo's actual
enabled locale set rather than baking that set into this crate.

Not implemented: general Request/Response objects, HeaderBag and ParameterBag
APIs, uploaded-file handling, JSON payload access, sessions, response cache
negotiation, streamed/binary responses, general proxy handling, routing,
HttpKernel events, a service container or bundle configuration. The next
milestone should choose a small additional compatibility contract and port
its upstream tests, rather than treating these omissions as supported.

MIT; derived from Symfony Demo (copyright Fabien Potencier) and the existing
Rust port. See the repository's LICENSE and source mappings. Unofficial;
not affiliated with or endorsed by Symfony.
