use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::distr::{Alphanumeric, SampleString};
use sha2::{Digest, Sha256};

const VERIFIER_LEN: usize = 64;

#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

pub fn generate_pkce() -> Pkce {
    let verifier = Alphanumeric.sample_string(&mut rand::rng(), VERIFIER_LEN);
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(digest);
    Pkce {
        verifier,
        challenge,
    }
}

pub fn generate_state() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_lengths_and_charset() {
        let pkce = generate_pkce();
        assert_eq!(pkce.verifier.len(), VERIFIER_LEN);
        assert!(pkce.verifier.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(!pkce.challenge.is_empty());
        assert!(!pkce.challenge.contains('+'));
        assert!(!pkce.challenge.contains('/'));
        assert!(!pkce.challenge.contains('='));
    }
}
