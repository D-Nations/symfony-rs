//! Port of Symfony\Component\HttpFoundation\Session\SessionUtils.php:
//! takes the session's own `Set-Cookie` header out of the headers sent so
//! far.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/SessionUtils.php

use php_value::{PhpString, url::urlencode};

use crate::php_session::{header, header_remove, headers_list};

/// `strpos($haystack, $needle, $offset) === $at`.
fn found_at(haystack: &[u8], needle: &[u8], offset: usize, at: usize) -> bool {
    haystack
        .get(offset..)
        .and_then(|rest| rest.windows(needle.len()).position(|w| w == needle))
        .is_some_and(|i| i + offset == at)
}

/// `SessionUtils::popSessionCookie($sessionName, $sessionId)`: the last
/// `Set-Cookie` header for the session's name, removed with every other
/// one for that name and id; the others are sent again. `None` when there
/// is none.
pub fn pop_session_cookie(session_name: &[u8], session_id: &[u8]) -> Option<PhpString> {
    let prefix = [b" ", &urlencode(session_name)[..], b"="].concat();
    let with_id = [&prefix[..], &urlencode(session_id), b";"].concat();
    let mut session_cookie = None;
    let mut other_cookies = Vec::new();
    for h in headers_list() {
        let bytes = h.as_bytes();
        if !bytes
            .get(..11)
            .is_some_and(|start| start.eq_ignore_ascii_case(b"Set-Cookie:"))
        {
            continue;
        }
        if found_at(bytes, &prefix, 11, 11) {
            if !found_at(bytes, &with_id, 11, 11) {
                other_cookies.push(h.clone());
            }
            session_cookie = Some(h);
        } else {
            other_cookies.push(h);
        }
    }
    let session_cookie = session_cookie?;
    header_remove("Set-Cookie");
    for h in other_cookies {
        header(h, false);
    }
    Some(session_cookie)
}
