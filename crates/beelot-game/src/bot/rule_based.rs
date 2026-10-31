//! The bot players meet in the application: hand-written bidding rules (US-050, US-051, US-055,
//! US-064) and card-play rules (US-053, US-054). The first of equal maximums or minimums wins a tie.

use std::cmp::Ordering;

use super::BotStrategy;
use super::hand_features::{self, Feature};
use super::memory::CardMemory;
use crate::announcements;
use crate::bidding::{AuctionDecision, AuctionView};
use crate::board::{PlayView, SeatCard};
use crate::card::{Card, Rank, Seat, Suit, Trump, Variant};
use crate::rules;

const MAX_BID: i32 = 160;
pub(crate) const TAKING_THRESHOLD: i32 = 6;
pub(crate) const BIDDING_MARGIN: i32 = 15;
pub(crate) const ALL_TRUMPS_MARGIN: i32 = 25;
const ALL_TRUMPS_INTERCEPT: i32 = 58;
const ALL_TRUMPS_WEIGHTS: [i32; 6] = [15, 15, 6, 2, 1, 19];
const NO_TRUMPS_INTERCEPT: i32 = 58;
const NO_TRUMPS_WEIGHTS: [i32; 5] = [15, 11, 4, 5, 11];
const ALL_TRUMPS_DEFENCE_INTERCEPT: i32 = 203;
const ALL_TRUMPS_DEFENCE_WEIGHTS: [i32; 3] = [-1, 13, 3];
const ALL_TRUMPS_CONTRACT_SLOPE: f64 = 1.84;
const NO_TRUMPS_DEFENCE_INTERCEPT: i32 = 235;
const NO_TRUMPS_DEFENCE_WEIGHTS: [i32; 3] = [24, -2, 2];
const NO_TRUMPS_CONTRACT_SLOPE: f64 = 2.29;
pub(crate) const LOW_VALUE_TRICK: i32 = 15;
pub(crate) const COINCHE_MARGIN: i32 = -4;
pub(crate) const ANNOUNCEMENT_WEIGHT: f64 = 0.75;
pub(crate) const ANNOUNCEMENT_STEP: i32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleBasedStrategy {
    taking_threshold: i32,
    bidding_margin: i32,
    trump_control: bool,
    all_trumps_tactics: bool,
}

impl Default for RuleBasedStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleBasedStrategy {
    pub fn new() -> Self {
        Self::with_settings(TAKING_THRESHOLD, BIDDING_MARGIN, true, true)
    }

    /// The same bidding, with the card play of US-053: the bot does not manage its trumps
    /// (US-054).
    pub fn without_trump_control() -> Self {
        Self::with_settings(TAKING_THRESHOLD, BIDDING_MARGIN, false, true)
    }

    /// The same bidding, with the all trumps play of US-063: the trump tactics treat every card as
    /// a trump, so the bot holds back the jack and nine of every suit and never cashes a master
    /// card in defence.
    pub fn without_all_trumps_tactics() -> Self {
        Self::with_settings(TAKING_THRESHOLD, BIDDING_MARGIN, true, false)
    }

    fn with_settings(
        taking_threshold: i32,
        bidding_margin: i32,
        trump_control: bool,
        all_trumps_tactics: bool,
    ) -> Self {
        Self {
            taking_threshold,
            bidding_margin,
            trump_control,
            all_trumps_tactics,
        }
    }

    /// Supports its partner's suit bid once, in the same suit (US-035). Otherwise bids at the
    /// level of its best estimate less a safety margin, rounded down to a multiple of 10: it opens
    /// when that level reaches 80, and overcalls an opponent's contract when that level is higher.
    /// When it does not overcall, it coinches an opponent's contract it expects to defeat
    /// (US-055). When the table plays all trumps and no trumps (US-064), the bot weighs them too,
    /// with their own margin: they swing more than a suit contract. The suits come first, so a tie
    /// keeps the suit. When the round plays with announcements, the contract estimate adds a share
    /// of the bot's own announcements (US-061); the coinche leaves them out.
    fn contree_decision(&self, view: &AuctionView) -> AuctionDecision {
        if view.partner_holds_contract() {
            // Raising all trumps and no trumps lost points in the arena (US-064), so the bot
            // leaves them alone.
            let supported = view
                .highest_bid_suit
                .expect("a contract held by the partner has a trump");
            let raise = match supported.suit() {
                Some(suit) if !view.has_bid() => support_raise(&view.hand, suit),
                _ => 0,
            };
            if raise > 0 && view.highest_bid < MAX_BID {
                return AuctionDecision::Bid {
                    value: (view.highest_bid + raise).min(MAX_BID),
                    trump: supported,
                };
            }
            return AuctionDecision::Pass;
        }
        let mut best = Trump::Clubs;
        let mut best_value = i32::MIN;
        for contract in Trump::ALL {
            if !contract.suit_contract() && !view.all_trumps {
                continue;
            }
            let margin = if contract.suit_contract() {
                self.bidding_margin
            } else {
                ALL_TRUMPS_MARGIN
            };
            let value = contract_estimate(&view.hand, contract)
                + announcement_bonus(view, contract)
                - margin;
            if value > best_value {
                best = contract;
                best_value = value;
            }
        }
        let level = MAX_BID.min(best_value.div_euclid(10) * 10);
        if level >= 80 && level > view.highest_bid {
            return AuctionDecision::Bid {
                value: level,
                trump: best,
            };
        }
        if view.coinche_allowed && expects_to_defeat(view) {
            AuctionDecision::Coinche
        } else {
            AuctionDecision::Pass
        }
    }

