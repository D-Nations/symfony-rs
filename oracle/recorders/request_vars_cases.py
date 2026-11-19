"""Query strings and Cookie headers recorded from PHP for
symfony-http-foundation's query and cookie parsers
(tests/data/query_vectors.json, tests/data/cookie_vectors.json).

Each query string is sent as the URL's query (when it is valid in a
request line) and as an application/x-www-form-urlencoded POST body.
Standard library only.
"""

from __future__ import annotations


def query_cases() -> list[tuple[str, bytes]]:
    """(name, raw query string)"""
    found: list[tuple[str, bytes]] = []

    def add(name: str, raw: str | bytes) -> None:
        found.append((name, raw if isinstance(raw, bytes) else raw.encode("latin-1")))

    # Pairs and separators.
    for i, raw in enumerate([
        "a=1", "a=1&a=2", "a=1&b=2&a=3", "a", "a=", "=x", "=", "&&a=1&&", "a=1&",
        "a=1;b=2", "a=b=c", "a==b", "&", "",
    ]):
        add(f"pairs-{i}", raw)
    # Decoding.
    for i, raw in enumerate([
        "a=%41", "a=%zz%41", "a=100%", "a=%4", "a=%%41", "a=%4g", "a+b=c+d", "a%20b=1",
        "a=%2B", "a=%00", "a%00b=1", "%00a=1", "a=%FF", "%FF=1", "a=%c3%a9", "a=%E9",
        "a%2eb=1", "a%2Eb=1", "%61=1", "a%3d1=2", "a%26b=1",
    ]):
        add(f"decode-{i}", raw)
    # Characters in names (raw bytes go only in the POST body).
    for i, raw in enumerate([
        "a.b=1", "a b=1", " a=1", "  a=1", "%20%20a=1", "%09a=1", "a%09b=1", "a..b=1",
        ".a=1", "a.=1", "a%80b=1", "a%9Fb=1", "a%A0b=1", "a%7Fb=1", "a%1Fb=1", "+a=1",
        "a+=1", " =1", "a-b=1", "a%5Db=1",
    ]):
        add(f"name-{i}", raw)
    # Brackets.
    for i, raw in enumerate([
        "a[]=1&a[]=2", "a[b]=1", "a[b][c]=1", "a[]=1&a[x]=2&a[]=3", "a[][]=1&a[][]=2",
        "a[=1", "a[b=1", "a[b.c=1", "a.b[c.d]=1", "a[b]c=1", "a[b]c[d]=1", "a[b][=1",
        "a[b]]=1", "a[[b]]=1", "a][=1", "]a=1", "[a]=1", "[]=1", "[=1", "a[ b]=1",
        "a[b ]=1", "a[.]=1", "a[ ]=1", "a [b]=1", "a.[b]=1", "a[b] [c]=1", "a[b][ c]=1",
        "a%5B%5D=1", "a%5Bb%5D=1", "a%5b=1", "a[%5D=1", "a[b%5D=1", "a[]x=1",
        "a[b][]=1&a[b][]=2", "a[x]=1&a=2", "a=2&a[x]=1", "a[x]=1&a[x][y]=2",
        "a[x][y]=2&a[x]=1", "a[]=1&a=2&a[]=3", "a[x]=1&b=2&a[y]=3", "a[\xff]=1",
        "a[%80]=1", "a[b.c]=1", "a[b c]=1", "a[b+c]=1", "a[b[c=1", "a[b c=1",
        "a[b]c.d=1", "a[%09]=1", "a[%0A]=1", "a[%20%20]=1", "a[%09b]=1", "a.b[=1",
        "a[b][c[d]=1", "a%5Bb=1", "a[b]%5Bc%5D=1", "a[%0D]=1", "a[%0B]=1", "a[%0C]=1",
        "a[%20%09]=1", "a[b][ ]=1&a[b][ ]=2", "a[b][%20%20]=1", "a[%00]=1", "a[b%00c]=1",
    ]):
        add(f"bracket-{i}", raw)
    # Array keys and the next index.
    for i, raw in enumerate([
        "a[1]=x&a[]=y", "a[-1]=x&a[]=y", "a[-3]=x&a[]=y", "a[5]=x&a[2]=y&a[]=z",
        "a[9223372036854775807]=x&a[]=y", "a[9223372036854775806]=x&a[]=y&a[]=z",
        "a[9223372036854775808]=x&a[]=y", "a[-9223372036854775808]=x&a[]=y",
        "a[-9223372036854775809]=x&a[]=y", "a[01]=x&a[]=y", "a[00]=x", "a[0]=x&a[00]=y",
        "a[-0]=x&a[]=y", "a[+1]=x&a[]=y", "a[%2B1]=x&a[]=y", "a[0x1]=x&a[]=y",
        "a[1e2]=x&a[]=y", "a[1.5]=x&a[]=y", "a[-]=x&a[]=y", "a[%201]=x&a[]=y",
        "a[1%20]=x&a[]=y", "a[1]=x&a[1]=y", "a[1]=x&a[01]=y", "a[x]=1&a[]=2",
    ]):
        add(f"key-{i}", raw)
    # Nesting depth (max_input_nesting_level).
    for depth in (62, 63, 64, 65, 66):
        add(f"depth-{depth}", "a" + "[x]" * depth + "=1&b=2")
        add(f"depth-append-{depth}", "a" + "[]" * depth + "=1&b=2")
    # max_input_vars.
    add("vars-1000", "&".join(f"v{i}=x" for i in range(1000)))
    add("vars-1001", "&".join(f"v{i}=x" for i in range(1001)))
    add("vars-1000-empty-names", "&".join(["=x"] * 10 + [f"v{i}=x" for i in range(995)] + ["last=1"]))
    add("vars-1000-empty-pairs", "&".join([""] * 10 + [f"v{i}=x" for i in range(995)] + ["last=1"]))
    add("vars-1001-same-name", "&".join(f"a={i}" for i in range(1001)))
    add("vars-1001-arrays", "&".join(f"a[]={i}" for i in range(1001)))
    add("vars-999-then-nested", "&".join(f"v{i}=x" for i in range(999)) + "&n[a]=1&n[b]=2&last=1")
    return found


