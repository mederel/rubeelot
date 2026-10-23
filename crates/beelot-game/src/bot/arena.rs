//! The bots the arena seats against each other: the current bot, the Challenging bot and the
//! reference bots that keep an older behaviour. A bot only sees its player's view, and any
//! randomness comes from the given generator so that a seeded run is repeatable. Port of the Java
//! `ArenaBot` and its implementations.

use std::fmt;

use super::BotStrategy;
use super::rule_based::{RuleBasedStrategy, by_value, first_min, preferring, support_raise};
use super::sampling::SamplingStrategy;
use crate::bidding::{AuctionDecision, AuctionView};
use crate::board::{PlayView, SeatCard};
use crate::card::{Card, Rank, Suit, Variant};
use crate::rng::RandomSource;
use crate::rules;

/// The names of the arena bots, in the order the arena lists them.
pub const ARENA_BOTS: [&str; 8] = [
    "current",
    "challenging",
    "baseline",
    "previous-play",
    "previous-trumps",
    "no-coinche",
    "no-announcement-bidding",
    "random",
];

/// A bot strategy the arena can seat.
pub trait ArenaBot: Send + Sync {
    fn name(&self) -> &'static str;

    fn decide_auction(&self, view: &AuctionView, rng: &mut dyn RandomSource) -> AuctionDecision;

    /// Returns one of the view's legal cards.
    fn choose_card(&self, view: &PlayView, rng: &mut dyn RandomSource) -> Card;
}

/// No arena bot has the requested name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownBot {
    name: String,
}

impl UnknownBot {
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for UnknownBot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Unknown bot {}. Available: [{}]",
            self.name,
            ARENA_BOTS.join(", ")
        )
    }
}

impl std::error::Error for UnknownBot {}

/// The arena bot of that name. The Challenging bot searches within its 200 ms budget.
pub fn arena_bot(name: &str) -> Result<Box<dyn ArenaBot>, UnknownBot> {
    named(name, SamplingStrategy::new(CHALLENGING_SEED))
}

/// The same, except that the Challenging bot has no time budget, so its decisions do not depend
/// on the speed of the machine. The fixture replay uses it.
pub fn arena_bot_unbounded(name: &str) -> Result<Box<dyn ArenaBot>, UnknownBot> {
    named(
        name,
        SamplingStrategy::with_settings(CHALLENGING_SEED, 50, None, 3),
    )
}

const CHALLENGING_SEED: i64 = 1;

fn named(name: &str, challenging: SamplingStrategy) -> Result<Box<dyn ArenaBot>, UnknownBot> {
    Ok(match name {
        "current" => Box::new(CurrentBot::default()),
        "challenging" => Box::new(ChallengingBot(challenging)),
        "baseline" => Box::new(BaselineBot::default()),
        "previous-play" => Box::new(PreviousPlayBot::default()),
        "previous-trumps" => {
            Box::new(PreviousTrumpsBot(RuleBasedStrategy::without_trump_control()))
        }
        "no-coinche" => Box::new(NoCoincheBot::default()),
        "no-announcement-bidding" => Box::new(NoAnnouncementBiddingBot::default()),
        "random" => Box::new(RandomCardBot::default()),
        _ => {
            return Err(UnknownBot {
                name: name.to_string(),
            });
        }
    })
}

/// The bot players use in the application.
#[derive(Default)]
struct CurrentBot(RuleBasedStrategy);

impl ArenaBot for CurrentBot {
    fn name(&self) -> &'static str {
        "current"
    }

    fn decide_auction(&self, view: &AuctionView, _: &mut dyn RandomSource) -> AuctionDecision {
        self.0.decide_auction(view)
    }

    fn choose_card(&self, view: &PlayView, _: &mut dyn RandomSource) -> Card {
        self.0.choose_card(view)
    }
}

/// The Challenging bot: it samples and solves deals before playing a card.
struct ChallengingBot(SamplingStrategy);

impl ArenaBot for ChallengingBot {
    fn name(&self) -> &'static str {
        "challenging"
    }

    fn decide_auction(&self, view: &AuctionView, _: &mut dyn RandomSource) -> AuctionDecision {
        self.0.decide_auction(view)
    }

    fn choose_card(&self, view: &PlayView, _: &mut dyn RandomSource) -> Card {
        self.0.choose_card(view)
    }
}

const MAX_BID: i32 = 160;

/// Reference bot: plays its cards like the current bot, but bids as bots did before US-050 and
/// US-051. It never takes a contract in classic Belote; in Contrée, it opens 80 in its longest
/// suit and only ever supports its partner once.
#[derive(Default)]
struct BaselineBot(RuleBasedStrategy);