    /// In the first round, accepts the upturned suit when its hand scores at least the threshold
    /// in that suit. In the second round, chooses its best suit other than the upturned one when
    /// that suit reaches the threshold. When the round plays with announcements, the hand scores
    /// one more point per 50 points of its announcements (US-061).
    fn classic_decision(&self, view: &AuctionView) -> AuctionDecision {
        let upturned_card = view
            .upturned_card
            .expect("a Classic auction has an upturned card");
        let upturned = upturned_card.suit;
        let mut hand = view.hand.clone();
        hand.push(upturned_card);
        let bonus = if view.announcements {
            announcement_points(&hand, upturned.into()) / ANNOUNCEMENT_STEP
        } else {
            0
        };
        if view.round == 1 {
            return if hand_score(&hand, upturned) + bonus >= self.taking_threshold {
                AuctionDecision::Trump { suit: upturned }
            } else {
                AuctionDecision::Pass
            };
        }
        let mut best = None;
        let mut best_score = -1;
        for suit in Suit::ALL {
            let score = hand_score(&hand, suit);
            if suit != upturned && score > best_score {
                best = Some(suit);
                best_score = score;
            }
        }
        match best {
            Some(suit) if best_score + bonus >= self.taking_threshold => {
                AuctionDecision::Trump { suit }
            }
            _ => AuctionDecision::Pass,
        }
    }

    fn lead(&self, view: &PlayView, memory: &CardMemory, candidates: &[Card]) -> Card {
        let trump = view.trump;
        if self.trump_control
            && view.declaring()
            && opponents_may_hold_trumps(view, memory)
            && let Some(&master_trump) = candidates
                .iter()
                .find(|card| trump.is_trump(card.suit) && memory.master(**card))
        {
            return master_trump;
        }
        let master = first_max(
            candidates.iter().copied().filter(|card| {
                !self.trump_tactics(*card, trump) && holds(*card, card.suit, 3, view, memory)
            }),
            by_value(trump),
        );
        if let Some(master) = master {
            return master;
        }
        let leads = preferring(candidates, |card| !self.trump_tactics(*card, trump));
        let leads = preferring(&leads, |card| !ten_without_ace(card.suit, view, memory));
        self.lowest(&leads, trump, memory)
    }

    fn follow(&self, view: &PlayView, memory: &CardMemory, candidates: &[Card]) -> Card {
        let trump = view.trump;
        let trick = &view.current_trick;
        let lead = trick[0].card.suit;
        let trick_cards: Vec<Card> = trick.iter().map(|played| played.card).collect();
        let winner = trick[rules::winning_index(&trick_cards, trump)];
        let players_after = 3 - trick.len();
        let partner_wins = winner.seat % 2 == view.seat % 2;
        if partner_wins && holds(winner.card, lead, players_after, view, memory) {
            return first_max(
                candidates
                    .iter()
                    .copied()
                    .filter(|card| !self.trump_tactics(*card, trump) && !memory.master(*card)),
                by_value(trump),
            )
            .unwrap_or_else(|| self.lowest(candidates, trump, memory));
        }
        let mut winning: Vec<Card> = candidates
            .iter()
            .copied()
            .filter(|card| rules::beats(*card, winner.card, lead, trump))
            .collect();
        if self.trump_control && trick_points(trick, trump) < LOW_VALUE_TRICK {
            winning.retain(|card| !self.trump_honour(*card, trump));
        }
        let sure = first_min(
            winning
                .iter()
                .copied()
                .filter(|card| holds(*card, lead, players_after, view, memory)),
            by_value(trump),
        );
        if let Some(sure) = sure {
            return sure;
        }
        if !partner_wins && let Some(cheapest) = first_min(winning.iter().copied(), by_value(trump))
        {
            return cheapest;
        }
        self.lowest(candidates, trump, memory)
    }

    /// The jack and nine of trumps, which a bot keeps to regain the lead.
    fn trump_honour(&self, card: Card, trump: Trump) -> bool {
        self.trump_tactics(card, trump) && (card.rank == Rank::Jack || card.rank == Rank::Nine)
    }

    /// Whether the trump tactics set the card apart as a trump: only in a suit contract, since
    /// under all trumps every card is a trump. Without the tactics of US-064, every trump is set
    /// apart, as in US-063.
    fn trump_tactics(&self, card: Card, trump: Trump) -> bool {
        (trump.suit_contract() || !self.all_trumps_tactics) && trump.is_trump(card.suit)
    }

    /// The lowest-value card, keeping masters and trumps when it can.
    fn lowest(&self, cards: &[Card], trump: Trump, memory: &CardMemory) -> Card {
        let spare = preferring(cards, |card| !self.trump_tactics(*card, trump));
        let spare = preferring(&spare, |card| !memory.master(*card));
        first_min(spare, by_value(trump)).expect("a seat to play has a legal card")
    }
}

impl BotStrategy for RuleBasedStrategy {
    /// In classic Belote, takes when its hand is strong enough in a suit it may choose. In
    /// Contrée, bids what its hand is worth and supports its partner's bid.
    fn decide_auction(&self, view: &AuctionView) -> AuctionDecision {
        if view.variant == Variant::Contree {
            self.contree_decision(view)
        } else {
            self.classic_decision(view)
        }
    }

