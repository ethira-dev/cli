/// Outcome of the optional post-login first sync prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstSyncOutcome {
    /// Cache already present; no prompt.
    NotNeeded,
    /// User accepted and sync completed.
    Synced,
    /// User declined; they can run `ethira sync` later.
    Declined,
}
