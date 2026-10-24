//! The retention periods of the privacy policy, enforced every night at 03:30 UTC: security events are kept 12
//! months, and accounts 3 years after their last sign-in. Each account is deleted on its own, through the deletion
//! path, so one failure leaves the others unaffected. Ports Java's `RetentionJob`.

use time::{Duration, OffsetDateTime, Time};

use crate::app::AppState;
use crate::db::{DbUuid, utc};
use crate::privacy::deletion;

const EVENT_RETENTION: Duration = Duration::days(365);
const ACCOUNT_RETENTION_YEARS: i32 = 3;
const RUNS_AT: Time = match Time::from_hms(3, 30, 0) {
    Ok(time) => time,
    Err(_) => panic!("a valid time"),
};

/// The same instant `years` earlier; February 29th becomes February 28th.
fn years_before(now: OffsetDateTime, years: i32) -> OffsetDateTime {
    let year = now.year() - years;
    now.replace_year(year)
        .or_else(|_| now.replace_day(28).and_then(|day| day.replace_year(year)))
        .unwrap_or(now - Duration::days(365 * i64::from(years)))
}

/// The next 03:30 UTC strictly after `now`.
pub fn next_run(now: OffsetDateTime) -> OffsetDateTime {
    let today = now.replace_time(RUNS_AT);
    if today > now {
        today
    } else {
        today + Duration::days(1)
    }
}

pub async fn run(state: &AppState, now: OffsetDateTime) {
    match sqlx::query("DELETE FROM security_event WHERE occurred_at < ?")
        .bind(utc(now - EVENT_RETENTION))
        .execute(&state.pool)
        .await
    {
        Ok(purged) if purged.rows_affected() > 0 => {
            tracing::info!(
                purged = purged.rows_affected(),
                "purged security events older than 12 months"
            );
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "could not purge the old security events"),
    }
    let due: Vec<DbUuid> =
        match sqlx::query_scalar("SELECT id FROM account WHERE last_sign_in_at <= ?")
            .bind(utc(years_before(now, ACCOUNT_RETENTION_YEARS)))
            .fetch_all(&state.pool)
            .await
        {
            Ok(due) => due,
            Err(error) => {
                tracing::warn!(%error, "could not find the inactive accounts");
                return;
            }
        };
    let count = due.len();
    for DbUuid(account) in due {
        if let Err(error) = deletion::delete(state, account, None).await {
            tracing::warn!(%error, %account, "could not delete an inactive account");
        }
    }
    if count > 0 {
        tracing::info!(count, "deleted accounts inactive for 3 years");
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn three_years_before_is_calendar_years() {
        assert_eq!(
            years_before(datetime!(2026-10-24 12:00 UTC), 3),
            datetime!(2023-10-24 12:00 UTC)
        );
        assert_eq!(
            years_before(datetime!(2028-02-29 12:00 UTC), 3),
            datetime!(2025-02-28 12:00 UTC)
        );
    }
}