    /// Plays to win tricks cheaply and to score them (US-053), and manages its trumps (US-054).
    /// Keeps trumps out of the first trick when defending. When leading on the declaring team,
    /// leads its master trump while an opponent may still hold trumps. Otherwise, when leading,
    /// cashes a master card no opponent can beat, or else leads a low card, avoiding trumps and
    /// suits in which it holds the ten without the ace. When following, adds its highest-value
    /// card that it does not need later (neither a trump nor a master) to a trick its partner is
    /// sure to win. Otherwise it plays its cheapest card sure to win the trick; it only overtakes
    /// its partner with such a card. Against the opponents, it falls back on its cheapest winning
    /// card, then on its lowest-value card. It keeps its jack and nine of trumps to regain the
    /// lead rather than win a trick worth less than 15 points with them. These trump tactics apply
    /// to a suit contract: under all trumps every card is played like a plain card (US-064).
    fn choose_card(&self, view: &PlayView) -> Card {
        let trump = view.trump;
        let memory = CardMemory::new(view);
        let mut candidates = view.legal_cards.clone();
        if view.tricks.is_empty() && !view.declaring() {
            candidates = preferring(&candidates, |card| !self.trump_tactics(*card, trump));
        }
        if view.current_trick.is_empty() {
            self.lead(view, &memory, &candidates)
        } else {
            self.follow(view, &memory, &candidates)
        }
    }
}

/// The points the bot's team can expect to make in Contrée with the given contract, fitted on bot
/// play over random deals (US-064). A suit contract uses [`suit_estimate`]. All trumps: 58, plus
/// 15 for each jack, 15 for each nine with its jack, 6 for each other nine, 2 for each ace, 1 for
/// each ten and 19 for each Belote (king and queen of a suit). No trumps: 58, plus 15 for each
/// ace, 11 for each ten with its ace, 4 for each other ten, 5 for each king with the ace and ten,
/// and 11 for each card beyond the fourth in a suit headed by the ace.
pub(crate) fn contract_estimate(hand: &[Card], trump: Trump) -> i32 {
    match trump {
        Trump::AllTrumps => weighted(
            hand,
            hand_features::offence(trump),
            ALL_TRUMPS_INTERCEPT,
            &ALL_TRUMPS_WEIGHTS,
        ),
        Trump::NoTrumps => weighted(
            hand,
            hand_features::offence(trump),
            NO_TRUMPS_INTERCEPT,
            &NO_TRUMPS_WEIGHTS,
        ),
        suit => suit_estimate(hand, suit.suit().expect("a suit contract has a suit")),
    }
}

/// The intercept plus each feature's count times its weight.
fn weighted(hand: &[Card], features: &[Feature], intercept: i32, weights: &[i32]) -> i32 {
    intercept
        + features
            .iter()
            .zip(weights)
            .map(|(feature, weight)| weight * (feature.count)(hand))
            .sum::<i32>()
}

/// The points the bot's team can expect to make in Contrée with the given trump suit, fitted on
/// bot play over random deals: 50, plus 7 for each trump, 22 for the jack of trumps, 10 for the
/// nine, 6 for the ace of trumps, and 8 for each ace in another suit.
pub(crate) fn suit_estimate(hand: &[Card], trump: Suit) -> i32 {
    let mut estimate = 50;
    for card in hand {
        if card.suit == trump {
            estimate += 7;
            if card.rank == Rank::Jack {
                estimate += 22;
            }
            if card.rank == Rank::Nine {
                estimate += 10;
            }
            if card.rank == Rank::Ace {
                estimate += 6;
            }
        } else if card.rank == Rank::Ace {
            estimate += 8;
        }
    }
    estimate
}

/// Whether the bot's team is expected to keep the declarers below their contract: its defensive
/// estimate comes within 4 points of the 162 − contract points that defeat it.
fn expects_to_defeat(view: &AuctionView) -> bool {
    let defeating_points = 162 - view.highest_bid;
    let trump = view
        .highest_bid_suit
        .expect("a contract that may be coinched has a trump");
    defensive_estimate(&view.hand, trump, view.highest_bid) >= defeating_points + COINCHE_MARGIN
}

/// The share of the bot's announcements its Contrée contract estimate counts, when the round
/// plays with them.
fn announcement_bonus(view: &AuctionView, trump: Trump) -> i32 {
    if view.announcements {
        round_half_up(ANNOUNCEMENT_WEIGHT * f64::from(announcement_points(&view.hand, trump)))
    } else {
        0
    }
}

/// The total value of the announcements the hand would declare (US-060).
pub(crate) fn announcement_points(hand: &[Card], trump: Trump) -> i32 {
    announcements::find(hand, trump)
        .iter()
        .map(|announcement| announcement.value)
        .sum()
}

/// The defence's prospects against the given contract. A suit contract uses
/// [`suit_defensive_estimate`]. Against all trumps and no trumps, the estimate is 162 less the
/// taker's expected total (cards, dix de der, Belote and announcements), fitted on bot play over
/// random deals (US-064), so it can be compared with the same 162 − contract. All trumps: 203,
/// plus 13 for each jack and 3 for each nine, less 1 for each ten and 1.84 for each point of the
/// contract. No trumps: 235, plus 24 for each ace and 2 for each jack, less 2 for each ten and
/// 2.29 for each point of the contract. A higher bid shows a stronger hand.
pub(crate) fn defensive_estimate(hand: &[Card], trump: Trump, contract: i32) -> i32 {
    let (intercept, weights, slope) = match trump {
        Trump::AllTrumps => (
            ALL_TRUMPS_DEFENCE_INTERCEPT,
            ALL_TRUMPS_DEFENCE_WEIGHTS,
            ALL_TRUMPS_CONTRACT_SLOPE,
        ),
        Trump::NoTrumps => (
            NO_TRUMPS_DEFENCE_INTERCEPT,
            NO_TRUMPS_DEFENCE_WEIGHTS,
            NO_TRUMPS_CONTRACT_SLOPE,
        ),
        suit => return suit_defensive_estimate(hand, suit.suit(), contract),
    };
    let cards = weighted(hand, hand_features::defence(trump), intercept, &weights);
    round_half_up(f64::from(cards) - slope * f64::from(contract))
}

