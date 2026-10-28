//! The match history tables: `match_record`, `match_seat` and `match_round`. Teams are stored as NORTH_SOUTH or
//! EAST_WEST, the rest as Java's enum names.

use std::collections::HashMap;

use beelot_game::{Difficulty, Team, Trump, Variant};
use sqlx::{FromRow, MySql, MySqlConnection, MySqlPool, QueryBuilder};
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use super::{DbUuid, from_utc, id, utc};
use crate::tables::finished::Mode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSeat {
    pub account: Option<Uuid>,
    pub name: String,
    pub team: Team,
    pub bot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRound {
    pub declaring_team: Team,
    pub trump: Trump,
    pub contract_value: Option<i32>,
    pub coinched: bool,
    pub contract_made: bool,
    pub capot_team: Option<Team>,
    pub belote_team: Option<Team>,
    pub north_south_points: i32,
    pub east_west_points: i32,
    pub north_south_announcements: i32,
    pub east_west_announcements: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMatch {
    pub id: Uuid,
    pub ended_at: OffsetDateTime,
    pub mode: Mode,
    pub variant: Variant,
    pub difficulty: Option<Difficulty>,
    pub north_south_score: i32,
    pub east_west_score: i32,
    pub winning_team: Team,
    pub announcements: bool,
    /// Whether bids could name tout-atout and sans-atout.
    pub all_trumps: bool,
    /// The points the match was played to.
    pub target: i32,
    /// In play order, North first.
    pub seats: Vec<StoredSeat>,
    /// In the order played.
    pub rounds: Vec<StoredRound>,
}

pub fn team_code(team: Team) -> &'static str {
    match team {
        Team::NorthSouth => "NORTH_SOUTH",
        Team::EastWest => "EAST_WEST",
    }
}

fn team(code: &str) -> Team {
    if code.starts_with("NORTH") {
        Team::NorthSouth
    } else {
        Team::EastWest
    }
}

pub fn mode_code(mode: Mode) -> &'static str {
    match mode {
        Mode::Solo => "SOLO",
        Mode::Private => "PRIVATE",
        Mode::Public => "PUBLIC",
    }
}

fn mode(code: &str) -> Mode {
    match code {
        "PRIVATE" => Mode::Private,
        "PUBLIC" => Mode::Public,
        _ => Mode::Solo,
    }
}

pub fn variant_code(variant: Variant) -> &'static str {
    match variant {
        Variant::Classic => "CLASSIC",
        Variant::Contree => "CONTREE",
    }
}

fn variant(code: &str) -> Variant {
    if code == "CONTREE" {
        Variant::Contree
    } else {
        Variant::Classic
    }
}

pub fn difficulty_code(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Relaxed => "RELAXED",
        Difficulty::Challenging => "CHALLENGING",
    }
}

fn difficulty(code: &str) -> Option<Difficulty> {
    match code {
        "RELAXED" => Some(Difficulty::Relaxed),
        "CHALLENGING" => Some(Difficulty::Challenging),
        _ => None,
    }
}

fn trump(code: &str) -> Trump {
    match code {
        "CLUBS" => Trump::Clubs,
        "DIAMONDS" => Trump::Diamonds,
        "SPADES" => Trump::Spades,
        "ALL_TRUMPS" => Trump::AllTrumps,
        "NO_TRUMPS" => Trump::NoTrumps,
        _ => Trump::Hearts,
    }
}

pub async fn exists(db: &mut MySqlConnection, match_id: Uuid) -> sqlx::Result<bool> {
    let found: Option<i32> = sqlx::query_scalar("SELECT 1 FROM match_record WHERE id = ?")
        .bind(id(match_id))
        .fetch_optional(db)
        .await?;
    Ok(found.is_some())
}

