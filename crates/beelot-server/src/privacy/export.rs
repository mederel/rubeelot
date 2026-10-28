//! The data a player downloads: their account, their security events and their matches, newest first, in format
//! `beelot-export-2`. Other players appear by the names shown in the history; no account id but the player's own is
//! ever part of it, and of the passkey only its dates. A player may download it once an hour; each download is
//! recorded as an `EXPORT` event, which sets the next. Ports Java's `DataExport` and `DataExportService`.

use std::net::IpAddr;

use serde::Serialize;
use sqlx::{FromRow, MySqlPool};
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use crate::db::matches::{self, difficulty_code, mode_code, team_code, variant_code};
use crate::db::{accounts, from_utc, id};
use crate::privacy::events::{EventType, Outcome, SecurityEvents};
use crate::tables::messages::suit_name;

pub const FORMAT: &str = "beelot-export-2";
const INTERVAL: time::Duration = time::Duration::hours(1);

/// Times as ISO-8601 instants in UTC.
mod instant {
    use serde::Serializer;
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    pub fn serialize<S: Serializer>(
        time: &OffsetDateTime,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&time.format(&Rfc3339).map_err(serde::ser::Error::custom)?)
    }

    pub mod option {
        use super::*;

        pub fn serialize<S: Serializer>(
            time: &Option<OffsetDateTime>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match time {
                Some(time) => super::serialize(time, serializer),
                None => serializer.serialize_none(),
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataExport {
    pub format: &'static str,
    #[serde(with = "instant")]
    pub exported_at: OffsetDateTime,
    pub account: Account,
    pub security_events: Vec<Event>,
    pub matches: Vec<Match>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: Uuid,
    pub display_name: String,
    #[serde(with = "instant")]
    pub created_at: OffsetDateTime,
    #[serde(with = "instant")]
    pub last_sign_in_at: OffsetDateTime,
    #[serde(with = "instant::option")]
    pub previous_sign_in_at: Option<OffsetDateTime>,
    pub passkey: Option<Passkey>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Passkey {
    #[serde(with = "instant")]
    pub created_at: OffsetDateTime,
    #[serde(with = "instant::option")]
    pub last_used_at: Option<OffsetDateTime>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub r#type: String,
    pub outcome: String,
    pub ip_address: Option<String>,
    #[serde(with = "instant")]
    pub occurred_at: OffsetDateTime,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    #[serde(with = "instant")]
    pub ended_at: OffsetDateTime,
    pub mode: &'static str,
    pub variant: &'static str,
    pub difficulty: Option<&'static str>,
    pub announcements: bool,
    pub target: i32,
    pub your_team: &'static str,
    pub north_south_score: i32,
    pub east_west_score: i32,
    pub won: bool,
    pub seats: Vec<Seat>,
    pub rounds: Vec<Round>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Seat {
    pub position: usize,
    pub name: String,
    pub team: &'static str,
    pub bot: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    pub declaring_team: &'static str,
    pub trump: &'static str,
    pub contract_value: Option<i32>,
    pub coinched: bool,
    pub contract_made: bool,
    pub capot_team: Option<&'static str>,
    pub belote_team: Option<&'static str>,
    pub north_south_points: i32,
    pub east_west_points: i32,
    pub north_south_announcements: i32,
    pub east_west_announcements: i32,
}

/// The export, or the minutes left before the next one.
pub enum Export {
    Ready(Box<DataExport>),
    TooSoon { minutes_left: i64 },
}

#[derive(FromRow)]
struct EventRow {
    r#type: String,
    outcome: String,
    ip_address: Option<String>,
    occurred_at: PrimitiveDateTime,
}

/// The account's data now; `None` when the account is gone.
pub async fn export(
    pool: &MySqlPool,
    events: &SecurityEvents,
    account_id: Uuid,
    ip: Option<IpAddr>,
    now: OffsetDateTime,
) -> sqlx::Result<Option<Export>> {
    let last: Option<PrimitiveDateTime> = sqlx::query_scalar(
        "SELECT MAX(occurred_at) FROM security_event WHERE account_id = ? AND type = 'EXPORT'",
    )
    .bind(id(account_id))
    .fetch_one(pool)
    .await?;
    if let Some(last) = last {
        let next = from_utc(last) + INTERVAL;
        if next > now {
            let seconds = (next - now).whole_seconds();
            return Ok(Some(Export::TooSoon {
                minutes_left: (seconds + 59) / 60,
            }));
        }
    }
    let Some(account) = accounts::find(pool, account_id).await? else {
        return Ok(None);
    };
    let passkey: Option<(PrimitiveDateTime, Option<PrimitiveDateTime>)> =
        sqlx::query_as("SELECT created_at, last_used_at FROM passkey WHERE account_id = ?")
            .bind(id(account_id))
            .fetch_optional(pool)
            .await?;
    let event_rows: Vec<EventRow> = sqlx::query_as(
        "SELECT type, outcome, ip_address, occurred_at FROM security_event WHERE account_id = ? \
         ORDER BY occurred_at DESC, id DESC",
    )
    .bind(id(account_id))
    .fetch_all(pool)
    .await?;
    let played = matches::played_by(pool, account_id).await?;
    let export = DataExport {
        format: FORMAT,
        exported_at: now,
        account: Account {
            id: account.id,
            display_name: account.display_name,
            created_at: account.created_at,
            last_sign_in_at: account.last_sign_in_at,
            previous_sign_in_at: account.previous_sign_in_at,
            passkey: passkey.map(|(created_at, last_used_at)| Passkey {
                created_at: from_utc(created_at),
                last_used_at: last_used_at.map(from_utc),
            }),
        },
        security_events: event_rows
            .into_iter()
            .map(|row| Event {
                r#type: row.r#type,
                outcome: row.outcome,
                ip_address: row.ip_address,
                occurred_at: from_utc(row.occurred_at),
            })
            .collect(),
        matches: played
            .into_iter()
            .filter_map(|stored| {
                let team = stored
                    .seats
                    .iter()
                    .find(|seat| seat.account == Some(account_id))?
                    .team;
                Some(Match {
                    ended_at: stored.ended_at,
                    mode: mode_code(stored.mode),
                    variant: variant_code(stored.variant),
                    difficulty: stored.difficulty.map(difficulty_code),
                    announcements: stored.announcements,
                    target: stored.target,
                    your_team: team_code(team),
                    north_south_score: stored.north_south_score,
                    east_west_score: stored.east_west_score,
                    won: stored.winning_team == team,
                    seats: stored
                        .seats
                        .iter()
                        .enumerate()
                        .map(|(position, seat)| Seat {
                            position,
                            name: seat.name.clone(),
                            team: team_code(seat.team),
                            bot: seat.bot,
                        })
                        .collect(),
                    rounds: stored
                        .rounds
                        .iter()
                        .map(|round| Round {
                            declaring_team: team_code(round.declaring_team),
                            trump: suit_name(round.trump),
                            contract_value: round.contract_value,
                            coinched: round.coinched,
                            contract_made: round.contract_made,
                            capot_team: round.capot_team.map(team_code),
                            belote_team: round.belote_team.map(team_code),
                            north_south_points: round.north_south_points,
                            east_west_points: round.east_west_points,
                            north_south_announcements: round.north_south_announcements,
                            east_west_announcements: round.east_west_announcements,
                        })
                        .collect(),
                })
            })
            .collect(),
    };
    events
        .record(EventType::Export, Outcome::Success, Some(account_id), ip)
        .await;
    Ok(Some(Export::Ready(Box::new(export))))
}
