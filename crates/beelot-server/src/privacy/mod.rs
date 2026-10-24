//! Privacy: the security event log, the player's data export, account deletion, retention, and the values the
//! privacy policy shows.

pub mod deletion;
pub mod events;
pub mod export;
pub mod retention;

use crate::config::PrivacyConfig;

/// A policy value as the policy shows it: trimmed, or none when blank.
pub fn policy_value(value: &str) -> Option<&str> {
    Some(value.trim()).filter(|value| !value.is_empty())
}

/// The variables the policy needs that are not set: it shows "not configured" for each.
pub fn unset_policy_values(privacy: &PrivacyConfig) -> Vec<&'static str> {
    [
        (&privacy.controller, "BEELOT_PRIVACY_CONTROLLER"),
        (&privacy.contact, "BEELOT_PRIVACY_CONTACT"),
        (&privacy.hosting_provider, "BEELOT_HOSTING_PROVIDER"),
        (
            &privacy.backup_retention_days,
            "BEELOT_BACKUP_RETENTION_DAYS",
        ),
    ]
    .into_iter()
    .filter(|(value, _)| policy_value(value).is_none())
    .map(|(_, variable)| variable)
    .collect()
}