/// The points the bot's team can expect to make in defence against the given suit contract,
/// fitted on bot play over random deals: 150, plus 6 for each trump, 9 for the jack of trumps, 7 for the
/// nine, 4 for each ace and 3 for each ten in another suit (every suit when the contract has no
/// trump suit), less 1.3 for each point of the contract, since a higher bid shows a stronger
/// hand.
pub(crate) fn suit_defensive_estimate(hand: &[Card], trump: Option<Suit>, contract: i32) -> i32 {
    let mut estimate = 150.0 - 1.3 * f64::from(contract);
    for card in hand {
        if Some(card.suit) == trump {
            estimate += 6.0;
            if card.rank == Rank::Jack {
                estimate += 9.0;
            }
            if card.rank == Rank::Nine {
                estimate += 7.0;
            }
        } else if card.rank == Rank::Ace {
            estimate += 4.0;
        } else if card.rank == Rank::Ten {
            estimate += 3.0;
        }
    }
    round_half_up(estimate)
}

/// The closest integer, halves rounded up (towards positive infinity). The estimates here are far from the `int` range.
fn round_half_up(value: f64) -> i32 {
    let floor = value.floor();
    let rounded = if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    };
    rounded as i32
}

/// How strong a classic hand is with the given trump suit, counting the upturned card the taker
/// receives: 3 for the jack of trumps, 2 for the nine, 1 for each trump, 1 for each ace in another
/// suit, and 1 for the belote (king and queen of trumps).
pub(crate) fn hand_score(hand: &[Card], trump: Suit) -> i32 {
    let mut score = 0;
    for card in hand {
        if card.suit == trump {
            score += 1;
            if card.rank == Rank::Jack {
                score += 3;
            }
            if card.rank == Rank::Nine {
                score += 2;
            }
        } else if card.rank == Rank::Ace {
            score += 1;
        }
    }
    if hand.contains(&Card::new(Rank::King, trump)) && hand.contains(&Card::new(Rank::Queen, trump))
    {
        score += 1;
    }
    score
}

/// How much a bot raises its partner's bid in the given suit: 20 with the jack of that suit, or
/// with two aces elsewhere and at least one card of that suit; 10 with an ace elsewhere and the
/// nine of that suit, or with two aces elsewhere and no card of that suit; otherwise 0.
pub fn support_raise(hand: &[Card], suit: Suit) -> i32 {
    let jack = hand.contains(&Card::new(Rank::Jack, suit));
    let nine = hand.contains(&Card::new(Rank::Nine, suit));
    let other_aces = hand
        .iter()
        .filter(|card| card.rank == Rank::Ace && card.suit != suit)
        .count();
    let suit_cards = hand.iter().filter(|card| card.suit == suit).count();
    if jack || (other_aces >= 2 && suit_cards > 0) {
        return 20;
    }
    if (other_aces >= 1 && nine) || other_aces >= 2 {
        return 10;
    }
    0
}

/// Whether the card, winning a trick led in the given suit, stays the winner: none of the
/// opponents among the given number of seats still to play after the bot may hold a card that
/// beats it.
fn holds(
    winner: Card,
    lead: Suit,
    players_after: usize,
    view: &PlayView,
    memory: &CardMemory,
) -> bool {
    let mut offset = 1;
    while offset <= players_after {
        let seat: Seat = (view.seat + offset) % 4;
        if memory.may_beat(seat, winner, lead) {
            return false;
        }
        offset += 2;
    }
    true
}

/// Whether an opponent who has not shown a void in trumps may still hold one; never under no
/// trumps.
fn opponents_may_hold_trumps(view: &PlayView, memory: &CardMemory) -> bool {
    memory.may_hold_trump((view.seat + 1) % 4) || memory.may_hold_trump((view.seat + 3) % 4)
}

fn trick_points(trick: &[SeatCard], trump: Trump) -> i32 {
    trick
        .iter()
        .map(|played| rules::points(played.card, trump))
        .sum()
}

/// Whether the bot holds the ten of the suit while the ace is still in another hand.
fn ten_without_ace(suit: Suit, view: &PlayView, memory: &CardMemory) -> bool {
    !view.trump.is_trump(suit)
        && view.hand.contains(&Card::new(Rank::Ten, suit))
        && memory.unseen(Card::new(Rank::Ace, suit))
}

/// Orders cards by points, then by strength within their suit.
pub(crate) fn by_value(trump: Trump) -> impl Fn(&Card, &Card) -> Ordering + Copy {
    move |a, b| {
        rules::points(*a, trump)
            .cmp(&rules::points(*b, trump))
            .then_with(|| rules::strength(*a, trump).cmp(&rules::strength(*b, trump)))
    }
}

