//! Loads the deals recorded from the Java engine and replays them through the Rust engine.
//! Shared by the integration tests that check the engine and the bots against Java.

#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use beelot_game::bidding::{AuctionDecision, AuctionView, BiddingState};
use beelot_game::board::{GameBoard, PlayView};
use beelot_game::bot::BotStrategy;
use beelot_game::bot::rule_based::RuleBasedStrategy;
use beelot_game::scoring::RoundResult;
use beelot_game::{Card, JavaRandom, Seat, SplitMix64, Variant};
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

/// The deal's state: the auction, then the card play once a card play step arrives.
enum Phase {
    Bidding(Box<BiddingState>),
    Board(Box<GameBoard>),
    /// Only while switching from the auction to the board.
    Switching,
}

/// Replays a deal through the engine. For each step, it asserts that the engine's view equals the
/// recorded view, calls `check` with the deal's single bot generator, then applies the recorded
/// decision. At the end it asserts that the result equals the recorded one (`None` ⇔ redeal).
/// Panics name the step index and show the expected and actual values.
pub fn replay(deal: &Deal, check: &mut dyn FnMut(&Step, &mut SplitMix64)) {
    let bidding = BiddingState::new(
        deal.variant,
        deal.dealer,
        Box::new(JavaRandom::new(deal.seed)),
    )
    .with_announcements(deal.announcements)
    .with_all_trumps(deal.all_trumps);
    let mut phase = Phase::Bidding(Box::new(bidding));
    let mut bots = SplitMix64::new(deal.seed);

    for (index, step) in deal.steps.iter().enumerate() {
        match step {
            Step::Auction {
                seat,
                view,
                decision,
            } => {
                let Phase::Bidding(bidding) = &mut phase else {
                    panic!("step {index}: an auction step after the auction completed");
                };
                assert!(
                    bidding.completed_board().is_none() && bidding.deals() == 1,
                    "step {index}: an auction step but the auction is over (deals: {})",
                    bidding.deals()
                );
                assert_eq!(
                    bidding.active_seat(),
                    *seat,
                    "step {index}: active seat (left: engine, right: recorded)"
                );
                let actual = bidding.auction_view(*seat);
                assert_eq!(
                    &actual, view,
                    "step {index}: auction view (left: engine, right: recorded)"
                );
                check(step, &mut bots);
                if let Err(error) = bidding.apply(*seat, *decision) {
                    panic!("step {index}: {decision:?} by seat {seat} was refused: {error:?}");
                }
            }
            Step::Play { seat, view, card } => {
                let board = board(&mut phase, index);
                assert!(
                    !board.reviewing_completed_trick(),
                    "step {index}: a play step while a completed trick is under review"
                );
                assert_eq!(
                    board.active_seat(),
                    *seat,
                    "step {index}: active seat (left: engine, right: recorded)"
                );
                let actual = board.play_view(*seat);
                assert_eq!(
                    &actual, view,
                    "step {index}: play view (left: engine, right: recorded)"
                );
                check(step, &mut bots);
                if let Err(error) = board.play(*seat, *card, true) {
                    panic!("step {index}: {card:?} by seat {seat} was refused: {error:?}");
                }
            }
            Step::Continue {} => {
                let board = board(&mut phase, index);
                assert!(
                    board.reviewing_completed_trick(),
                    "step {index}: a continue step but no completed trick is under review"
                );
                check(step, &mut bots);
                if let Err(error) = board.continue_after_trick() {
                    panic!("step {index}: continuing after the trick was refused: {error:?}");
                }
            }
        }
    }

    let steps = deal.steps.len();
    match (&phase, &deal.result) {
        (Phase::Bidding(bidding), None) => {
            assert!(
                bidding.completed_board().is_none(),
                "after {steps} steps: the recorded deal was redealt but the engine completed the auction"
            );
            assert_eq!(
                bidding.deals(),
                2,
                "after {steps} steps: deals (left: engine, right: recorded redeal)"
            );
        }
        (Phase::Bidding(bidding), Some(expected)) => panic!(
            "after {steps} steps: expected result {expected:?}, but the engine is still bidding \
             (deals: {}, completed: {})",
            bidding.deals(),
            bidding.completed_board().is_some()
        ),
        (Phase::Board(board), expected) => assert_eq!(
            board.round_result(),
            expected.as_ref(),
            "after {steps} steps: round result (left: engine, right: recorded)"
        ),
        (Phase::Switching, _) => unreachable!(),
    }
}

/// The board, switching to it when the auction has just completed: the recorded steps decide when.
fn board(phase: &mut Phase, index: usize) -> &mut GameBoard {
    if let Phase::Bidding(_) = phase {
        let Phase::Bidding(bidding) = std::mem::replace(phase, Phase::Switching) else {
            unreachable!()
        };
        assert_eq!(
            bidding.deals(),
            1,
            "step {index}: a card play step but the engine dealt again"
        );
        let Some(board) = bidding.into_board() else {
            panic!("step {index}: a card play step but the engine's auction is not complete");
        };
        *phase = Phase::Board(Box::new(board));
    }
    match phase {
        Phase::Board(board) => board,
        Phase::Bidding(_) | Phase::Switching => unreachable!(),
    }
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
