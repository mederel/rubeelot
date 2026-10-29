//! Replays the matches the Java recorder dealt as at a real table (US-085): each deal is cut from
//! the cards of the one before, an all-pass gathers the hands, and every view, bot decision, deal
//! summary, trick pile and gathered deck must equal Java's.

mod common;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use beelot_game::bidding::{BiddingState, DealSummary};
use beelot_game::bot::arena::arena_bot_unbounded;
use beelot_game::deck::{Deck, Gathering};
use beelot_game::scoring::RoundResult;
use beelot_game::{Card, JavaRandom, Seat, SplitMix64, Variant};
use common::Step;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Match {
    seed: i64,
    variant: Variant,
    announcements: bool,
    all_trumps: bool,
    north_south: String,
    east_west: String,
    deals: Vec<TableDeal>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TableDeal {
    dealer: Seat,
    /// `None` for the first deal of the match, which is shuffled.
    deck: Option<Deck>,
    /// How many times the cards were dealt before a contract was reached.
    deals: u32,
    summaries: Vec<DealSummary>,
    steps: Vec<Step>,
    result: RoundResult,
    trick_piles: [Vec<Card>; 2],
    gathered: Deck,
}

fn load(variant: &str) -> Vec<Match> {
    let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures"))
        .join(format!("table-{variant}.jsonl"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{path:?}: {error}"))
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("a recorded match"))
        .collect()
}

fn replay(label: &str, recorded: &Match) {
    let north_south = arena_bot_unbounded(&recorded.north_south).unwrap();
    let east_west = arena_bot_unbounded(&recorded.east_west).unwrap();
    let mut bots = SplitMix64::new(recorded.seed);
    // One generator cuts every deal of the match, as a game's does.
    let cuts = SharedRandom(Arc::new(Mutex::new(JavaRandom::new(recorded.seed))));
    let mut deck: Option<Deck> = None;
    for (index, deal) in recorded.deals.iter().enumerate() {
        let label = format!("{label} deal {index}");
        assert_eq!(deck, deal.deck, "{label}: the deck dealt from");
        let mut bidding = BiddingState::dealt_from(
            recorded.variant,
            deal.dealer,
            Box::new(cuts.clone()),
            deck.take(),
        )
        .with_announcements(recorded.announcements)
        .with_all_trumps(recorded.all_trumps);
        let mut summaries = vec![bidding.deal_summary().clone()];
        let mut steps = deal.steps.iter().enumerate();
        while bidding.completed_board().is_none() {
            let (
                step,
                Step::Auction {
                    seat,
                    view,
                    decision,
                },
            ) = steps.next().expect("an auction step")
            else {
                panic!("{label}: a card step during the auction");
            };
            assert_eq!(bidding.active_seat(), *seat, "{label} step {step}");
            assert_eq!(
                &bidding.auction_view(*seat),
                view,
                "{label} step {step}: view"
            );
            let bot = if seat % 2 == 0 {
                &north_south
            } else {
                &east_west
            };
            assert_eq!(
                bot.decide_auction(view, &mut bots),
                *decision,
                "{label} step {step}: decision"
            );
            let before = bidding.deals();
            bidding.apply(*seat, *decision).unwrap();
            if bidding.deals() != before {
                summaries.push(bidding.deal_summary().clone());
            }
        }
        assert_eq!(summaries, deal.summaries, "{label}: deal summaries");
        assert_eq!(bidding.deals(), deal.deals, "{label}: deals");
        let next_dealer = (bidding.dealer() + 1) % 4;
        let mut board = bidding.into_board().unwrap();
        for (step, recorded_step) in steps {
            match recorded_step {
                Step::Auction { .. } => {
                    panic!("{label} step {step}: an auction step after it ended")
                }
                Step::Continue {} => board.continue_after_trick().unwrap(),
                Step::Play { seat, view, card } => {
                    assert_eq!(&board.play_view(*seat), view, "{label} step {step}: view");
                    let bot = if seat % 2 == 0 {
                        &north_south
                    } else {
                        &east_west
                    };
                    assert_eq!(
                        bot.choose_card(view, &mut bots),
                        *card,
                        "{label} step {step}: card"
                    );
                    board.play(*seat, *card, true).unwrap();
                }
            }
        }
        assert_eq!(board.round_result(), Some(&deal.result), "{label}: result");
        assert_eq!(board.trick_pile(true), deal.trick_piles[0], "{label}: pile");
        assert_eq!(
            board.trick_pile(false),
            deal.trick_piles[1],
            "{label}: pile"
        );
        let gathered = board.gathered_deck(next_dealer).unwrap();
        assert_eq!(gathered, deal.gathered, "{label}: gathered deck");
        assert_eq!(gathered.gathering(), Gathering::Tricks);
        deck = Some(gathered);
    }
}

/// The match's generator, shared by its deals.
#[derive(Clone)]
struct SharedRandom(Arc<Mutex<JavaRandom>>);

impl beelot_game::RandomSource for SharedRandom {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.0.lock().unwrap().next_int(bound)
    }
}

#[test]
fn classic_table_matches_match_java() {
    let matches = load("classic");
    assert_eq!(matches.len(), 4);
    let redeals: u32 = matches
        .iter()
        .flat_map(|recorded| &recorded.deals)
        .map(|deal| deal.deals - 1)
        .sum();
    assert!(
        redeals > 0,
        "the classic matches gather hands after an all-pass"
    );
    for (index, recorded) in matches.iter().enumerate() {
        replay(&format!("table-classic.jsonl:{}", index + 1), recorded);
    }
}

#[test]
fn contree_table_matches_match_java() {
    let matches = load("contree");
    assert_eq!(matches.len(), 4);
    for (index, recorded) in matches.iter().enumerate() {
        replay(&format!("table-contree.jsonl:{}", index + 1), recorded);
    }
}