/// The first of the equal maximums.
fn first_max(
    cards: impl IntoIterator<Item = Card>,
    compare: impl Fn(&Card, &Card) -> Ordering,
) -> Option<Card> {
    cards.into_iter().reduce(|best, card| {
        if compare(&best, &card).is_ge() {
            best
        } else {
            card
        }
    })
}

/// The first of the equal minimums.
pub(crate) fn first_min(
    cards: impl IntoIterator<Item = Card>,
    compare: impl Fn(&Card, &Card) -> Ordering,
) -> Option<Card> {
    cards.into_iter().reduce(|best, card| {
        if compare(&best, &card).is_le() {
            best
        } else {
            card
        }
    })
}

/// The cards that match, or all of them when none does.
pub(crate) fn preferring(cards: &[Card], preferred: impl Fn(&Card) -> bool) -> Vec<Card> {
    let matching: Vec<Card> = cards
        .iter()
        .copied()
        .filter(|card| preferred(card))
        .collect();
    if matching.is_empty() {
        cards.to_vec()
    } else {
        matching
    }
}

#[cfg(test)]
mod tests {
    //! The bot sits in seat 1; its partner is seat 3.

    use super::*;
    use crate::bidding::{AuctionDecision, AuctionView, BiddingState};
    use crate::bot::BotStrategy;
    use crate::bot::turns::take_auction_turn;
    use crate::card::{Card, Seat, Suit, Variant};
    use crate::rng::SplitMix64;

    const BID_SUIT: Suit = Suit::Hearts;

    fn card(code: &str) -> Card {
        code.parse().unwrap()
    }

    fn cards(codes: &str) -> Vec<Card> {
        codes.split_whitespace().map(card).collect()
    }

    fn contree_view(
        highest_bid: i32,
        suit: Option<Trump>,
        bidder: Option<Seat>,
        announcements: bool,
        hand: &str,
    ) -> AuctionView {
        AuctionView {
            seat: 1,
            hand: cards(hand),
            upturned_card: None,
            round: 1,
            variant: Variant::Contree,
            dealer_index: 0,
            auction: Vec::new(),
            highest_bid,
            highest_bid_suit: suit,
            highest_bidder_seat: bidder,
            coinche_allowed: false,
            announcements,
            all_trumps: false,
        }
    }

    /// An opponent at seat 0 holds the given hearts contract, and the bot at seat 1 may coinche it.
    fn coinche_view(highest_bid: i32, announcements: bool, hand: &str) -> AuctionView {
        AuctionView {
            coinche_allowed: true,
            ..contree_view(
                highest_bid,
                Some(BID_SUIT.into()),
                Some(0),
                announcements,
                hand,
            )
        }
    }

    fn classic_view(round: i32, upturned: &str, announcements: bool, hand: &str) -> AuctionView {
        AuctionView {
            seat: 1,
            hand: cards(hand),
            upturned_card: Some(card(upturned)),
            round,
            variant: Variant::Classic,
            dealer_index: 0,
            auction: Vec::new(),
            highest_bid: 0,
            highest_bid_suit: None,
            highest_bidder_seat: None,
            coinche_allowed: false,
            announcements,
            all_trumps: false,
        }
    }

    /// Jack, nine, ace and ten of spades, with the aces of hearts and clubs.
    const STRONG_SPADES: &str = "JS 9S AS 10S AH AC 7D 8D";

    /// A carré of aces and no sequence; with spades as trump it is worth 111 points without the
    /// carré.
    const CARRE_OF_ACES: &str = "AS AH AC AD 7S 9S 8H 10D";

    fn decide(view: &AuctionView) -> AuctionDecision {
        RuleBasedStrategy::new().decide_auction(view)
    }

    /// The first seat to speak is seat 0.
    fn contree_bidding(seed: i64) -> BiddingState {
        BiddingState::new(Variant::Contree, 3, Box::new(SplitMix64::new(seed)))
    }

    #[test]
    fn raises_by_twenty_with_the_jack_of_the_bid_suit() {
        assert_eq!(support_raise(&cards("JH 7C"), BID_SUIT), 20);
    }

    #[test]
    fn raises_by_twenty_with_two_aces_elsewhere_and_some_cards_of_the_bid_suit() {
        assert_eq!(support_raise(&cards("AC AS 7H 8H"), BID_SUIT), 20);
    }

    #[test]
    fn raises_by_ten_with_an_ace_elsewhere_and_the_nine_of_the_bid_suit() {
        assert_eq!(support_raise(&cards("AC 9H"), BID_SUIT), 10);
    }

    #[test]
    fn raises_by_ten_with_two_aces_elsewhere_and_no_card_of_the_bid_suit() {
        assert_eq!(support_raise(&cards("AC AS 7D"), BID_SUIT), 10);
    }

    #[test]
    fn does_not_raise_without_support() {
        assert_eq!(support_raise(&cards("AC AH 10H"), BID_SUIT), 0);
        assert_eq!(support_raise(&cards("9H KC"), BID_SUIT), 0);
    }

    /// Several seeds, so that both a raise and a pass are exercised.
    #[test]
    fn bot_supports_its_partners_bid_in_the_same_suit() {
        let mut raised = false;
        let mut passed = false;
        for seed in 0..16 {
            let mut bidding = contree_bidding(seed);
            bidding.bid(0, 80, BID_SUIT.into()).unwrap();
            bidding.pass(1).unwrap();
            let raise = support_raise(bidding.hand(2), BID_SUIT);

            take_auction_turn(&mut bidding, &RuleBasedStrategy::new()).unwrap();

            assert_eq!(bidding.highest_bid(), 80 + raise, "seed {seed}");
            assert_eq!(
                bidding.highest_bid_suit(),
                Some(BID_SUIT.into()),
                "seed {seed}"
            );
            assert_eq!(
                bidding.highest_bidder(),
                Some(if raise > 0 { 2 } else { 0 }),
                "seed {seed}"
            );
            raised |= raise > 0;
            passed |= raise == 0;
        }
        assert!(raised && passed);
    }

