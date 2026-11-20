"""The multipart/form-data bodies recorded from PHP for
symfony-http-foundation's multipart parser (tests/data/multipart_vectors.json).

Moved from symfony-demo-rs (compare/multipart.py: the login-form cases,
which that repository still sends over HTTP; compare/multipart_vectors.py:
the cases only the vectors need). Standard library only.
"""

from __future__ import annotations


B = "XyZb0und"
CT = f"multipart/form-data; boundary={B}"


def part(name: str, value: str, extra: str = "") -> str:
    return f"--{B}\r\nContent-Disposition: form-data; name=\"{name}\"{extra}\r\n\r\n{value}\r\n"


def login(username: str = "marker") -> str:
    return (part("_username", username) + part("_password", "wrong")
            + part("_csrf_token", "csrf-token"))


def rest() -> str:
    return part("_password", "wrong") + part("_csrf_token", "csrf-token")


END = f"--{B}--\r\n"


def cases() -> list[tuple[str, str, bytes]]:
    """(name, Content-Type, body)"""
    found = []

    def add(name: str, body: str | bytes, content_type: str = CT) -> None:
        found.append((name, content_type, body if isinstance(body, bytes) else body.encode()))

    add("standard", login() + END)
    add("lf-only", (login() + END).replace("\r\n", "\n"))
    add("preamble-epilogue", "preamble\r\n" + login() + END + "epilogue")
    add("no-closing-boundary", login())
    add("quoted-boundary", login() + END, f'multipart/form-data; boundary="{B}"; charset=utf-8')
    add("uppercase-type", login() + END, f"MULTIPART/FORM-DATA; BOUNDARY={B}")
    add("boundary-then-comma", login() + END, f"multipart/form-data; boundary={B}, x")
    add("no-boundary", login() + END, "multipart/form-data")
    add("unclosed-quoted-boundary", login() + END, f'multipart/form-data; boundary="{B}')
    add("not-multipart", login() + END, f"multipart/form-datax; boundary={B}")
    add("unquoted-name", f"--{B}\r\nContent-Disposition: form-data; name=_username\r\n\r\nmarker\r\n" + rest() + END)
    add("single-quoted-name", f"--{B}\r\nContent-Disposition: form-data; name='_username'\r\n\r\nmarker\r\n" + rest() + END)
    add("name-spaces", f"--{B}\r\nContent-Disposition: form-data ;  name = \"_username\" \r\n\r\nmarker\r\n" + rest() + END)
    add("uppercase-header", f"--{B}\r\nCONTENT-DISPOSITION: FORM-DATA; NAME=\"_username\"\r\n\r\nmarker\r\n" + rest() + END)
    add("continued-header", f"--{B}\r\nContent-Disposition: form-data;\r\n name=\"_username\"\r\n\r\nmarker\r\n" + rest() + END)
    add("duplicate-disposition", f"--{B}\r\nContent-Disposition: form-data; name=\"other\"\r\n"
        f"Content-Disposition: form-data; name=\"_username\"\r\n\r\nmarker\r\n" + rest() + END)
    add("name-twice", f"--{B}\r\nContent-Disposition: form-data; name=\"other\"; name=\"_username\"\r\n\r\nmarker\r\n" + rest() + END)
    add("part-content-type", part("_username", "marker", "") .replace("\r\n\r\n", "\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n", 1) + rest() + END)
    add("filename", part("_username", "marker", '; filename="u.txt"') + rest() + END)
    add("empty-filename-before", part("file", "", '; filename=""') + login() + END)
    add("file-shadows-field", part("_username", "data", '; filename="u.txt"') + login() + END)
    add("file-starting-with-boundary", part("file", f"--{B}\r\nContent-Disposition: form-data; name=\"_username\"\r\n\r\nhidden", '; filename="f.txt"') + login("marker") + END)
    add("text-starting-with-boundary", part("pad", f"--{B}\r\nContent-Disposition: form-data; name=\"_username\"\r\n\r\nfrom-inside") + rest() + END)
    add("garbled-part", part("_username", "first") + f"--{B}\r\nContent-Disposition: form-data\r\n\r\nx\r\n" + part("_username", "second") + rest() + END)
    add("no-disposition", f"--{B}\r\nContent-Type: text/plain\r\n\r\nx\r\n" + login() + END)
    add("value-with-newline", login("line1\r\nline2") + END)
    add("value-ending-cr", login("marker\r") + END)
    add("username-array", part("_username[]", "marker") + rest() + END)
    add("nul-in-header", f"--{B}\r\nContent-Disposition: form-data; name=\"_user\0name\"\r\n\r\nmarker\r\n" + rest() + END)
    add("nul-in-value", login("mark\0er") + END)
    add("dotted-name", part("_user.name", "dotted") + login() + END)
    # A boundary split across PHP's 5 KiB buffer, at several offsets.
    for pad in (5050, 5060, 5070, 5075, 5080, 5085, 5090, 5100):
        add(f"pad-{pad}", part("pad", "p" * pad) + login() + END)
    add("long-value", part("pad", "q" * 20000) + login() + END)
    add("long-header-line", part("_username", "marker", "; x=" + "h" * 6000) + rest() + END)
    add("long-header-then-name", f"--{B}\r\nX-Long: {'h' * 6000}\r\nContent-Disposition: form-data; name=\"_username\"\r\n\r\nmarker\r\n" + rest() + END)
    # max_input_vars: the 1001st variable is dropped.
    add("vars-1000-then-username", "".join(part(f"v{i}", "x") for i in range(997)) + login() + END)
    add("vars-1001-then-username", "".join(part(f"v{i}", "x") for i in range(998)) + login() + END)
    # max_file_uploads: 21 files, then the fields.
    add("21-files", "".join(part(f"f{i}", "d", f'; filename="{i}.txt"') for i in range(21)) + login() + END)
    add("max-file-size-negative", part("MAX_FILE_SIZE", "-1") + part("file", f"--{B}x\r\nContent-Disposition: form-data; name=\"_username\"\r\n\r\ninside", '; filename="f.txt"') + login() + END)
    return found