impl ArenaBot for BaselineBot {
    fn name(&self) -> &'static str {
        "baseline"
    }

    fn decide_auction(&self, view: &AuctionView, _: &mut dyn RandomSource) -> AuctionDecision {
        if view.variant != Variant::Contree {
            return AuctionDecision::Pass;
        }
        if view.highest_bid == 0 {
            return AuctionDecision::Bid {
                value: 80,
                suit: longest_suit(&view.hand),
            };
        }
        let raise = match view.highest_bid_suit {
            Some(suit) if view.partner_holds_contract() && !view.has_bid() => {
                support_raise(&view.hand, suit)
            }
            _ => 0,
        };
        if let Some(suit) = view.highest_bid_suit
            && raise > 0
            && view.highest_bid < MAX_BID
        {
            return AuctionDecision::Bid {
                value: (view.highest_bid + raise).min(MAX_BID),
                suit,
            };
        }
        AuctionDecision::Pass
    }

    fn choose_card(&self, view: &PlayView, _: &mut dyn RandomSource) -> Card {
        self.0.choose_card(view)
    }
}

/// The suit with the most cards in the hand; the first of equals in suit order wins.
fn longest_suit(hand: &[Card]) -> Suit {
    let mut best = Suit::Clubs;
    let mut best_count = None;
    for suit in Suit::ALL {
        let count = hand.iter().filter(|card| card.suit == suit).count();
        if best_count.is_none_or(|best_count| count > best_count) {
            best = suit;
            best_count = Some(count);
        }
    }
    best
}

/// Reference bot: bids like the current bot, but plays its cards as bots did before US-053 (the
/// rules of US-033 to US-037). When no rule applies, it plays its first legal card.
#[derive(Default)]
struct PreviousPlayBot(RuleBasedStrategy);

impl ArenaBot for PreviousPlayBot {
    fn name(&self) -> &'static str {
        "previous-play"
    }

    fn decide_auction(&self, view: &AuctionView, _: &mut dyn RandomSource) -> AuctionDecision {
        self.0.decide_auction(view)
    }

    /// Cashes an ace likely to be trumped the next time its suit is played. Keeps trumps out of
    /// the first trick when defending, and aces out of a trick the opponents have already won
    /// with a trump. When the opponents win the trick, the bot cannot beat them and its partner
    /// has already played, the bot plays its lowest-value card. Otherwise it plays its first
    /// legal card.
    fn choose_card(&self, view: &PlayView, _: &mut dyn RandomSource) -> Card {
        let trump = view.trump;
        let mut candidates = view.legal_cards.clone();
        if view.tricks.is_empty() && !view.declaring() {
            candidates = preferring(&candidates, |card| card.suit != trump);
        }
        let trick = &view.current_trick;
        if trick.is_empty() {
            return candidates
                .iter()
                .copied()
                .find(|card| ace_at_risk(*card, view))
                .unwrap_or(candidates[0]);
        }
        let lead = trick[0].card.suit;
        let winner = trick[rules::winning_index(
            &trick.iter().map(|played| played.card).collect::<Vec<_>>(),
            trump,
        )];
        if winner.card.suit != trump {
            let ace = Card::new(Rank::Ace, lead);
            if candidates.contains(&ace) && ace_at_risk(ace, view) {
                return ace;
            }
        }
        let opponents_win = winner.seat % 2 != view.seat % 2;
        let opponents_trumped = winner.card.suit == trump && opponents_win;
        let can_win = candidates
            .iter()
            .any(|card| rules::beats(*card, winner.card, lead, trump));
        if opponents_trumped && !can_win {
            candidates = preferring(&candidates, |card| card.rank != Rank::Ace);
        }
        let partner_played = trick.len() >= 2;
        if opponents_win && partner_played && !can_win {
            return first_min(candidates, by_value(trump))
                .expect("a seat to play has a legal card");
        }
        candidates[0]
    }
}

fn ace_at_risk(card: Card, view: &PlayView) -> bool {
    let trump = view.trump;
    if card.rank != Rank::Ace || card.suit == trump || unseen_cards(trump, view) == 0 {
        return false;
    }
    let voids = shown_voids(view);
    let mut opponent_shown_void = false;
    for (seat, seat_voids) in voids.iter().enumerate() {
        if seat % 2 != view.seat % 2
            && seat_voids.contains(&card.suit)
            && !seat_voids.contains(&trump)
        {
            opponent_shown_void = true;
        }
    }
    opponent_shown_void || unseen_cards(card.suit, view) <= 2
}

/// For each seat, the suits it did not follow in the tricks played so far.
fn shown_voids(view: &PlayView) -> [Vec<Suit>; 4] {
    let mut voids: [Vec<Suit>; 4] = Default::default();
    for trick in played_tricks(view) {
        let lead = trick[0].card.suit;
        for played in trick {
            if played.card.suit != lead && !voids[played.seat].contains(&lead) {
                voids[played.seat].push(lead);
            }
        }
    }
    voids
}