    #[test]
    fn bot_does_not_support_an_opponents_bid() {
        let view = contree_view(
            80,
            Some(BID_SUIT.into()),
            Some(0),
            false,
            "JH 7C 8C 9C 7D 8D 7S 8S",
        );

        assert_eq!(
            decide(&view),
            AuctionDecision::Pass,
            "the jack of the opponent's suit would support a partner, not an opponent"
        );
    }

    #[test]
    fn bot_that_already_bid_does_not_raise_its_partners_support() {
        let mut bidding = contree_bidding(1);
        bidding.bid(0, 80, BID_SUIT.into()).unwrap();
        bidding.pass(1).unwrap();
        bidding.bid(2, 100, BID_SUIT.into()).unwrap();
        bidding.pass(3).unwrap();

        take_auction_turn(&mut bidding, &RuleBasedStrategy::new()).unwrap();

        assert_eq!(bidding.highest_bid(), 100);
    }

    #[test]
    fn bot_raise_is_capped_at_the_maximum_contract() {
        for seed in 0..16 {
            let mut bidding = contree_bidding(seed);
            bidding.bid(0, 150, BID_SUIT.into()).unwrap();
            bidding.pass(1).unwrap();
            let raise = support_raise(bidding.hand(2), BID_SUIT);

            take_auction_turn(&mut bidding, &RuleBasedStrategy::new()).unwrap();

            assert_eq!(
                bidding.highest_bid(),
                if raise > 0 { 160 } else { 150 },
                "seed {seed}"
            );
        }
    }

    #[test]
    fn scores_a_classic_hand_from_its_trump_honours_trumps_aces_and_belote() {
        let trump = Suit::Spades;
        assert_eq!(
            hand_score(&cards("JS 9S 7C"), trump),
            4 + 3,
            "jack 3 + 1, nine 2 + 1, nothing for a low side card"
        );
        assert_eq!(
            hand_score(&cards("KS QS AH AC"), trump),
            1 + 1 + 1 + 1 + 1,
            "two trumps, two aces, belote"
        );
        assert_eq!(hand_score(&cards("KH 8C"), trump), 0);
        assert_eq!(
            hand_score(&cards("AS"), trump),
            1,
            "a trump ace counts as a trump"
        );
    }

    #[test]
    fn accepts_the_upturned_suit_in_the_first_round_with_a_strong_hand() {
        let view = classic_view(1, "7S", false, "JS 9S 8C 7H 8H");

        assert_eq!(
            decide(&view),
            AuctionDecision::Trump { suit: Suit::Spades },
            "jack and nine of trumps plus the upturned seven score 4 + 3 + 1"
        );
    }

    #[test]
    fn passes_in_the_first_round_below_the_threshold() {
        let view = classic_view(1, "7S", false, "JS 8C 7H 8H 9C");

        assert_eq!(
            decide(&view),
            AuctionDecision::Pass,
            "jack and one more trump: 5"
        );
    }

    #[test]
    fn chooses_its_best_other_suit_in_the_second_round() {
        let view = classic_view(2, "JS", false, "JH 9H AC QS KS");

        assert_eq!(
            decide(&view),
            AuctionDecision::Trump { suit: Suit::Hearts },
            "hearts score 4 + 3 + 1 for the ace; spades, the upturned suit, may not be chosen"
        );
    }

    #[test]
    fn passes_in_the_second_round_when_no_other_suit_is_strong_enough() {
        let view = classic_view(2, "JS", false, "9S AS 8C 7H 10D");

        assert_eq!(decide(&view), AuctionDecision::Pass);
    }

    #[test]
    fn estimates_the_contract_from_trumps_trump_honours_and_side_aces() {
        assert_eq!(
            suit_estimate(&cards(STRONG_SPADES), Suit::Spades),
            50 + 4 * 7 + 22 + 10 + 6 + 2 * 8
        );
        assert_eq!(
            suit_estimate(&cards(STRONG_SPADES), Suit::Diamonds),
            50 + 2 * 7 + 3 * 8,
            "diamonds: two low trumps and three aces in other suits"
        );
    }

    #[test]
    fn opens_at_the_level_of_its_estimate() {
        let view = contree_view(0, None, None, false, STRONG_SPADES);

        assert_eq!(
            decide(&view),
            AuctionDecision::Bid {
                value: 110,
                trump: Trump::Spades
            },
            "an estimate of 132 less the margin of 15, rounded down to 110"
        );
    }

    #[test]
    fn passes_instead_of_opening_with_a_weak_hand() {
        let view = contree_view(0, None, None, false, "JS 7S AH 8C 9C 7D 8D 8H");

        assert_eq!(
            decide(&view),
            AuctionDecision::Pass,
            "spades estimate 50 + 14 + 22 + 8 = 94, below 80 after the margin"
        );
    }

    #[test]
    fn overcalls_an_opponents_lower_contract() {
        let view = contree_view(90, Some(Trump::Hearts), Some(0), false, STRONG_SPADES);

        assert_eq!(
            decide(&view),
            AuctionDecision::Bid {
                value: 110,
                trump: Trump::Spades
            }
        );
    }

