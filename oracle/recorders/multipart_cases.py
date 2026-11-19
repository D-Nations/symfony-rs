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


def all_cases() -> list[tuple[str, str, bytes]]:
    return cases() + extra_cases()