def extra_cases() -> list[tuple[str, str, bytes]]:
    """Cases that only the vectors need (the login page cannot show them)."""
    found = []

    def add(name: str, body: str | bytes, content_type: str = CT) -> None:
        found.append((name, content_type, body if isinstance(body, bytes) else body.encode()))

    add("file-then-same-text", part("f", "data", '; filename="u.txt"') + part("f", "shadow") + END)
    add("file-then-tmp-name", part("f", "first") + part("f", "data", '; filename="u.txt"')
        + part("f[tmp_name]", "t") + part("g", "x") + END)
    add("array-file", part("f[a]", "data", '; filename="u.txt"') + part("f[a]", "text") + END)
    add("arrays", part("a[]", "1") + part("a[]", "2") + part("a[k][]", "3") + part("b[x", "4") + END)
    add("escaped-name", f'--{B}\r\nContent-Disposition: form-data; name="a\\"b\\\\c"\r\n\r\nv\r\n' + END)
    add("unquoted-escape", f"--{B}\r\nContent-Disposition: form-data; name=a\\\\b c\r\n\r\nv\r\n" + END)
    add("name-equals-twice", f'--{B}\r\nContent-Disposition: form-data; name==="x"\r\n\r\nv\r\n' + END)
    add("semicolon-in-quotes", f'--{B}\r\nContent-Disposition: form-data; name="a;b"\r\n\r\nv\r\n' + END)
    add("empty-name", part("", "v") + part("ok", "1") + END)
    add("space-name", part(" ", "v") + part(" lead", "2") + END)
    add("empty-body", b"")
    add("only-boundary", f"--{B}\r\n")
    add("boundary-with-space", f"--{B} \r\n" + part("a", "1")[len(f"--{B}\r\n"):] + END)
    add("bad-brackets-file", part("f]", "d", '; filename="x"') + part("a", "1") + END)
    add("filename-no-name", f'--{B}\r\nContent-Disposition: form-data; filename="x"\r\n\r\nd\r\n' + part("a", "1") + END)
    add("invalid-utf8", part("a", "") .replace("\r\n\r\n", "\r\n\r\n\xff") .encode("latin-1") + END.encode())
    for pad in (5098, 5099, 5101, 5102, 5103, 5104, 5105, 5106, 5107, 5108, 5110, 5112):
        add(f"pad-{pad}", part("pad", "p" * pad) + part("a", "1") + END)
    add("long-name-line", part("n" * 6000, "v") + part("a", "1") + END)
    add("cr-before-boundary-at-fill", part("pad", "p" * 5105 + "\r") + part("a", "1") + END)
    add("max-file-size", part("MAX_FILE_SIZE", "3") + part("f", "toolong\r\n--XyZb0und", '; filename="f"') + part("a", "1") + END)
    add("boundary-too-large", "x", "multipart/form-data; boundary=" + "b" * 5200)
    return found


