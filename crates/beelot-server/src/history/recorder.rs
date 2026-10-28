//! Stores each finished match a signed-in player took part in. Ports Java's `MatchHistoryService`.

use futures_util::future::BoxFuture;
use sqlx::MySqlPool;

use crate::db::accounts;
use crate::db::matches::{self, StoredMatch, StoredRound, StoredSeat};
use crate::tables::finished::{DELETED_PLAYER, FinishedMatch, MatchSink};
use beelot_game::Team;

/// A seat taken by a signed-in player is named after their account, or "Deleted player" with no account once that
/// account is gone. A match is stored once, under its id. The history never stops a game: when it cannot be written,
/// the failure is logged and the match is left out.
#[derive(Debug, Clone)]
pub struct HistoryRecorder {
    pool: MySqlPool,
}

impl HistoryRecorder {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    async fn store(&self, finished: &FinishedMatch) -> sqlx::Result<()> {
        let mut transaction = self.pool.begin().await?;
        if matches::exists(&mut transaction, finished.id).await? {
            return Ok(());
        }
        let mut seats = Vec::with_capacity(finished.seats.len());
        for (position, seat) in finished.seats.iter().enumerate() {
            let (mut account, mut name) = (seat.account, seat.name.clone());
            if let Some(id) = seat.account {
                // The account may have been deleted while the match was played: the seat is then anonymous.
                match accounts::find(&mut *transaction, id).await? {
                    Some(found) => name = found.display_name,
                    None => {
                        account = None;
                        name = DELETED_PLAYER.into();
                    }
                }
            }
            seats.push(StoredSeat {
                account,
                name,
                team: Team::of(position),
                bot: seat.bot,
            });
        }
        let rounds = finished
            .rounds
            .iter()
            .map(|round| {
                let result = &round.result;
                StoredRound {
                    declaring_team: round.declaring_team,
                    trump: round.trump,
                    contract_value: round.contract_value,
                    coinched: round.coinched,
                    contract_made: result.contract_made,
                    capot_team: result.capot_team,
                    belote_team: if result.north_south_belote_bonus > 0 {
                        Some(Team::NorthSouth)
                    } else if result.east_west_belote_bonus > 0 {
                        Some(Team::EastWest)
                    } else {
                        None
                    },
                    north_south_points: result.north_south_awarded,
                    east_west_points: result.east_west_awarded,
                    north_south_announcements: result.north_south_announcements,
                    east_west_announcements: result.east_west_announcements,
                }
            })
            .collect();
        let stored = StoredMatch {
            id: finished.id,
            ended_at: finished.ended_at,
            mode: finished.mode,
            variant: finished.variant,
            difficulty: finished.difficulty,
            north_south_score: finished.north_south_score,
            east_west_score: finished.east_west_score,
            winning_team: finished.winning_team,
            announcements: finished.announcements,
            all_trumps: finished.all_trumps,
            target: finished.target,
            seats,
            rounds,
        };
        matches::insert(&mut transaction, &stored).await?;
        transaction.commit().await
    }
}

impl MatchSink for HistoryRecorder {
    fn record(&self, finished: FinishedMatch) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            if !finished.has_signed_in_player() {
                return;
            }
            if let Err(error) = self.store(&finished).await {
                tracing::warn!(%error, match_id = %finished.id, "could not record a match in the history");
            }
        })
    }
}