fn unseen_cards(suit: Suit, view: &PlayView) -> i64 {
    let played = played_tricks(view)
        .iter()
        .flat_map(|trick| trick.iter())
        .filter(|played| played.card.suit == suit)
        .count() as i64;
    let held = view.hand.iter().filter(|card| card.suit == suit).count() as i64;
    8 - held - played
}

fn played_tricks(view: &PlayView) -> Vec<&[SeatCard]> {
    let mut tricks: Vec<&[SeatCard]> = view.tricks.iter().map(Vec::as_slice).collect();
    if !view.current_trick.is_empty() {
        tricks.push(&view.current_trick);
    }
    tricks
}

/// Reference bot: bids like the current bot, but plays its cards as bots did before US-054,
/// without managing trumps.
struct PreviousTrumpsBot(RuleBasedStrategy);

impl ArenaBot for PreviousTrumpsBot {
    fn name(&self) -> &'static str {
        "previous-trumps"
    }

    fn decide_auction(&self, view: &AuctionView, _: &mut dyn RandomSource) -> AuctionDecision {
        self.0.decide_auction(view)
    }

    fn choose_card(&self, view: &PlayView, _: &mut dyn RandomSource) -> Card {
        self.0.choose_card(view)
    }
}

/// Reference bot: bids and plays like the current bot, but passes where it would coinche, as
/// bots did before US-055.
#[derive(Default)]
struct NoCoincheBot(RuleBasedStrategy);

impl ArenaBot for NoCoincheBot {
    fn name(&self) -> &'static str {
        "no-coinche"
    }

    fn decide_auction(&self, view: &AuctionView, _: &mut dyn RandomSource) -> AuctionDecision {
        match self.0.decide_auction(view) {
            AuctionDecision::Coinche => AuctionDecision::Pass,
            decision => decision,
        }
    }

    fn choose_card(&self, view: &PlayView, _: &mut dyn RandomSource) -> Card {
        self.0.choose_card(view)
    }
}

/// Reference bot: plays like the current bot, but bids as though the round played without
/// announcements (US-061).
#[derive(Default)]
struct NoAnnouncementBiddingBot(RuleBasedStrategy);

impl ArenaBot for NoAnnouncementBiddingBot {
    fn name(&self) -> &'static str {
        "no-announcement-bidding"
    }

    fn decide_auction(&self, view: &AuctionView, _: &mut dyn RandomSource) -> AuctionDecision {
        let mut view = view.clone();
        view.announcements = false;
        self.0.decide_auction(&view)
    }

    fn choose_card(&self, view: &PlayView, _: &mut dyn RandomSource) -> Card {
        self.0.choose_card(view)
    }
}

/// Baseline bot: bids like the current bot, then plays a random legal card.
#[derive(Default)]
struct RandomCardBot(RuleBasedStrategy);

impl ArenaBot for RandomCardBot {
    fn name(&self) -> &'static str {
        "random"
    }

    fn decide_auction(&self, view: &AuctionView, _: &mut dyn RandomSource) -> AuctionDecision {
        self.0.decide_auction(view)
    }

    fn choose_card(&self, view: &PlayView, rng: &mut dyn RandomSource) -> Card {
        view.legal_cards[rng.next_int(view.legal_cards.len() as i32) as usize]
    }
}

#[cfg(test)]
mod tests {
    //! Port of the Java `ArenaBot.named` contract; the bots' decisions are checked against Java in
    //! `tests/deals.rs`.

    use super::*;

    #[test]
    fn every_listed_bot_can_be_named_and_reports_its_name() {
        for name in ARENA_BOTS {
            assert_eq!(arena_bot(name).unwrap().name(), name);
            assert_eq!(arena_bot_unbounded(name).unwrap().name(), name);
        }
    }

    #[test]
    fn an_unknown_bot_is_refused_with_the_available_names() {
        let error = arena_bot("nobody").err().expect("an unknown bot");

        assert_eq!(
            error.to_string(),
            "Unknown bot nobody. Available: [current, challenging, baseline, previous-play, \
             previous-trumps, no-coinche, no-announcement-bidding, random]"
        );
    }

    #[test]
    fn the_longest_suit_is_the_first_of_equals() {
        let hand: Vec<Card> = [
            (Rank::Seven, Suit::Hearts),
            (Rank::Eight, Suit::Hearts),
            (Rank::Seven, Suit::Spades),
            (Rank::Eight, Suit::Spades),
            (Rank::Nine, Suit::Clubs),
        ]
        .map(|(rank, suit)| Card::new(rank, suit))
        .into();

        assert_eq!(longest_suit(&hand), Suit::Hearts);
        assert_eq!(longest_suit(&[]), Suit::Clubs);
    }
}
