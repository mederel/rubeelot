//! The Challenging bot (US-056). It bids like the rule-based bot. To choose a card, it samples
//! deals of the cards it has not seen, consistent with its own hand, the cards played and the
//! voids shown; solves each sample for every legal card with the double-dummy solver; and plays
//! the card with the best average score (Perfect Information Monte Carlo). Early in the round,
//! where solving a sample takes too long, it plays like the rule-based bot.
//!
//! A strategy may be shared between tables: each decision draws its samples from a generator
//! seeded by the strategy's seed and the bot's view, so the same view gives the same card as long
//! as the time budget does not cut the search short, and it borrows a solver from a pool.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::BotStrategy;
use super::memory::CardMemory;
use super::rule_based::RuleBasedStrategy;
use super::solver::{DoubleDummySolver, Position};
use crate::bidding::{AuctionDecision, AuctionView};
use crate::board::{PlayView, SeatCard};
use crate::card::{Card, Rank, Seat, Suit, Variant};
use crate::rng::{RandomSource, SplitMix64};
use crate::scoring::{self, Declaration, Totals};

pub const DEFAULT_BUDGET: Duration = Duration::from_millis(200);
pub const DEFAULT_SAMPLES: usize = 50;
pub const DEFAULT_FIRST_SEARCHED_TRICK: usize = 3;
const DEAL_ATTEMPTS: usize = 50;
const GOLDEN_GAMMA: i64 = 0x9E3779B97F4A7C15_u64 as i64;

pub struct SamplingStrategy {
    rules: RuleBasedStrategy,
    seed: i64,
    samples: usize,
    /// `None` searches every sample, however long it takes.
    budget: Option<Duration>,
    first_searched_trick: usize,
    solvers: Mutex<Vec<DoubleDummySolver>>,
}

impl SamplingStrategy {
    /// 50 samples per card, within 200 ms, from the third trick onwards.
    pub fn new(seed: i64) -> Self {
        Self::with_settings(
            seed,
            DEFAULT_SAMPLES,
            Some(DEFAULT_BUDGET),
            DEFAULT_FIRST_SEARCHED_TRICK,
        )
    }

    /// A strategy that solves up to the given number of samples per card, within the time budget
    /// if any, from the given trick (numbered from 1) onwards.
    pub fn with_settings(
        seed: i64,
        samples: usize,
        budget: Option<Duration>,
        first_searched_trick: usize,
    ) -> Self {
        assert!(samples >= 1, "A bot needs at least one sample.");
        Self {
            rules: RuleBasedStrategy::new(),
            seed,
            samples,
            budget,
            first_searched_trick,
            solvers: Mutex::new(Vec::new()),
        }
    }

    fn search(
        &self,
        view: &PlayView,
        legal: &[Card],
        solver: &mut DoubleDummySolver,
        deadline: Option<Instant>,
    ) -> Card {
        let in_time = || deadline.is_none_or(|deadline| Instant::now() < deadline);
        let memory = CardMemory::new(view);
        let mut random = SplitMix64::new(self.seed ^ fingerprint(view));
        let mut totals = vec![0i64; legal.len()];
        let mut solved = 0;
        while solved < self.samples && in_time() {
            let Some(hands) = deal(view, &memory, &mut random) else {
                break;
            };
            let mut scores = vec![0i64; legal.len()];
            let mut complete = true;
            let mut index = 0;
            while index < legal.len() && complete {
                scores[index] = score(view, &hands, legal[index], solver);
                complete = in_time() || index == legal.len() - 1;
                index += 1;
            }
            if !complete {
                break;
            }
            for (total, score) in totals.iter_mut().zip(&scores) {
                *total = total.wrapping_add(*score);
            }
            solved += 1;
        }
        if solved == 0 {
            return self.rules.choose_card(view);
        }
        let mut best = 0;
        for index in 1..legal.len() {
            if totals[index] > totals[best] {
                best = index;
            }
        }
        legal[best]
    }
}

