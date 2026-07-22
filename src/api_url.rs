use anyhow::{Result, bail};
use url::Url;

pub const DEFAULT_API_HOST: &str = "api.ethira.dev";

/// Normalize and validate the API base URL.
///
/// Requires `https:` except for explicit loopback hosts (`localhost`, `127.0.0.1`)
/// which may use `http:` for local development.
///
/// Non-default, non-loopback hosts require `allow_untrusted`.
pub fn validate_and_normalize(api_url: &str, allow_untrusted: bool) -> Result<String> {
    let trimmed = api_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        bail!("API URL must not be empty");
    }
    let parsed = Url::parse(trimmed).map_err(|e| anyhow::anyhow!("invalid API URL: {e}"))?;
    let scheme = parsed.scheme();
    let host = parsed.host_str().unwrap_or("");
    if host.is_empty() {
        bail!("API URL must include a host");
    }
    let loopback = is_loopback_host(host);
    match scheme {
        "https" => {}
        "http" if loopback => {}
        "http" => bail!(
            "refusing cleartext API URL `{trimmed}` — use https:// (http:// is only allowed for localhost / 127.0.0.1)"
        ),
        other => bail!("unsupported API URL scheme `{other}` — use https://"),
    }
    if !parsed.path().is_empty() && parsed.path() != "/" {
        bail!("API URL must not include a path (got `{}`)", parsed.path());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        bail!("API URL must not include query or fragment");
    }

    if !loopback && !host.eq_ignore_ascii_case(DEFAULT_API_HOST) && !allow_untrusted {
        bail!(
            "refusing API host `{host}` — only `{DEFAULT_API_HOST}` and loopback are trusted by default. \
             Pass --allow-untrusted-api-host if you intend to send OAuth tokens to this host."
        );
    }

    Ok(trimmed.to_string())
}

pub fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

/// Return an error when CLI override disagrees with stored credentials host.
pub fn ensure_api_url_consistent(cli_api_url: &str, stored_api_url: &str) -> Result<()> {
    let cli = cli_api_url.trim_end_matches('/');
    let stored = stored_api_url.trim_end_matches('/');
    if stored.is_empty() || cli.eq_ignore_ascii_case(stored) {
        return Ok(());
    }
    bail!(
        "CLI API URL `{cli}` does not match stored credentials API URL `{stored}`. \
         Re-run `ethira login` against the intended API, or pass the matching --api-url."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_default_https() {
        assert_eq!(
            validate_and_normalize("https://api.ethira.dev", false).unwrap(),
            "https://api.ethira.dev"
        );
        assert_eq!(
            validate_and_normalize("https://api.ethira.dev/", false).unwrap(),
            "https://api.ethira.dev"
        );
    }

    #[test]
    fn accepts_loopback_http() {
        assert!(validate_and_normalize("http://localhost:3000", false).is_ok());
        assert!(validate_and_normalize("http://127.0.0.1:3000", false).is_ok());
    }

    #[test]
    fn rejects_cleartext_remote() {
        let err = validate_and_normalize("http://evil.example", false).unwrap_err();
        assert!(format!("{err:#}").contains("cleartext"));
    }

    #[test]
    fn rejects_path() {
        assert!(validate_and_normalize("https://api.ethira.dev/v1", false).is_err());
    }

    #[test]
    fn rejects_untrusted_host_without_flag() {
        let err = validate_and_normalize("https://evil.example", false).unwrap_err();
        assert!(format!("{err:#}").contains("allow-untrusted-api-host"));
    }

    #[test]
    fn accepts_untrusted_host_with_flag() {
        assert!(validate_and_normalize("https://evil.example", true).is_ok());
    }

    #[test]
    fn detects_api_url_mismatch() {
        assert!(
            ensure_api_url_consistent("https://api.ethira.dev", "https://other.example").is_err()
        );
        assert!(
            ensure_api_url_consistent("https://api.ethira.dev", "https://api.ethira.dev/").is_ok()
        );
    }
}