pub async fn insert(db: &mut MySqlConnection, stored: &StoredMatch) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO match_record (id, ended_at, mode, variant, difficulty, north_south_score, east_west_score, \
         winning_team, announcements, all_trumps, target) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id(stored.id))
    .bind(utc(stored.ended_at))
    .bind(mode_code(stored.mode))
    .bind(variant_code(stored.variant))
    .bind(stored.difficulty.map(difficulty_code))
    .bind(stored.north_south_score)
    .bind(stored.east_west_score)
    .bind(team_code(stored.winning_team))
    .bind(stored.announcements)
    .bind(stored.all_trumps)
    .bind(stored.target)
    .execute(&mut *db)
    .await?;
    for (position, seat) in stored.seats.iter().enumerate() {
        sqlx::query(
            "INSERT INTO match_seat (match_id, position, account_id, name, team, bot) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id(stored.id))
        .bind(position as i32)
        .bind(seat.account.map(id))
        .bind(&seat.name)
        .bind(team_code(seat.team))
        .bind(seat.bot)
        .execute(&mut *db)
        .await?;
    }
    for (index, round) in stored.rounds.iter().enumerate() {
        sqlx::query(
            "INSERT INTO match_round (match_id, round_index, declaring_team, trump, contract_value, coinched, \
             contract_made, capot_team, belote_team, north_south_points, east_west_points, \
             north_south_announcements, east_west_announcements) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id(stored.id))
        .bind(index as i32)
        .bind(team_code(round.declaring_team))
        .bind(crate::tables::messages::trump_name(round.trump))
        .bind(round.contract_value)
        .bind(round.coinched)
        .bind(round.contract_made)
        .bind(round.capot_team.map(team_code))
        .bind(round.belote_team.map(team_code))
        .bind(round.north_south_points)
        .bind(round.east_west_points)
        .bind(round.north_south_announcements)
        .bind(round.east_west_announcements)
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}

#[derive(FromRow)]
struct MatchRow {
    id: DbUuid,
    ended_at: PrimitiveDateTime,
    mode: String,
    variant: String,
    difficulty: Option<String>,
    north_south_score: i32,
    east_west_score: i32,
    winning_team: String,
    announcements: bool,
    all_trumps: bool,
    target: i32,
}

#[derive(FromRow)]
struct SeatRow {
    match_id: DbUuid,
    account_id: Option<DbUuid>,
    name: String,
    team: String,
    bot: bool,
}

#[derive(FromRow)]
struct RoundRow {
    match_id: DbUuid,
    declaring_team: String,
    trump: String,
    contract_value: Option<i32>,
    coinched: bool,
    contract_made: bool,
    capot_team: Option<String>,
    belote_team: Option<String>,
    north_south_points: i32,
    east_west_points: i32,
    north_south_announcements: i32,
    east_west_announcements: i32,
}

/// The matches in which the account held a seat, most recent first, with their seats and rounds.
pub async fn played_by(pool: &MySqlPool, account: Uuid) -> sqlx::Result<Vec<StoredMatch>> {
    let rows: Vec<MatchRow> = sqlx::query_as(
        "SELECT DISTINCT m.id, m.ended_at, m.mode, m.variant, m.difficulty, m.north_south_score, \
         m.east_west_score, m.winning_team, m.announcements, m.all_trumps, m.target FROM match_record m \
         JOIN match_seat s ON s.match_id = m.id WHERE s.account_id = ? ORDER BY m.ended_at DESC",
    )
    .bind(id(account))
    .fetch_all(pool)
    .await?;
    let mut matches: Vec<StoredMatch> = rows
        .into_iter()
        .map(|row| StoredMatch {
            id: row.id.0,
            ended_at: from_utc(row.ended_at),
            mode: mode(&row.mode),
            variant: variant(&row.variant),
            difficulty: row.difficulty.as_deref().and_then(difficulty),
            north_south_score: row.north_south_score,
            east_west_score: row.east_west_score,
            winning_team: team(&row.winning_team),
            announcements: row.announcements,
            all_trumps: row.all_trumps,
            target: row.target,
            seats: Vec::new(),
            rounds: Vec::new(),
        })
        .collect();
    if matches.is_empty() {
        return Ok(matches);
    }
    let index: HashMap<Uuid, usize> = matches
        .iter()
        .enumerate()
        .map(|(position, stored)| (stored.id, position))
        .collect();
    let ids = |builder: &mut QueryBuilder<'_, MySql>| {
        let mut list = builder.separated(", ");
        for stored in &matches {
            list.push_bind(id(stored.id));
        }
        builder.push(")");
    };
    let mut seats = QueryBuilder::new(
        "SELECT match_id, account_id, name, team, bot FROM match_seat WHERE match_id IN (",
    );
    ids(&mut seats);
    seats.push(" ORDER BY match_id, position");
    let seat_rows: Vec<SeatRow> = seats.build_query_as().fetch_all(pool).await?;
    let mut rounds = QueryBuilder::new(
        "SELECT match_id, declaring_team, trump, contract_value, coinched, contract_made, capot_team, belote_team, \
         north_south_points, east_west_points, north_south_announcements, east_west_announcements \
         FROM match_round WHERE match_id IN (",
    );
    ids(&mut rounds);
    rounds.push(" ORDER BY match_id, round_index");
    let round_rows: Vec<RoundRow> = rounds.build_query_as().fetch_all(pool).await?;
    for row in seat_rows {
        matches[index[&row.match_id.0]].seats.push(StoredSeat {
            account: row.account_id.map(|account| account.0),
            name: row.name,
            team: team(&row.team),
            bot: row.bot,
        });
    }
    for row in round_rows {
        matches[index[&row.match_id.0]].rounds.push(StoredRound {
            declaring_team: team(&row.declaring_team),
            trump: trump(&row.trump),
            contract_value: row.contract_value,
            coinched: row.coinched,
            contract_made: row.contract_made,
            capot_team: row.capot_team.as_deref().map(team),
            belote_team: row.belote_team.as_deref().map(team),
            north_south_points: row.north_south_points,
            east_west_points: row.east_west_points,
            north_south_announcements: row.north_south_announcements,
            east_west_announcements: row.east_west_announcements,
        });
    }
    Ok(matches)
}
