/// Truncate and scrub response bodies before showing them in the terminal.
pub fn redact_error_body(body: &str) -> String {
    const MAX: usize = 240;
    let lower = body.to_ascii_lowercase();
    for needle in [
        "access_token",
        "refresh_token",
        "id_token",
        "client_secret",
        "authorization",
        "bearer ",
    ] {
        if lower.contains(needle) {
            return format!("[redacted response containing `{needle}`]");
        }
    }

    let scrubbed = scrub_token_shaped(body);

    // Collapse whitespace for single-line errors.
    let flat: String = scrubbed
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if flat.chars().count() > MAX {
        let head: String = flat.chars().take(MAX).collect();
        format!("{head}…")
    } else {
        flat
    }
}

/// Redact JWT-shaped strings and long opaque tokens that appear without JSON keys.
fn scrub_token_shaped(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if looks_like_jwt_at(bytes, i) {
            out.push_str("<redacted-jwt>");
            i = skip_jwt(bytes, i);
            continue;
        }
        if looks_like_opaque_token_at(bytes, i) {
            out.push_str("<redacted-token>");
            i = skip_token_chars(bytes, i);
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn is_b64url(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'=')
}

fn looks_like_jwt_at(bytes: &[u8], i: usize) -> bool {
    // Compact JWT: eyJ… . … . …
    if i + 8 > bytes.len() {
        return false;
    }
    if !(bytes[i] == b'e' && bytes[i + 1] == b'y' && bytes[i + 2] == b'J') {
        return false;
    }
    let mut dots = 0;
    let mut j = i;
    while j < bytes.len() && (is_b64url(bytes[j]) || bytes[j] == b'.') {
        if bytes[j] == b'.' {
            dots += 1;
        }
        j += 1;
        if j - i > 4096 {
            break;
        }
    }
    dots >= 2 && (j - i) >= 20
}

fn skip_jwt(bytes: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < bytes.len() && (is_b64url(bytes[j]) || bytes[j] == b'.') {
        j += 1;
        if j - i > 8192 {
            break;
        }
    }
    j
}

fn looks_like_opaque_token_at(bytes: &[u8], i: usize) -> bool {
    // Long high-entropy-looking runs (avoid short IDs).
    const MIN: usize = 40;
    if i > 0 {
        let prev = bytes[i - 1];
        if is_b64url(prev) {
            return false; // mid-token
        }
    }
    let mut j = i;
    while j < bytes.len() && is_b64url(bytes[j]) {
        j += 1;
        if j - i > 512 {
            break;
        }
    }
    let len = j - i;
    if len < MIN {
        return false;
    }
    // Prefer runs that look token-like (mixed case or includes -/_).
    let slice = &bytes[i..j];
    let has_sep = slice.iter().any(|b| *b == b'-' || *b == b'_');
    let has_upper = slice.iter().any(|b| b.is_ascii_uppercase());
    let has_lower = slice.iter().any(|b| b.is_ascii_lowercase());
    let has_digit = slice.iter().any(|b| b.is_ascii_digit());
    (has_sep || (has_upper && has_lower)) && has_digit
}

fn skip_token_chars(bytes: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < bytes.len() && is_b64url(bytes[j]) {
        j += 1;
        if j - i > 512 {
            break;
        }
    }
    j
}

/// Printable authorize URL without sensitive query parameters.
pub fn authorize_url_for_display(url: &url::Url) -> String {
    let mut safe = url.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    format!(
        "{} (query parameters omitted — open the browser prompt or re-run login)",
        safe.as_str().trim_end_matches('?')
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn redacts_token_bearing_bodies() {
        let out = redact_error_body(r#"{"access_token":"secret"}"#);
        assert!(out.contains("redacted"));
        assert!(!out.contains("secret"));
    }

    #[test]
    fn truncates_long_bodies() {
        let out = redact_error_body(&"x".repeat(500));
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 241);
    }

    #[test]
    fn strips_authorize_query() {
        let u =
            Url::parse("https://api.ethira.dev/v1/oauth/authorize?state=abc&code_challenge=xyz")
                .unwrap();
        let shown = authorize_url_for_display(&u);
        assert!(!shown.contains("state="));
        assert!(shown.contains("https://api.ethira.dev/v1/oauth/authorize"));
    }

    #[test]
    fn redacts_jwt_shaped_strings() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signaturepart";
        let out = redact_error_body(&format!("boom {jwt} done"));
        assert!(out.contains("<redacted-jwt>"));
        assert!(!out.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn redacts_long_opaque_tokens() {
        let token = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789-_extra";
        let out = scrub_token_shaped(&format!("id={token}"));
        assert!(out.contains("<redacted-token>"));
        assert!(!out.contains("AbCdEfGhIjKlMnOpQrStUvWxYz0123456789"));
    }
}