def rewrite_cases() -> list[tuple[str, str, bytes]]:
    """Cases added for the rewrite without php-src (PLAN.md decision 8):
    behaviour the earlier cases leave open."""
    found = []

    def add(name: str, body: str | bytes, content_type: str = CT) -> None:
        found.append((name, content_type, body if isinstance(body, bytes) else body.encode("latin-1")))

    end = END
    # What may follow a delimiter on its line, inside and after a value.
    add("delimiter-then-text", part("a", f"x\r\n--{B}yz\r\nmore") + part("b", "2") + end)
    add("delimiter-then-space", part("a", "1")[:-2] + f"\r\n--{B}  \r\nContent-Disposition: form-data; name=\"b\"\r\n\r\n2\r\n" + end)
    add("delimiter-then-tab", part("a", "1") + f"--{B}\t\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\n2\r\n" + end)
    add("close-then-parts", part("a", "1") + f"--{B}--\r\n" + part("b", "2") + end)
    add("close-then-text", part("a", "1") + f"--{B}--junk\r\n" + part("b", "2") + end)
    add("dashes-in-value", part("a", f"--{B[:-1]}\r\n--x") + part("b", "2") + end)
    add("delimiter-without-newline-before", part("a", f"x--{B}\r\ny") + end)
    add("delimiter-after-cr-only", part("a", f"x\r--{B}\r\ny") + end)
    # The first delimiter.
    add("preamble-delimiter-prefix", f"--{B}x\r\n" + part("a", "1") + end)
    add("preamble-delimiter-close", f"--{B}--\r\n" + part("a", "1") + end)
    add("leading-crlf", "\r\n" + part("a", "1") + end)
    add("first-delimiter-with-space", f"--{B} \r\n--{B}\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n" + end)
    # Headers.
    add("header-without-colon", f"--{B}\r\nnot a header\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n" + end)
    add("header-no-space", f"--{B}\r\ncontent-disposition:form-data;name=a\r\n\r\n1\r\n" + end)
    add("header-tab-continuation", f"--{B}\r\nContent-Disposition: form-data;\r\n\tname=\"a\"\r\n\r\n1\r\n" + end)
    add("folded-name", f"--{B}\r\nContent-Disposition: form-data; name=\"a\r\n b\"\r\n\r\n1\r\n" + end)
    add("folded-unquoted-name", f"--{B}\r\nContent-Disposition: form-data; name=a\r\n b\r\n\r\n1\r\n" + end)
    add("disposition-not-form-data", f"--{B}\r\nContent-Disposition: attachment; name=\"a\"\r\n\r\n1\r\n" + end)
    add("disposition-only-value", f"--{B}\r\nContent-Disposition: name=\"a\"\r\n\r\n1\r\n" + end)
    add("disposition-space-before-colon", f"--{B}\r\nContent-Disposition : form-data; name=\"a\"\r\n\r\n1\r\n" + end)
    add("eof-in-headers", f"--{B}\r\nContent-Disposition: form-data; name=\"a\"\r\n")
    add("eof-after-headers", f"--{B}\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n")
    # Parameters.
    add("param-uppercase-name", f"--{B}\r\nContent-Disposition: form-data; NaMe=\"a\"\r\n\r\n1\r\n" + end)
    add("param-name-star", f"--{B}\r\nContent-Disposition: form-data; name*=\"a\"; name=\"b\"\r\n\r\n1\r\n" + end)
    add("param-unclosed-quote", f"--{B}\r\nContent-Disposition: form-data; name=\"abc\r\n\r\n1\r\n" + end)
    add("param-empty-unquoted", f"--{B}\r\nContent-Disposition: form-data; name=; x=1\r\n\r\n1\r\n" + part("b", "2") + end)
    add("param-unquoted-semicolon", f"--{B}\r\nContent-Disposition: form-data; name=a;b\r\n\r\n1\r\n" + end)
    add("param-unquoted-tab", f"--{B}\r\nContent-Disposition: form-data; name=a\tb\r\n\r\n1\r\n" + end)
    add("param-quote-inside-unquoted", f"--{B}\r\nContent-Disposition: form-data; name=a\"b\"c\r\n\r\n1\r\n" + end)
    add("param-mixed-quotes", f"--{B}\r\nContent-Disposition: form-data; name=\"a'b\"\r\n\r\n1\r\n" + end)
    add("param-escape-other", f"--{B}\r\nContent-Disposition: form-data; name=\"a\\nb\"\r\n\r\n1\r\n" + end)
    add("param-text-after-quote", f"--{B}\r\nContent-Disposition: form-data; name=\"a\"b; x=1\r\n\r\n1\r\n" + end)
    add("param-leading-tab", f"--{B}\r\nContent-Disposition: form-data;\tname=\"a\"\r\n\r\n1\r\n" + end)
    add("param-no-semicolon", f"--{B}\r\nContent-Disposition: form-data name=\"a\"\r\n\r\n1\r\n" + end)
    add("filename-then-name", f"--{B}\r\nContent-Disposition: form-data; filename=\"f\"; name=\"a\"\r\n\r\nd\r\n" + part("b", "2") + end)
    add("filename-uppercase", f"--{B}\r\nContent-Disposition: form-data; name=\"a\"; FILENAME=\"f\"\r\n\r\nd\r\n" + part("b", "2") + end)
    add("filename-star", f"--{B}\r\nContent-Disposition: form-data; name=\"a\"; filename*=UTF-8''f\r\n\r\nd\r\n" + part("b", "2") + end)
    # End of input inside a value.
    add("eof-value-cr", part("a", "1") + f"--{B}\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\nv\r")
    add("eof-value-two-lf", part("a", "1") + f"--{B}\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\nv\n\n")
    add("eof-value-lfcr", part("a", "1") + f"--{B}\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\nv\n\r")
    # The boundary parameter.
    for i, content_type in enumerate([
        f"multipart/form-data;boundary={B}", f"multipart/form-data; boundary = {B}",
        f"multipart/form-data; boundary={B} x", f"multipart/form-data; xboundary={B}",
        f"multipart/form-data; charset=x; boundary={B}", f"multipart/form-data; boundary={B}; boundary=other",
        "multipart/form-data; boundary=", "multipart/form-data; boundary=\"\"",
        f"multipart/form-data; boundary=\"{B}\"x", f"multipart/form-data; boundary={B}\t",
        f"multipart/form-data, boundary={B}", f"multipart/form-data boundary={B}",
        f" multipart/form-data; boundary={B}", f"multipart/form-data ; boundary={B}",
    ]):
        add(f"boundary-param-{i}", part("a", "1") + end, content_type)
    # Limits: parts (max_multipart_body_parts), files, empty file fields.
    add("parts-1020-no-disposition", f"--{B}\r\nX: y\r\n\r\nz\r\n" * 1019 + part("a", "1") + end)
    add("parts-1021-no-disposition", f"--{B}\r\nX: y\r\n\r\nz\r\n" * 1020 + part("a", "1") + end)
    add("files-20-then-field", "".join(part(f"f{i}", "d", f'; filename="{i}.txt"') for i in range(20)) + part("a", "1") + end)
    add("files-21-empty-names", "".join(part(f"f{i}", "", '; filename=""') for i in range(21)) + part("a", "1") + end)
    add("vars-1000-empty-names", "".join(part("", "x") for _ in range(10)) + "".join(part(f"v{i}", "x") for i in range(995)) + part("last", "1") + end)
    return found


def all_cases() -> list[tuple[str, str, bytes]]:
    return cases() + extra_cases() + rewrite_cases()
