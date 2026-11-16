# Request lab

A small second application proving that the shared request crate can be used
without the Symfony Demo application's state or dependencies. Its JSON output
is a diagnostic representation, not a reproduction of a Symfony Demo page.
PHP array keys are represented as JSON object keys, preserving their order.

```sh
cargo run -p request-lab
```

The default address is `127.0.0.1:8012`. Override it with
`REQUEST_LAB_LISTEN_ADDR`. No database, translations or static assets are needed.

| Route | Purpose |
|---|---|
| `/inspect` | Any method: parsed query, form fields, cookies, preferred language (`en`/`fr`) and request scheme |
| `/inspect?scalar[]=x` | A 400 response demonstrating `InputBag::get()` rejecting an array |
| `/redirect` | A fixed 303 redirect to `/inspect` |
| `/error` | A fixed 400 JSON error |
| `/item/{id}` | A GET/HEAD route using the shared positive signed 64-bit requirement |

Route dispatch uses the same private routing crate as the demo. GET/HEAD
slash variants redirect with 301 and preserve the query; method refusals
send 405 with Allow. HEAD responses have empty bodies. `/item/0` and ids
outside signed 64-bit range return 404.

Examples:

```sh
curl 'http://127.0.0.1:8012/inspect?a.b=first&a_b=last&tags[]=rust'
curl -d 'name=one&name=two' http://127.0.0.1:8012/inspect
```

The application performs no writes and trusts no reverse proxies. Its
inspection endpoint reports request data, so keep it as a local development
tool. Tests cover duplicate query/cookie rules, PHP arrays, language selection,
untrusted forwarded headers, form methods, partial multipart failures, scalar
errors and fixed redirect behavior:

```sh
cargo test -p request-lab
```

MIT; see the repository LICENSE. This example is not an official Symfony project.
