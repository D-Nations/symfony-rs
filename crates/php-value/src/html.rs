//! `html_entity_decode($string)` with PHP's default flags
//! (`ENT_QUOTES | ENT_SUBSTITUTE | ENT_HTML401`, UTF-8), written from the
//! PHP manual and PHP 8.4.27's recorded answers
//! (`tests/data/html_entity_decode.json`,
//! `oracle/recorders/php_value_html_entity_decode.php`):
//!
//! - a named reference is one of the 252 HTML 4.01 entities, matched case
//!   sensitively and only with its `;` (`&apos;` is not one of them);
//! - a numeric reference is `&#` and decimal digits, or `&#x`/`&#X` and hex
//!   digits, any number of them, then `;`; it is decoded only for U+0009,
//!   U+000A, U+000D, U+0020–U+007E, U+00A0–U+D7FF and U+E000–U+10FFFF;
//! - anything else, invalid UTF-8 included, is kept as it is, and decoding
//!   is one pass (`&amp;lt;` gives `&lt;`).
//!
//! Upstream: none

/// `html_entity_decode($string)`.
pub fn html_entity_decode(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'&'
            && let Some((c, len)) = reference(&s[i + 1..])
        {
            let mut buf = [0; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            i += 1 + len;
            continue;
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

/// The character a reference after `&` stands for, and the reference's
/// length up to and including its `;`.
fn reference(s: &[u8]) -> Option<(char, usize)> {
    // a reference is letters, digits and `#`; stopping at anything else
    // keeps a run of `&` linear
    let end = s
        .iter()
        .position(|&c| !(c.is_ascii_alphanumeric() || c == b'#'))?;
    if s[end] != b';' {
        return None;
    }
    let body = &s[..end];
    let code = if let Some(number) = body.strip_prefix(b"#") {
        let (digits, radix) = match number {
            [b'x' | b'X', hex @ ..] => (hex, 16),
            decimal => (decimal, 10),
        };
        if digits.is_empty() || !digits.iter().all(|c| char::from(*c).is_digit(radix)) {
            return None;
        }
        let mut value: u32 = 0;
        for &d in digits {
            value = value
                .checked_mul(radix)?
                .checked_add(char::from(d).to_digit(radix)?)?;
            if value > 0x10_FFFF {
                return None;
            }
        }
        if !allowed(value) {
            return None;
        }
        value
    } else {
        let index = ENTITIES
            .binary_search_by(|(name, _)| (*name).cmp(body))
            .ok()?;
        ENTITIES[index].1
    };
    Some((char::from_u32(code)?, end + 1))
}

/// Code points a numeric reference may stand for under `ENT_HTML401`.
const fn allowed(c: u32) -> bool {
    matches!(c, 0x09 | 0x0A | 0x0D | 0x20..=0x7E | 0xA0..=0xD7FF | 0xE000..=0x10_FFFF)
}

/// The HTML 4.01 entities, sorted by name, as PHP 8.4.27's
/// `get_html_translation_table(HTML_ENTITIES, ENT_QUOTES | ENT_HTML401)`
/// lists them (its 253rd entry, `'` as `&#039;`, is numeric).
const ENTITIES: [(&[u8], u32); 252] = [
    (b"AElig", 0x00C6),
    (b"Aacute", 0x00C1),
    (b"Acirc", 0x00C2),
    (b"Agrave", 0x00C0),
    (b"Alpha", 0x0391),
    (b"Aring", 0x00C5),
    (b"Atilde", 0x00C3),
    (b"Auml", 0x00C4),
    (b"Beta", 0x0392),
    (b"Ccedil", 0x00C7),
    (b"Chi", 0x03A7),
    (b"Dagger", 0x2021),
    (b"Delta", 0x0394),
    (b"ETH", 0x00D0),
    (b"Eacute", 0x00C9),
    (b"Ecirc", 0x00CA),
    (b"Egrave", 0x00C8),
    (b"Epsilon", 0x0395),
    (b"Eta", 0x0397),
    (b"Euml", 0x00CB),
    (b"Gamma", 0x0393),
    (b"Iacute", 0x00CD),
    (b"Icirc", 0x00CE),
    (b"Igrave", 0x00CC),
    (b"Iota", 0x0399),
    (b"Iuml", 0x00CF),
    (b"Kappa", 0x039A),
    (b"Lambda", 0x039B),
    (b"Mu", 0x039C),
    (b"Ntilde", 0x00D1),
    (b"Nu", 0x039D),
    (b"OElig", 0x0152),
    (b"Oacute", 0x00D3),
    (b"Ocirc", 0x00D4),
    (b"Ograve", 0x00D2),
    (b"Omega", 0x03A9),
    (b"Omicron", 0x039F),
    (b"Oslash", 0x00D8),
    (b"Otilde", 0x00D5),
    (b"Ouml", 0x00D6),
    (b"Phi", 0x03A6),
    (b"Pi", 0x03A0),
    (b"Prime", 0x2033),
    (b"Psi", 0x03A8),
    (b"Rho", 0x03A1),
    (b"Scaron", 0x0160),
    (b"Sigma", 0x03A3),
    (b"THORN", 0x00DE),
    (b"Tau", 0x03A4),
    (b"Theta", 0x0398),
    (b"Uacute", 0x00DA),
    (b"Ucirc", 0x00DB),
    (b"Ugrave", 0x00D9),
    (b"Upsilon", 0x03A5),
    (b"Uuml", 0x00DC),
    (b"Xi", 0x039E),
    (b"Yacute", 0x00DD),
    (b"Yuml", 0x0178),
    (b"Zeta", 0x0396),
    (b"aacute", 0x00E1),
    (b"acirc", 0x00E2),
    (b"acute", 0x00B4),
    (b"aelig", 0x00E6),
    (b"agrave", 0x00E0),
    (b"alefsym", 0x2135),
    (b"alpha", 0x03B1),
    (b"amp", 0x0026),
    (b"and", 0x2227),
    (b"ang", 0x2220),
    (b"aring", 0x00E5),
    (b"asymp", 0x2248),
    (b"atilde", 0x00E3),
    (b"auml", 0x00E4),
    (b"bdquo", 0x201E),
    (b"beta", 0x03B2),
    (b"brvbar", 0x00A6),
    (b"bull", 0x2022),
    (b"cap", 0x2229),
    (b"ccedil", 0x00E7),
    (b"cedil", 0x00B8),
    (b"cent", 0x00A2),
    (b"chi", 0x03C7),
    (b"circ", 0x02C6),
    (b"clubs", 0x2663),
    (b"cong", 0x2245),
    (b"copy", 0x00A9),
    (b"crarr", 0x21B5),
    (b"cup", 0x222A),
    (b"curren", 0x00A4),
    (b"dArr", 0x21D3),
    (b"dagger", 0x2020),
    (b"darr", 0x2193),
    (b"deg", 0x00B0),
    (b"delta", 0x03B4),
    (b"diams", 0x2666),
    (b"divide", 0x00F7),
    (b"eacute", 0x00E9),
    (b"ecirc", 0x00EA),
    (b"egrave", 0x00E8),
    (b"empty", 0x2205),
    (b"emsp", 0x2003),
    (b"ensp", 0x2002),
    (b"epsilon", 0x03B5),
    (b"equiv", 0x2261),
    (b"eta", 0x03B7),
    (b"eth", 0x00F0),
    (b"euml", 0x00EB),
    (b"euro", 0x20AC),
    (b"exist", 0x2203),
    (b"fnof", 0x0192),
    (b"forall", 0x2200),
    (b"frac12", 0x00BD),
    (b"frac14", 0x00BC),
    (b"frac34", 0x00BE),
    (b"frasl", 0x2044),
    (b"gamma", 0x03B3),
    (b"ge", 0x2265),
    (b"gt", 0x003E),
    (b"hArr", 0x21D4),
    (b"harr", 0x2194),
    (b"hearts", 0x2665),
    (b"hellip", 0x2026),
    (b"iacute", 0x00ED),
    (b"icirc", 0x00EE),
    (b"iexcl", 0x00A1),
    (b"igrave", 0x00EC),
    (b"image", 0x2111),
    (b"infin", 0x221E),
    (b"int", 0x222B),
    (b"iota", 0x03B9),
    (b"iquest", 0x00BF),
    (b"isin", 0x2208),
    (b"iuml", 0x00EF),
    (b"kappa", 0x03BA),
    (b"lArr", 0x21D0),
    (b"lambda", 0x03BB),
    (b"lang", 0x2329),
    (b"laquo", 0x00AB),
    (b"larr", 0x2190),
    (b"lceil", 0x2308),
    (b"ldquo", 0x201C),
    (b"le", 0x2264),
    (b"lfloor", 0x230A),
    (b"lowast", 0x2217),
    (b"loz", 0x25CA),
    (b"lrm", 0x200E),
    (b"lsaquo", 0x2039),
    (b"lsquo", 0x2018),
    (b"lt", 0x003C),
    (b"macr", 0x00AF),
    (b"mdash", 0x2014),
    (b"micro", 0x00B5),
    (b"middot", 0x00B7),
    (b"minus", 0x2212),
    (b"mu", 0x03BC),
    (b"nabla", 0x2207),
    (b"nbsp", 0x00A0),
    (b"ndash", 0x2013),
    (b"ne", 0x2260),
    (b"ni", 0x220B),
    (b"not", 0x00AC),
    (b"notin", 0x2209),
    (b"nsub", 0x2284),
    (b"ntilde", 0x00F1),
    (b"nu", 0x03BD),
    (b"oacute", 0x00F3),
    (b"ocirc", 0x00F4),
    (b"oelig", 0x0153),
    (b"ograve", 0x00F2),
    (b"oline", 0x203E),
    (b"omega", 0x03C9),
    (b"omicron", 0x03BF),
    (b"oplus", 0x2295),
    (b"or", 0x2228),
    (b"ordf", 0x00AA),
    (b"ordm", 0x00BA),
    (b"oslash", 0x00F8),
    (b"otilde", 0x00F5),
    (b"otimes", 0x2297),
    (b"ouml", 0x00F6),
    (b"para", 0x00B6),
    (b"part", 0x2202),
    (b"permil", 0x2030),
    (b"perp", 0x22A5),
    (b"phi", 0x03C6),
    (b"pi", 0x03C0),
    (b"piv", 0x03D6),
    (b"plusmn", 0x00B1),
    (b"pound", 0x00A3),
    (b"prime", 0x2032),
    (b"prod", 0x220F),
    (b"prop", 0x221D),
    (b"psi", 0x03C8),
    (b"quot", 0x0022),
    (b"rArr", 0x21D2),
    (b"radic", 0x221A),
    (b"rang", 0x232A),
    (b"raquo", 0x00BB),
    (b"rarr", 0x2192),
    (b"rceil", 0x2309),
    (b"rdquo", 0x201D),
    (b"real", 0x211C),
    (b"reg", 0x00AE),
    (b"rfloor", 0x230B),
    (b"rho", 0x03C1),
    (b"rlm", 0x200F),
    (b"rsaquo", 0x203A),
    (b"rsquo", 0x2019),
    (b"sbquo", 0x201A),
    (b"scaron", 0x0161),
    (b"sdot", 0x22C5),
    (b"sect", 0x00A7),
    (b"shy", 0x00AD),
    (b"sigma", 0x03C3),
    (b"sigmaf", 0x03C2),
    (b"sim", 0x223C),
    (b"spades", 0x2660),
    (b"sub", 0x2282),
    (b"sube", 0x2286),
    (b"sum", 0x2211),
    (b"sup", 0x2283),
    (b"sup1", 0x00B9),
    (b"sup2", 0x00B2),
    (b"sup3", 0x00B3),
    (b"supe", 0x2287),
    (b"szlig", 0x00DF),
    (b"tau", 0x03C4),
    (b"there4", 0x2234),
    (b"theta", 0x03B8),
    (b"thetasym", 0x03D1),
    (b"thinsp", 0x2009),
    (b"thorn", 0x00FE),
    (b"tilde", 0x02DC),
    (b"times", 0x00D7),
    (b"trade", 0x2122),
    (b"uArr", 0x21D1),
    (b"uacute", 0x00FA),
    (b"uarr", 0x2191),
    (b"ucirc", 0x00FB),
    (b"ugrave", 0x00F9),
    (b"uml", 0x00A8),
    (b"upsih", 0x03D2),
    (b"upsilon", 0x03C5),
    (b"uuml", 0x00FC),
    (b"weierp", 0x2118),
    (b"xi", 0x03BE),
    (b"yacute", 0x00FD),
    (b"yen", 0x00A5),
    (b"yuml", 0x00FF),
    (b"zeta", 0x03B6),
    (b"zwj", 0x200D),
    (b"zwnj", 0x200C),
];