impl BotStrategy for SamplingStrategy {
    fn decide_auction(&self, view: &AuctionView) -> AuctionDecision {
        self.rules.decide_auction(view)
    }

    fn choose_card(&self, view: &PlayView) -> Card {
        let legal = &view.legal_cards;
        if legal.len() == 1 || view.tricks.len() + 1 < self.first_searched_trick {
            return self.rules.choose_card(view);
        }
        let deadline = self
            .budget
            .and_then(|budget| Instant::now().checked_add(budget));
        let pooled = self
            .solvers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        let mut solver = pooled.unwrap_or_default();
        let card = self.search(view, legal, &mut solver, deadline);
        self.solvers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(solver);
        card
    }
}

/// Deals the unseen cards to the other three seats: each seat first receives the cards it revealed
/// with its announcements and still holds, then as many more cards as it still holds, and no card
/// of a suit it has shown to be void in. The most constrained cards are dealt first. Returns
/// `None` when no consistent deal is found.
pub(crate) fn deal(
    view: &PlayView,
    memory: &CardMemory,
    random: &mut SplitMix64,
) -> Option<[Vec<Card>; 4]> {
    let mut capacity = [8i32; 4];
    for played in view.tricks.iter().flatten().chain(&view.current_trick) {
        capacity[played.seat] -= 1;
    }
    capacity[view.seat] = 0;
    let known: [Vec<Card>; 4] = std::array::from_fn(|seat| memory.known_cards(seat));
    for seat in 0..4 {
        capacity[seat] -= known[seat].len() as i32;
    }
    let mut unseen = memory.unseen_cards();
    unseen.retain(|card| !known.iter().any(|cards| cards.contains(card)));
    (0..DEAL_ATTEMPTS).find_map(|_| try_deal(view, memory, &known, &unseen, capacity, random))
}

fn try_deal(
    view: &PlayView,
    memory: &CardMemory,
    known: &[Vec<Card>; 4],
    unseen: &[Card],
    mut capacity: [i32; 4],
    random: &mut SplitMix64,
) -> Option<[Vec<Card>; 4]> {
    let mut hands: [Vec<Card>; 4] = Default::default();
    hands[view.seat].extend_from_slice(&view.hand);
    for seat in 0..4 {
        hands[seat].extend_from_slice(&known[seat]);
    }
    let mut cards = unseen.to_vec();
    for index in (1..cards.len()).rev() {
        let other = random.next_int(index as i32 + 1) as usize;
        cards.swap(index, other);
    }
    // A stable sort.
    cards.sort_by_key(|&card| allowed_seats(card, view, memory));
    for card in cards {
        let total: i32 = (0..4)
            .filter(|&seat| allows(seat, card, view, memory))
            .map(|seat| capacity[seat])
            .sum();
        if total == 0 {
            return None;
        }
        let mut pick = random.next_int(total);
        for seat in 0..4 {
            if !allows(seat, card, view, memory) {
                continue;
            }
            pick -= capacity[seat];
            if pick < 0 {
                hands[seat].push(card);
                capacity[seat] -= 1;
                break;
            }
        }
    }
    Some(hands)
}

fn allowed_seats(card: Card, view: &PlayView, memory: &CardMemory) -> usize {
    (0..4)
        .filter(|&seat| allows(seat, card, view, memory))
        .count()
}

fn allows(seat: Seat, card: Card, view: &PlayView, memory: &CardMemory) -> bool {
    seat != view.seat && !memory.shown_void(seat, card.suit)
}