    #[test]
    fn does_not_overcall_a_contract_at_or_above_its_estimate() {
        let view = contree_view(110, Some(Trump::Hearts), Some(0), false, STRONG_SPADES);

        assert_eq!(decide(&view), AuctionDecision::Pass);
    }

    #[test]
    fn supports_its_partner_rather_than_overcalling_it() {
        let view = contree_view(80, Some(Trump::Hearts), Some(3), false, STRONG_SPADES);

        assert_eq!(
            decide(&view),
            AuctionDecision::Bid {
                value: 100,
                trump: Trump::Hearts
            },
            "a heart and two aces elsewhere: the partner support adds 20 in hearts instead of \
             bidding spades"
        );
    }

    #[test]
    fn estimates_its_defensive_points_from_trumps_trump_honours_side_aces_and_tens_and_the_contract()
     {
        let hand = cards("JH 9H 7H AS 10C 7D 8D 7C");

        assert_eq!(
            defensive_estimate(&hand, BID_SUIT.into(), 100),
            150 - 130 + 3 * 6 + 9 + 7 + 4 + 3
        );
    }

    #[test]
    fn coinches_an_opponents_contract_it_expects_to_defeat() {
        let view = coinche_view(120, false, "JH 9H AH 7H AS 10S AC 10C");

        assert_eq!(decide(&view), AuctionDecision::Coinche);
    }

    #[test]
    fn passes_on_an_opponents_contract_it_may_not_defeat() {
        let view = coinche_view(120, false, "7H 8H AS 7S 10C 7C 7D 8D");

        assert_eq!(decide(&view), AuctionDecision::Pass);
    }

    #[test]
    fn overcalls_rather_than_coinching_when_its_own_contract_is_higher() {
        let hand = "JH 9H JS 9S AS 10S AC 7D";
        let view = coinche_view(80, false, hand);

        assert_eq!(
            defensive_estimate(&cards(hand), BID_SUIT.into(), 80),
            85,
            "enough to coinche the 80 contract"
        );
        assert_eq!(
            decide(&view),
            AuctionDecision::Bid {
                value: 100,
                trump: Trump::Spades
            }
        );
    }

    #[test]
    fn announcement_points_add_carres_and_sequences() {
        assert_eq!(
            announcement_points(&cards("JS JH JC JD 7C 8C 9C AH"), Trump::Spades),
            220
        );
    }

    #[test]
    fn a_carre_raises_the_contree_bid() {
        let decision = decide(&contree_view(0, None, None, true, CARRE_OF_ACES));

        let AuctionDecision::Bid { value, trump } = decision else {
            panic!("a bid was expected: {decision:?}");
        };
        assert_eq!(trump, Trump::Spades);
        assert!(
            value > 90,
            "higher than the 90 bid without announcements: {decision:?}"
        );
    }

    #[test]
    fn with_announcements_off_bids_as_before() {
        assert_eq!(
            decide(&contree_view(0, None, None, false, CARRE_OF_ACES)),
            AuctionDecision::Bid {
                value: 90,
                trump: Trump::Spades
            }
        );
    }

    #[test]
    fn a_carre_makes_a_classic_bot_take() {
        let hand = "KS KH KC KD 7C";

        assert_eq!(
            decide(&classic_view(1, "9H", true, hand)),
            AuctionDecision::Trump { suit: BID_SUIT }
        );
        assert_eq!(
            decide(&classic_view(1, "9H", false, hand)),
            AuctionDecision::Pass
        );
    }

    #[test]
    fn announcements_do_not_make_the_bot_coinche() {
        let hand = "QH 7H AS AC 10D QS QC QD";

        // The defence's announcements never lower the taker's total, so they don't help defeat
        // the contract.
        assert_eq!(
            decide(&coinche_view(140, true, hand)),
            AuctionDecision::Pass
        );
        assert_eq!(
            decide(&coinche_view(140, false, hand)),
            AuctionDecision::Pass
        );
    }

    /// The bot opens at a table that plays all trumps and no trumps.
    fn all_trumps_opening(announcements: bool, hand: &str) -> AuctionView {
        AuctionView {
            all_trumps: true,
            ..contree_view(0, None, None, announcements, hand)
        }
    }

    /// An opponent at seat 0 holds the given all trumps or no trumps contract, and the bot at seat
    /// 1 may coinche it.
    fn all_trumps_coinche_view(trump: Trump, highest_bid: i32, hand: &str) -> AuctionView {
        AuctionView {
            coinche_allowed: true,
            all_trumps: true,
            ..contree_view(highest_bid, Some(trump), Some(0), false, hand)
        }
    }

    /// The ace, ten, king, nine and eight of spades, the jack and nine of hearts and the queen of
    /// diamonds.
    const MIXED_HAND: &str = "AS 10S KS 9S 8S JH 9H QD";

    /// Every ace and three tens with their aces.
    const ACES_AND_TENS: &str = "AS 10S AH 10H AD 10D AC KC";

    /// Every jack and two nines with their jacks.
    const FOUR_JACKS: &str = "JS 9S JH 9H JD JC AC 7D";

    /// Three jacks and low cards.
    const THREE_JACKS: &str = "JS JH JD 7C 8C 7D 8D 7H";

    fn bid_trump(decision: AuctionDecision) -> (i32, Trump) {
        match decision {
            AuctionDecision::Bid { value, trump } => (value, trump),
            _ => panic!("a bid was expected: {decision:?}"),
        }
    }

