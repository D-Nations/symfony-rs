# Symfony Runtime (subset) and the HTTP adapter

Private workspace crate; MIT, with Symfony's source copyright retained in
the implementation. This is an unofficial partial port, not the Runtime
component: it has Symfony Runtime's `HttpKernelRunner` and an Axum adapter
that serves a ported kernel the way nginx in front of PHP-FPM serves a
Symfony front controller. `SymfonyRuntime`, `GenericRuntime`, the
FrankenPHP worker runner and the console runners are not ported.

| Module/API | Source behavior | Evidence | Limits |
|---|---|---|---|
| `http_kernel_runner` | Symfony `HttpKernelRunner` (v8.1.9): `handle()`, `send(false)`, then `fastcgi_finish_request()` or `litespeed_finish_request()` where the SAPI has it and not in debug mode, else `Response::closeOutputBuffers(0, true)` and `flush()`; `terminate()` for a terminable kernel; `RunnableKernel` for `instanceof TerminableInterface` | Runner tests for each SAPI and debug mode (`tests/server.rs`) | The runtime's own tests (`SymfonyRuntimeTest`, `FrankenPhpWorkerRunnerTest`) cover classes not ported; the manifest rows stay pending |
| `server::fastcgi_params` | nginx's `fastcgi_params` with Symfony's documented location: the parameters in order, `SCRIPT_FILENAME` and `DOCUMENT_ROOT` from the root, headers as `HTTP_*` (names with `_` dropped, `Proxy` dropped, repeats joined with `, ` and `Cookie` with `; ` as nginx 1.23+), `CONTENT_LENGTH` of a buffered chunked body | `tests/server.rs`; the PHP side of these parameters is recorded (`symfony-http-foundation`'s `tests/data/sapi.json`) | Written from nginx's documentation, not recorded from nginx; `REQUEST_URI` is the request target's path and query; no static files (`try_files` finds none); `/index.php`, `/index.php/...` and `*.php` are 404 (empty body, where nginx sends its HTML page); bodies over `client_max_body_size` are 413 |
| `server::into_response` | nginx passing PHP-FPM's CGI response on: `Status:` gives the status (502 outside 100–999), other lines become headers, no body for HEAD | `tests/server.rs` | The reason phrase is the `http` crate's canonical one (PHP-FPM's `Status: 299 Custom Text` text is lost); a line `http` cannot carry (no colon, a name that is not a token, a control byte in the value) is dropped and logged, where nginx answers 502 for some; header names are lowercased |
| `server::FrontController` | One PHP request per HTTP request on a blocking thread (a PHP-FPM worker): superglobals set, a bootstrap hook (trusted proxies and hosts), `Request::createFromGlobals()`, a new kernel from the factory, `HttpKernelRunner`; the output goes to the client as nginx passes PHP-FPM's on (the head once the headers are sent; the body at each `flush()` with `X-Accel-Buffering: no`, else in 4 KiB pieces and the rest at the end, chunked without `Content-Length`) and ends at `fastcgi_finish_request()` while `terminate()` runs; nginx's hidden `X-Accel-*` headers are not passed on; once the client is gone, `connection_aborted()` answers true; afterwards the superglobals, uploaded files and `Request`'s statics are cleared; an uncaught throwable is logged and answered 500 | `tests/server.rs` (early response, 500, refused paths, uploaded file removal, a `StreamedResponse`, flushes passed on with and without buffering, a full buffer, hidden `X-Accel-*` headers); request-lab's HTTP tests | Warnings are written to stderr, never to the body (`display_errors` off); `expose_php` is off (`Ini::x_powered_by`); php.ini settings are `sapi::Ini`'s defaults (PHP's). Where nginx cuts buffered output is approximated (one 4 KiB page per piece), and PHP-FPM's own 8 KiB write buffer between flushes is not modelled. A response that ends before any `flush()` is sent whole with a `Content-Length` axum computes, where nginx would send it chunked. `X-Accel-Redirect`, `-Expires`, `-Limit-Rate` and `-Charset` are hidden but not applied (logged) |

`set_finish_hook()` and `finish_request()` are the per-thread
`fastcgi_finish_request()`; the adapter sets the hook for each request.

MIT; see the repository LICENSE. This crate is not an official Symfony
project.
