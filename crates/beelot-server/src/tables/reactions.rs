//! The preset reactions players send at an online table.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use super::messages as msg;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TableReaction {
    WellPlayed,
    NiceHand,
    Oops,
    Wow,
    Thinking,
    GoodGame,
}

const KEPT_FOR: Duration = Duration::seconds(30);
const MAX_KEPT: usize = 40;
const BURST: usize = 3;
const BURST_WINDOW: Duration = Duration::seconds(10);

/// A reaction sent from the seat at `position`; the sequence orders the reactions of every table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reaction {
    pub sequence: u64,
    pub position: usize,
    pub reaction: TableReaction,
    pub sent_at: OffsetDateTime,
}

/// The reactions recently sent at one table. A reaction is kept for 30 seconds, and a player may send at most 3
/// reactions in 10 seconds. Ports Java's `TableReactions`.
#[derive(Debug, Default)]
pub struct Reactions {
    sequence: Arc<AtomicU64>,
    kept: VecDeque<Reaction>,
    sent_by_seat: [VecDeque<OffsetDateTime>; 4],
}

impl Reactions {
    pub fn new(sequence: Arc<AtomicU64>) -> Self {
        Self {
            sequence,
            ..Self::default()
        }
    }

    /// Sends a reaction from the seat; returns its sequence.
    pub fn send(
        &mut self,
        position: usize,
        reaction: TableReaction,
        now: OffsetDateTime,
    ) -> Result<u64, AppError> {
        let sent = &mut self.sent_by_seat[position];
        while sent.front().is_some_and(|&at| at <= now - BURST_WINDOW) {
            sent.pop_front();
        }
        if sent.len() >= BURST {
            return Err(AppError::conflict("slow-down", msg::SLOW_DOWN));
        }
        sent.push_back(now);
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        self.kept.push_back(Reaction {
            sequence,
            position,
            reaction,
            sent_at: now,
        });
        self.prune(now);
        Ok(sequence)
    }

    /// The reactions sent in the last 30 seconds, oldest first.
    pub fn recent(&self, now: OffsetDateTime) -> Vec<Reaction> {
        self.kept
            .iter()
            .filter(|reaction| reaction.sent_at >= now - KEPT_FOR)
            .copied()
            .collect()
    }

    fn prune(&mut self, now: OffsetDateTime) {
        while self.kept.len() > MAX_KEPT
            || self
                .kept
                .front()
                .is_some_and(|reaction| reaction.sent_at < now - KEPT_FOR)
        {
            self.kept.pop_front();
        }
    }
}

#[cfg(test)]
#[path = "reactions_tests.rs"]
mod tests;
