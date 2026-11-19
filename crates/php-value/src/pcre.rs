//! Character classes of PHP's PCRE under the `/u` modifier, as recorded
//! from PHP 8.4.27 (every code point tested; tests/data/php.json).
//!
//! Upstream: none

/// `\s` with `/u`: ASCII whitespace, NEL, the Z categories and U+180E.
pub fn is_space_u(c: char) -> bool {
    matches!(
        c,
        '\t'..='\r'
            | ' '
            | '\u{85}'
            | '\u{a0}'
            | '\u{1680}'
            | '\u{180e}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
    )
}

/// `\p{Zs}` with `/u`.
pub fn is_zs_u(c: char) -> bool {
    matches!(
        c,
        ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}
