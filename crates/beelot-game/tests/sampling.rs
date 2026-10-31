//! The parts of the Java `SamplingStrategyTest` that only need the public API, and a check that a
//! search without a time budget depends only on the seed and the view.

mod common;

use std::time::{Duration, Instant};

use beelot_game::bidding::BiddingState;
use beelot_game::board::{GameBoard, PlayView};
use beelot_game::bot::BotStrategy;
use beelot_game::bot::rule_based::RuleBasedStrategy;
use beelot_game::bot::sampling::SamplingStrategy;
use beelot_game::bot::turns::{play_turn, take_auction_turn};
use beelot_game::card::{Card, Variant};
use beelot_game::rng::SplitMix64;
use common::{Step, load_deals};

/// A seeded Contrée deal with the strategy bidding at every seat, or `None` if it was redealt.
fn auction(seed: i64, strategy: &dyn BotStrategy) -> Option<GameBoard> {
    let mut bidding = BiddingState::new(Variant::Contree, 0, Box::new(SplitMix64::new(seed)));
    while bidding.completed_board().is_none() && bidding.deals() == 1 {
        take_auction_turn(&mut bidding, strategy).unwrap();
    }
    bidding.into_board()
}

/// Plays a seeded Contrée deal with the strategy at every seat; returns the cards in the order
/// played.
fn play_round(strategy: &dyn BotStrategy) -> Vec<Card> {
    let mut seed = 1;
    let mut board = loop {
        if let Some(board) = auction(seed, strategy) {
            break board;
        }
        seed += 1;
    };
    let mut played = Vec::new();
    while board.round_result().is_none() {
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            let seat = board.active_seat();
            let card = strategy.choose_card(&board.play_view(seat));
            played.push(card);
            board.play(seat, card, true).unwrap();
        }
    }
    played
}

/// A seeded Contrée deal played by rule-based bots up to the given number of tricks, or `None` if
/// redealt.
fn board_after_tricks(seed: i64, tricks: usize) -> Option<GameBoard> {
    let rules = RuleBasedStrategy::new();
    let mut board = auction(seed, &rules)?;
    while board.play_view(board.active_seat()).tricks.len() < tricks
        || board.reviewing_completed_trick()
    {
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            play_turn(&mut board, &rules).unwrap();
        }
    }
    Some(board)
}

#[test]
fn the_same_seed_plays_the_same_round() {
    // Java uses the default 200 ms budget; without one, the search is never cut short.
    let first = play_round(&SamplingStrategy::with_settings(7, 50, None, 3));
    let second = play_round(&SamplingStrategy::with_settings(7, 50, None, 3));

    assert_eq!(first, second);
}

#[test]
fn chooses_a_card_within_its_time_budget() {
    let board = board_after_tricks(3, 2).expect("the deal is played");
    let view = board.play_view(board.active_seat());
    let strategy =
        SamplingStrategy::with_settings(1, 1_000_000, Some(Duration::from_millis(50)), 1);

    let start = Instant::now();
    let card = strategy.choose_card(&view);
    let elapsed = start.elapsed();

    assert!(view.legal_cards.contains(&card));
    assert!(
        elapsed < Duration::from_millis(50 + 150),
        "took {elapsed:?}"
    );
}

/// The first recorded view from the fifth trick on with a choice of cards.
fn late_round_view() -> PlayView {
    load_deals()
        .into_iter()
        .flat_map(|(_, deal)| deal.steps)
        .find_map(|step| match step {
            Step::Play { view, .. } if view.tricks.len() >= 4 && view.legal_cards.len() > 1 => {
                Some(view)
            }
            _ => None,
        })
        .expect("a recorded late-round view")
}

#[test]
fn sampling_without_budget_is_repeatable() {
    let view = late_round_view();

    let first = SamplingStrategy::with_settings(1, 50, None, 3).choose_card(&view);
    let second = SamplingStrategy::with_settings(1, 50, None, 3).choose_card(&view);

    assert!(view.legal_cards.contains(&first));
    assert_eq!(first, second);
}