def cookie_cases() -> list[tuple[str, bytes]]:
    """(name, Cookie header value)"""
    found: list[tuple[str, bytes]] = []

    def add(name: str, raw: str | bytes) -> None:
        found.append((name, raw if isinstance(raw, bytes) else raw.encode("latin-1")))

    for i, raw in enumerate([
        "a=1", "a=1; b=2", "a=1;b=2", "a=1;  b=2", "a=1; a=2", "a=1;a=2;a=3", "a.b=1",
        "a b=1", " a=1", "a%20b=1", "a%2eb=1", "a=%20", "a=+", "a=%2B", "a=1, b=2",
        "a=1,b=2", "a=1\tb=2", "a", "a=", "=1", "a=1;;b=2", "a==b", "a=b=c", "a=\"q\"",
        "a=1 ", "a =1", "a= 1", ";a=1", "a=%FF", "a=%zz", "a[b]=1", "a[]=1; a[]=2",
        "a[b]=1; a=2", "a=2; a[b]=1", "a[b]=1; a[b]=2",
        "PHPSESSID=abc; REMEMBERME=%3Aam9obl91c2Vy%3A1; a.b=1; PHPSESSID=second",
        "a=1;\tb=2", "a=1; ; b=2", "a=%E9", "a=%c3%a9",
    ]):
        add(f"cookie-{i}", raw)
    # Added for the rewrite: names after conversion, tabs, the variable limit.
    # (A raw NUL is left out: the built-in server cuts the header there.)
    for i, raw in enumerate([
        "a.b=1; a_b=2", "a_b=1; a.b=2", "a\tb=1", "\ta=1", "a=1;\t\tb=2", "a[=1",
        "a[b.c=1", "a[ ]=1; a[ ]=2", "[a]=1", " =1", "a+b=1", "a=1;b",
    ]):
        add(f"cookie-more-{i}", raw)
    add("cookie-vars-1001", "; ".join(f"v{i}=x" for i in range(1001)))
    add("cookie-vars-1000-empty", "; ".join([""] * 10 + [f"v{i}=x" for i in range(995)] + ["last=1"]))
    return found


def content_type_cases() -> list[tuple[str, bytes]]:
    """(name, Content-Type sent with the body a=1&b[]=2)"""
    found: list[tuple[str, bytes]] = []
    for i, raw in enumerate([
        "application/x-www-form-urlencoded", "application/x-www-form-urlencoded; charset=utf-8",
        "APPLICATION/X-WWW-FORM-URLENCODED", "application/x-www-form-urlencoded ",
        " application/x-www-form-urlencoded", "application/x-www-form-urlencoded,x",
        "application/x-www-form-urlencodedx", "application/x-www-form-urlencoded\tx",
        "application/x-www-form-urlencoded;x", "application/x-www-form-urlencode",
        "text/plain", "",
    ]):
        found.append((f"type-{i}", raw.encode("latin-1")))
    return found
