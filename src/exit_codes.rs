//! Process exit codes for agent-friendly scripting.
//!
//! | Code | Meaning |
//! | ---- | ------- |
//! | 0 | Success |
//! | 1 | Generic error |
//! | 2 | Not logged in / session expired / rejected |
//! | 3 | Local inventory cache missing or unusable |
//! | 4 | SQL rejected or failed (sandbox / prepare / execute) |

use crate::ui::UnauthorizedError;

pub const SUCCESS: u8 = 0;
pub const GENERIC: u8 = 1;
pub const UNAUTHORIZED: u8 = 2;
pub const CACHE_MISSING: u8 = 3;
pub const SQL_ERROR: u8 = 4;

/// Typed error: local inventory cache is missing.
#[derive(Debug)]
pub struct CacheMissingError {
    pub message: String,
}

impl CacheMissingError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CacheMissingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CacheMissingError {}

/// Typed error: sandboxed SQL was rejected or failed.
#[derive(Debug)]
pub struct SqlError {
    pub message: String,
}

impl SqlError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SqlError {}

/// Map an error to a process exit code.
pub fn code_for(err: &anyhow::Error) -> u8 {
    if err.downcast_ref::<UnauthorizedError>().is_some() {
        return UNAUTHORIZED;
    }
    if err.downcast_ref::<CacheMissingError>().is_some() {
        return CACHE_MISSING;
    }
    if err.downcast_ref::<SqlError>().is_some() {
        return SQL_ERROR;
    }
    GENERIC
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{AuthFailure, UnauthorizedError};

    #[test]
    fn classifies_typed_errors() {
        let err: anyhow::Error = UnauthorizedError::new(AuthFailure::NotLoggedIn).into();
        assert_eq!(code_for(&err), UNAUTHORIZED);
        let err: anyhow::Error = CacheMissingError::new("missing").into();
        assert_eq!(code_for(&err), CACHE_MISSING);
        let err: anyhow::Error = SqlError::new("bad sql").into();
        assert_eq!(code_for(&err), SQL_ERROR);
        let err = anyhow::anyhow!("other");
        assert_eq!(code_for(&err), GENERIC);
    }
}