/// The score difference the bot's team can force in the sample after playing the card.
fn score(
    view: &PlayView,
    hands: &[Vec<Card>; 4],
    card: Card,
    solver: &mut DoubleDummySolver,
) -> i64 {
    let mut remaining = hands.clone();
    let own = &mut remaining[view.seat];
    if let Some(index) = own.iter().position(|&held| held == card) {
        own.remove(index);
    }
    let mut tricks = view.tricks.clone();
    let mut trick = view.current_trick.clone();
    trick.push(SeatCard {
        seat: view.seat,
        card,
    });
    if trick.len() == 4 {
        tricks.push(trick);
        trick = Vec::new();
    }
    let solution = solver.solve(&Position {
        hands: remaining,
        trump: view.trump,
        tricks,
        current_trick: trick,
        first_leader: view.seat,
    });
    outcome(
        view,
        hands,
        solution.north_south_points,
        solution.east_west_points,
    )
}

/// The bot's team's score less the opponents', as the engine scores the round with these points
/// (US-060): the solved points include the dix de der and the belote, and the announcements that
/// scored are added. Neither the coinche, which doubles both scores, nor the capot, which the
/// solver does not count, changes the best card.
pub(crate) fn outcome(
    view: &PlayView,
    hands: &[Vec<Card>; 4],
    north_south: i32,
    east_west: i32,
) -> i64 {
    let belote = belote_points(view, hands);
    let mut announcements = [0i32; 2];
    for declaration in view
        .revealed_announcements
        .iter()
        .filter(|declaration| declaration.scored)
    {
        for announcement in &declaration.announcements {
            announcements[declaration.seat % 2] += announcement.value;
        }
    }
    // Four tricks each: the solver does not count tricks, so no capot.
    let result = scoring::score(
        &Totals {
            variant: view.variant,
            north_south_declares: view.declaring_seat.is_multiple_of(2),
            contract_value: if view.variant == Variant::Contree {
                view.contract_value
            } else {
                0
            },
            coinched: false,
            north_south_card_points: north_south - belote[0],
            east_west_card_points: east_west - belote[1],
            north_south_dix_de_der: 0,
            east_west_dix_de_der: 0,
            north_south_belote: belote[0],
            east_west_belote: belote[1],
            north_south_tricks: 4,
            north_south_announcements: announcements[0],
            east_west_announcements: announcements[1],
        },
        &[] as &[Declaration],
    );
    let difference = i64::from(result.north_south_awarded) - i64::from(result.east_west_awarded);
    if view.seat.is_multiple_of(2) {
        difference
    } else {
        -difference
    }
}

/// The belote points of each team in the sample: 20 for every seat dealt the king and queen of a
/// trump suit.
fn belote_points(view: &PlayView, hands: &[Vec<Card>; 4]) -> [i32; 2] {
    let mut points = [0; 2];
    for seat in 0..4 {
        let dealt: Vec<Card> = hands[seat]
            .iter()
            .copied()
            .chain(
                view.tricks
                    .iter()
                    .flatten()
                    .chain(&view.current_trick)
                    .filter(|played| played.seat == seat)
                    .map(|played| played.card),
            )
            .collect();
        for suit in Suit::ALL {
            if view.trump.is_trump(suit)
                && dealt.contains(&Card::new(Rank::King, suit))
                && dealt.contains(&Card::new(Rank::Queen, suit))
            {
                points[seat % 2] += 20;
            }
        }
    }
    points
}

/// A number identifying the bot's view that is the same in every run: it mixes the hash of each rank's
/// label, not an address or a randomly keyed hash.
fn fingerprint(view: &PlayView) -> i64 {
    let mut hash = 31i64
        .wrapping_mul(view.seat as i64)
        .wrapping_add(view.trump.ordinal() as i64);
    for &card in &view.hand {
        hash = mix(hash, card);
    }
    for played in view.tricks.iter().flatten().chain(&view.current_trick) {
        hash = mix(
            hash.wrapping_mul(7).wrapping_add(played.seat as i64),
            played.card,
        );
    }
    hash
}

fn mix(hash: i64, card: Card) -> i64 {
    hash.wrapping_mul(GOLDEN_GAMMA)
        ^ 31i64
            .wrapping_mul(i64::from(card.rank.label_hash()))
            .wrapping_add(card.suit.ordinal() as i64)
}

