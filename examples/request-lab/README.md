# Request lab

A small second application proving that the ported Symfony crates can be
used without the Symfony Demo application's state or dependencies. Its JSON
output is a diagnostic representation, not a reproduction of a Symfony Demo
page. PHP array keys are represented as JSON object keys, preserving their
order.

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
| `/item/{id}` | A GET/HEAD route whose requirement accepts 1 to `PHP_INT_MAX` |

## How it runs

The lab runs on the ported Symfony kernel, as a PHP application behind
nginx and PHP-FPM would:

- `symfony-runtime`'s `FrontController` turns each Axum request into the
  FastCGI parameters nginx sends for Symfony's documented location, builds
  the superglobals as PHP-FPM does (`symfony-http-foundation`'s `sapi`,
  recorded from PHP-FPM 8.4.27), runs `Request::createFromGlobals()`,
  hands the request to `HttpKernelRunner` and turns what `Response::send()`
  recorded into the HTTP response.
- Its routes are a routing `RouteCollection` (`_controller` defaults naming
  `App\Controller\LabController::<action>`), matched by a
  `RedirectableUrlMatcher` through HttpKernel's `RouterListener`. The
  matcher's `redirect()` is the lab's own `slashRedirect` action: GET/HEAD
  slash variants redirect with 301 and keep the query string as sent.
- `LabController` is declared in a `ClassRegistry` for the
  `ControllerResolver`; its actions declare their parameters
  (`Request $request`, `string $path`) for the `ArgumentResolver`.
- A `kernel.exception` listener answers a `BadRequestHttpException` with
  `{"error": <message>}` and any other HTTP exception with an empty body;
  the kernel applies the exception's status and headers (404, 405 with
  `Allow`). Other throwables are PHP's uncaught exception: a 500.
- JSON is `JsonResponse` with `JSON_FORCE_OBJECT | JSON_UNESCAPED_SLASHES
  | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE`, which gives the
  bytes the lab wrote before it ran on the kernel.

What changed for a client when the lab moved from the typed Axum subset
(`Kernel<C>`) to the ported kernel: statuses, bodies and the headers it
sent before are unchanged for the 29 requests compared in the stage log
(`docs/milestones/m2-a-axum-adapter-request-lab.md`); Symfony's Response
adds `Cache-Control: no-cache, private` and `Date` to every response and
`Content-Type: text/html; charset=UTF-8` to the bodiless ones (redirects,
404, 405), and `/index.php/...` is now refused (404) by the web server, as
Symfony's nginx location refuses it.

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