    #[test]
    fn estimates_a_no_trumps_hand_with_the_fitted_weights() {
        // 58, plus 15 for the ace, 11 for the ten with its ace, 5 for the king with both and 11
        // for the fifth spade.
        assert_eq!(
            contract_estimate(&cards(MIXED_HAND), Trump::NoTrumps),
            58 + 15 + 11 + 5 + 11
        );
    }

    #[test]
    fn estimates_an_all_trumps_hand_with_the_fitted_weights() {
        // 58, plus 15 for the jack, 15 for the nine of hearts with its jack, 6 for the nine of
        // spades, 2 for the ace and 1 for the ten.
        assert_eq!(
            contract_estimate(&cards(MIXED_HAND), Trump::AllTrumps),
            58 + 15 + 15 + 6 + 2 + 1
        );
    }

    #[test]
    fn a_suit_contract_keeps_its_estimate() {
        assert_eq!(
            contract_estimate(&cards(STRONG_SPADES), Trump::Spades),
            suit_estimate(&cards(STRONG_SPADES), Suit::Spades)
        );
    }

    #[test]
    fn bids_no_trumps_with_aces_and_tens() {
        let (_, trump) = bid_trump(decide(&all_trumps_opening(false, ACES_AND_TENS)));
        assert_eq!(trump, Trump::NoTrumps);
    }

    #[test]
    fn bids_all_trumps_with_four_jacks() {
        let (_, trump) = bid_trump(decide(&all_trumps_opening(false, FOUR_JACKS)));
        assert_eq!(trump, Trump::AllTrumps);
    }

    #[test]
    fn never_bids_the_new_contracts_when_the_table_does_not_play_them() {
        for hand in [ACES_AND_TENS, FOUR_JACKS] {
            let decision = decide(&contree_view(0, None, None, false, hand));
            assert!(
                match decision {
                    AuctionDecision::Pass => true,
                    AuctionDecision::Bid { trump, .. } => trump.suit_contract(),
                    _ => false,
                },
                "{decision:?}"
            );
        }
    }

    #[test]
    fn counts_a_carre_of_aces_at_its_no_trumps_value() {
        assert_eq!(
            announcement_points(&cards(CARRE_OF_ACES), Trump::NoTrumps),
            200
        );

        let without = bid_trump(decide(&all_trumps_opening(false, CARRE_OF_ACES)));
        let with = bid_trump(decide(&all_trumps_opening(true, CARRE_OF_ACES)));

        assert_eq!(without.1, Trump::NoTrumps);
        assert_eq!(with.1, Trump::NoTrumps);
        assert!(with.0 > without.0, "{without:?} then {with:?}");
    }

    #[test]
    fn leaves_its_partners_all_trumps_alone() {
        for trump in [Trump::AllTrumps, Trump::NoTrumps] {
            let view = AuctionView {
                all_trumps: true,
                ..contree_view(80, Some(trump), Some(3), false, FOUR_JACKS)
            };
            assert_eq!(decide(&view), AuctionDecision::Pass, "{trump:?}");
        }
    }

    #[test]
    fn estimates_the_defence_against_all_trumps_with_the_fitted_weights() {
        // 203, plus 13 for each jack, less 1.84 for each point of the contract.
        assert_eq!(
            defensive_estimate(&cards(THREE_JACKS), Trump::AllTrumps, 100),
            203 + 3 * 13 - 184
        );
    }

    #[test]
    fn estimates_the_defence_against_no_trumps_with_the_fitted_weights() {
        // 235, plus 24 for each ace and 2 for the jack, less 2 for the ten and 2.29 for each point
        // of the contract.
        assert_eq!(
            defensive_estimate(&cards("AS AH 10H JC 7C 8D 7D 9S"), Trump::NoTrumps, 100),
            235 + 2 * 24 - 2 + 2 - 229
        );
    }

    #[test]
    fn coinches_an_all_trumps_contract_with_three_jacks() {
        // 203 + 39 − 1.84 × 90 = 76, enough to keep the taker below 90 (162 − 90 − 4 = 68); its
        // own all trumps estimate, 58 + 45 less the margin, is too low to overcall.
        assert_eq!(
            decide(&all_trumps_coinche_view(Trump::AllTrumps, 90, THREE_JACKS)),
            AuctionDecision::Coinche
        );
    }

    #[test]
    fn does_not_coinche_an_all_trumps_contract_without_honours() {
        let hand = "7S 8S 7H 8H 7C 8C 7D 8D";
        for trump in [Trump::AllTrumps, Trump::NoTrumps] {
            assert_eq!(
                decide(&all_trumps_coinche_view(trump, 160, hand)),
                AuctionDecision::Pass,
                "{trump:?}"
            );
        }
    }

    /// Rounding goes half up, also for negative values.
    #[test]
    fn round_half_up_rounds_half_up() {
        assert_eq!(round_half_up(2.5), 3);
        assert_eq!(round_half_up(-2.5), -2);
        assert_eq!(round_half_up(-2.6), -3);
        assert_eq!(round_half_up(0.499_999_999_999_999_94), 0);
    }

    /// The first of equal maximums wins, as the first of equal minimums does.
    #[test]
    fn first_max_keeps_the_first_of_equal_cards() {
        let hand = cards("8C 8D 7S");
        assert_eq!(
            first_max(hand.iter().copied(), by_value(Trump::Hearts)),
            Some(card("8C"))
        );
    }
}