#[cfg(test)]
mod tests {
    //! The tests that need the module's internals; the others are in `tests/sampling.rs`.

    use std::collections::HashSet;

    use super::*;
    use crate::announcements::{Announcement, AnnouncementKind};
    use crate::bidding::BiddingState;
    use crate::board::GameBoard;
    use crate::bot::turns::{play_turn, take_auction_turn};
    use crate::card::{Suit, Trump, Variant};
    use crate::rng::SplitMix64;

    fn card(code: &str) -> Card {
        code.parse().unwrap()
    }

    fn cards(codes: &str) -> Vec<Card> {
        codes.split_whitespace().map(card).collect()
    }

    /// A seeded Contrée deal played by rule-based bots up to the given number of tricks, or
    /// `None` if redealt.
    fn board_after_tricks(seed: i64, tricks: usize) -> Option<GameBoard> {
        let rules = RuleBasedStrategy::new();
        let mut bidding = BiddingState::new(Variant::Contree, 0, Box::new(SplitMix64::new(seed)));
        while bidding.completed_board().is_none() && bidding.deals() == 1 {
            take_auction_turn(&mut bidding, &rules).unwrap();
        }
        let mut board = bidding.into_board()?;
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

    /// The cards the seat has played so far.
    fn played(view: &PlayView, seat: Seat) -> usize {
        view.tricks
            .iter()
            .flatten()
            .chain(&view.current_trick)
            .filter(|played| played.seat == seat)
            .count()
    }

    #[test]
    fn samples_deal_the_unseen_cards_consistently_with_what_the_bot_saw() {
        for seed in 1..=20 {
            let Some(board) = board_after_tricks(seed, 4) else {
                continue;
            };
            let view = board.play_view(board.active_seat());
            let memory = CardMemory::new(&view);

            let hands = deal(&view, &memory, &mut SplitMix64::new(seed)).expect("a deal");

            assert_eq!(hands[view.seat], view.hand);
            let mut dealt = HashSet::new();
            for (seat, hand) in hands.iter().enumerate() {
                assert_eq!(
                    hand.len(),
                    8 - played(&view, seat),
                    "cards held at seat {seat}"
                );
                for &card in hand {
                    assert!(dealt.insert(card), "dealt once: {card:?}");
                    if seat != view.seat {
                        assert!(memory.unseen(card), "not seen yet: {card:?}");
                        assert!(
                            !memory.shown_void(seat, card.suit),
                            "no card of a suit seat {seat} lacks"
                        );
                    }
                }
            }
            for card in &view.hand {
                dealt.remove(card);
            }
            assert_eq!(
                dealt,
                memory.unseen_cards().into_iter().collect::<HashSet<_>>()
            );
        }
    }

    #[test]
    fn an_unrepresentably_long_budget_means_no_deadline() {
        let board = (1..=20)
            .find_map(|seed| board_after_tricks(seed, 3))
            .expect("a board");
        let view = board.play_view(board.active_seat());
        let strategy = SamplingStrategy::with_settings(1, 2, Some(Duration::MAX), 1);
        assert!(view.legal_cards.contains(&strategy.choose_card(&view)));
    }

    fn spade_tierce() -> Announcement {
        Announcement {
            kind: AnnouncementKind::Tierce,
            cards: cards("JS QS KS"),
            value: 20,
            suit: Some(Suit::Spades),
        }
    }

    fn declaration(seat: Seat, scored: bool, announcement: Announcement) -> Declaration {
        Declaration {
            seat,
            announcements: vec![announcement],
            scored,
        }
    }

    /// A completed trick led from the given seat, its cards in play order.
    fn seat_cards(leader: Seat, codes: &str) -> Vec<SeatCard> {
        cards(codes)
            .into_iter()
            .enumerate()
            .map(|(index, card)| SeatCard {
                seat: (leader + index) % 4,
                card,
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn view(
        seat: Seat,
        hand: Vec<Card>,
        variant: Variant,
        contract_value: i32,
        tricks: Vec<Vec<SeatCard>>,
        announcements: bool,
        revealed: Vec<Declaration>,
    ) -> PlayView {
        PlayView {
            seat,
            hand: hand.clone(),
            legal_cards: hand,
            trump: Trump::Hearts,
            variant,
            contract_value,
            coinched: false,
            declaring_seat: 0,
            tricks,
            current_trick: Vec::new(),
            announcements,
            revealed_announcements: revealed,
        }
    }

    #[test]
    fn deals_revealed_cards_to_their_owners() {
        let tricks = vec![seat_cards(1, "JS 7S 8S 9S"), seat_cards(1, "7D 8D 9D 10D")];
        let view = view(
            0,
            cards("AC KC 10C AD AH KH"),
            Variant::Classic,
            0,
            tricks,
            true,
            vec![declaration(1, false, spade_tierce())],
        );
        let memory = CardMemory::new(&view);

        for sample in 0..200 {
            let hands = deal(&view, &memory, &mut SplitMix64::new(sample)).expect("a deal");
            assert!(
                hands[1].contains(&card("QS")) && hands[1].contains(&card("KS")),
                "{hands:?}"
            );
            assert!(hands.iter().all(|hand| hand.len() == 6), "{hands:?}");
        }
    }

    const NO_HANDS: [Vec<Card>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];

    #[test]
    fn scores_a_failed_contract_as_the_defenders_winning_everything() {
        let view = view(
            1,
            Vec::new(),
            Variant::Contree,
            100,
            Vec::new(),
            false,
            Vec::new(),
        );

        assert_eq!(
            outcome(&view, &NO_HANDS, 90, 72),
            162,
            "the declarers made 90 of their 100"
        );
        assert_eq!(
            outcome(&view, &NO_HANDS, 110, 52),
            52 - 110,
            "the declarers made their contract"
        );
    }

    /// North (seat 0) took the contract and is the bot.
    fn north_view(variant: Variant, contract: i32, revealed: Vec<Declaration>) -> PlayView {
        view(0, Vec::new(), variant, contract, Vec::new(), true, revealed)
    }

    #[test]
    fn outcome_matches_round_scoring() {
        let north_belote = [cards("KH QH"), Vec::new(), Vec::new(), Vec::new()];
        let classic = north_view(Variant::Classic, 0, Vec::new());

        assert_eq!(outcome(&classic, &NO_HANDS, 82, 80), 2, "made");
        assert_eq!(outcome(&classic, &NO_HANDS, 81, 81), -162, "a tie fails");
        assert_eq!(
            outcome(&classic, &north_belote, 92, 70),
            22,
            "made thanks to the Belote"
        );
        let contree = north_view(
            Variant::Contree,
            100,
            vec![declaration(0, true, spade_tierce())],
        );
        assert_eq!(
            outcome(&contree, &NO_HANDS, 90, 72),
            110 - 72,
            "made thanks to the tierce"
        );
    }

    #[test]
    fn unscored_declarations_add_nothing() {
        let view = north_view(
            Variant::Classic,
            0,
            vec![declaration(0, false, spade_tierce())],
        );

        assert_eq!(outcome(&view, &NO_HANDS, 81, 81), -162);
    }

    #[test]
    fn the_fingerprint_mixes_the_label_hash_of_the_ranks() {
        // 31 × seat + trump, then (hash × golden gamma) ^ (31 × the hash of "A" + clubs).
        let view = view(
            1,
            cards("AC"),
            Variant::Classic,
            0,
            Vec::new(),
            false,
            Vec::new(),
        );
        let start: i64 = 31 + 2;

        assert_eq!(
            fingerprint(&view),
            start.wrapping_mul(0x9E3779B97F4A7C15_u64 as i64) ^ (31 * 65)
        );
    }
}
