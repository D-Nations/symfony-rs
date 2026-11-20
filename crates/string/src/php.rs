//! PHP's native `wordwrap($string, $width, $break, $cut_long_words)`, as
//! symfony/string's `AbstractString::wordwrap()` calls it: on a mask of
//! ASCII bytes (` ` for a space, `?` for any other grapheme, the break for
//! an existing break), with a one-byte break.
//!
//! Written from the PHP manual (no php-src, PLAN.md decision 8) and checked
//! against 4,005 recorded calls (`tests/data/wordwrap.json`,
//! `scripts/record.sh string`). The manual: the string is wrapped at
//! `width` characters using `break`; with `cut_long_words`, words longer
//! than `width` are split, so no line is longer than `width`. As observed:
//! breaks already in the string start a new line; a line is filled
//! greedily, word by word, a space becoming the break where the next word
//! no longer fits; consecutive spaces are words of length zero.
//!
//! Upstream: none

/// `wordwrap($text, $width, $break, $cut)` for a one-byte `brk`. PHP
/// refuses `$cut` with a width of 0 (`ValueError`); callers never pass it.
pub fn wordwrap(text: &[u8], width: usize, brk: u8, cut: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + text.len() / width.max(1));
    for (index, segment) in text.split(|&b| b == brk).enumerate() {
        if index > 0 {
            out.push(brk);
        }
        wrap_segment(segment, width, brk, cut, &mut out);
    }
    out
}

fn wrap_segment(segment: &[u8], width: usize, brk: u8, cut: bool, out: &mut Vec<u8>) {
    let mut line_length = 0;
    for (index, word) in segment.split(|&b| b == b' ').enumerate() {
        if index > 0 {
            // A space that starts a line is kept: a break never makes an
            // empty line.
            if line_length == 0 || line_length + 1 + word.len() <= width {
                out.push(b' ');
                line_length += 1;
            } else {
                out.push(brk);
                line_length = 0;
            }
        }
        let mut word = word;
        if cut {
            while line_length + word.len() > width && word.len() > width - line_length.min(width) {
                let room = width - line_length;
                if room == 0 {
                    out.push(brk);
                    line_length = 0;
                    continue;
                }
                out.extend_from_slice(&word[..room]);
                out.push(brk);
                word = &word[room..];
                line_length = 0;
            }
        }
        out.extend_from_slice(word);
        line_length += word.len();
    }
}
