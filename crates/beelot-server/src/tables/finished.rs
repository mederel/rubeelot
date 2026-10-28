//! A match a team won, as a table hands it over for the history. Ports Java's `history/FinishedMatch`.

use beelot_game::scoring::RoundSummary;
use beelot_game::{Difficulty, Team, Variant};
use futures_util::future::BoxFuture;
use time::OffsetDateTime;
use uuid::Uuid;

/// The name a seat takes once its player's account is deleted.
pub const DELETED_PLAYER: &str = "Deleted player";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Solo,
    Private,
    Public,
}

/// A seat, with the account of the signed-in player who took it (none for a guest or a bot).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedSeat {
    pub account: Option<Uuid>,
    pub name: String,
    pub bot: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FinishedMatch {
    pub id: Uuid,
    pub ended_at: OffsetDateTime,
    pub mode: Mode,
    pub variant: Variant,
    pub difficulty: Option<Difficulty>,
    pub north_south_score: i32,
    pub east_west_score: i32,
    pub winning_team: Team,
    /// In play order, North first.
    pub seats: Vec<FinishedSeat>,
    pub rounds: Vec<RoundSummary>,
    pub announcements: bool,
    /// Whether bids could name tout-atout and sans-atout; never in Classic.
    pub all_trumps: bool,
    pub target: i32,
}

impl FinishedMatch {
    pub fn has_signed_in_player(&self) -> bool {
        self.seats.iter().any(|seat| seat.account.is_some())
    }
}

/// Where finished matches go: the history, once it exists.
pub trait MatchSink: Send + Sync {
    /// Keeps the match; a failure is the sink's to log, never the caller's.
    fn record(&self, finished: FinishedMatch) -> BoxFuture<'_, ()>;
}

/// Forgets every match.
#[derive(Debug, Default)]
pub struct NoopSink;

impl MatchSink for NoopSink {
    fn record(&self, _: FinishedMatch) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }
}
