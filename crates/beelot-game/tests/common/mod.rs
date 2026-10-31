//! Loads the recorded deals. Shared by the integration tests that check the bots against them.

#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use beelot_game::bidding::{AuctionDecision, AuctionView};
use beelot_game::board::PlayView;
use beelot_game::bot::BotStrategy;
use beelot_game::bot::rule_based::RuleBasedStrategy;
use beelot_game::scoring::RoundResult;
use beelot_game::{Card, Seat, Variant};
use serde::Deserialize;

/// One deal as `FixtureRecorder` wrote it: the setup, every recorded step and the round's result.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deal {
    pub seed: i64,
    pub variant: Variant,
    pub announcements: bool,
    /// Whether the table plays all trumps and no trumps (US-063).
    pub all_trumps: bool,
    pub dealer: Seat,
    pub north_south: String,
    pub east_west: String,
    pub steps: Vec<Step>,
    /// `None` when everyone passed and the cards were dealt again.
    pub result: Option<RoundResult>,
}

/// A recorded step: an auction call, a card play, or moving on after a completed trick.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Auction {
        seat: Seat,
        view: AuctionView,
        decision: AuctionDecision,
    },
    Play {
        seat: Seat,
        view: PlayView,
        card: Card,
    },
    Continue {},
}

fn fixtures_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures"))
}

/// Every recorded deal, labelled `<file>:<line>` (lines counted from 1), in file name order.
pub fn load_deals() -> Vec<(String, Deal)> {
    let mut files: Vec<_> = fs::read_dir(fixtures_dir())
        .expect("the fixtures directory is readable")
        .map(|entry| entry.expect("a fixtures directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("deals-") && name.ends_with(".jsonl"))
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no deals-*.jsonl in {:?}",
        fixtures_dir()
    );

    let mut deals = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let label = format!("{name}:{}", index + 1);
            let deal: Deal =
                serde_json::from_str(line).unwrap_or_else(|error| panic!("{label}: {error}"));
            deals.push((label, deal));
        }
    }
    deals
}

/// Plays the current rules and counts the decisions it was asked for.
pub struct RecordingStrategy {
    rules: RuleBasedStrategy,
    auction_decisions: AtomicUsize,
    card_decisions: AtomicUsize,
}

impl RecordingStrategy {
    pub fn new() -> Self {
        Self {
            rules: RuleBasedStrategy::new(),
            auction_decisions: Default::default(),
            card_decisions: Default::default(),
        }
    }

    pub fn decisions(&self) -> usize {
        self.auction_decisions.load(Ordering::Relaxed) + self.card_decisions()
    }

    pub fn card_decisions(&self) -> usize {
        self.card_decisions.load(Ordering::Relaxed)
    }
}

impl BotStrategy for RecordingStrategy {
    fn decide_auction(&self, view: &AuctionView) -> AuctionDecision {
        self.auction_decisions.fetch_add(1, Ordering::Relaxed);
        self.rules.decide_auction(view)
    }

    fn choose_card(&self, view: &PlayView) -> Card {
        self.card_decisions.fetch_add(1, Ordering::Relaxed);
        self.rules.choose_card(view)
    }
}
