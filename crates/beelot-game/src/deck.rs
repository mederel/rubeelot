//! The 32 cards between two deals, as a face-down pile listed from its top card, and how they were
//! put together (US-085): shuffled for the first deal of a match, then gathered from the previous
//! deal without a shuffle. Port of the Java `Deck`.

use serde::{Deserialize, Serialize};

use crate::card::Card;
use crate::rng::{RandomSource, shuffle};

/// How a deck was put together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Gathering {
    /// Shuffled, for the first deal of a match.
    Shuffled,
    /// Each team's tricks, counted, one pile on the other.
    Tricks,
    /// The hands, after everyone passed.
    Hands,
}

/// The cards, top card first. The pile sizes and which pile went on top are set for cards gathered
/// from the tricks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "DeckFields")]
pub struct Deck {
    cards: Vec<Card>,
    gathering: Gathering,
    north_south_pile: usize,
    east_west_pile: usize,
    north_south_on_top: bool,
}

/// A deck as it is read, before its cards are checked.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeckFields {
    cards: Vec<Card>,
    gathering: Gathering,
    north_south_pile: usize,
    east_west_pile: usize,
    north_south_on_top: bool,
}

impl TryFrom<DeckFields> for Deck {
    type Error = &'static str;

    fn try_from(fields: DeckFields) -> Result<Self, Self::Error> {
        if !holds_every_card_once(&fields.cards) {
            return Err(NOT_A_DECK);
        }
        Ok(Deck {
            cards: fields.cards,
            gathering: fields.gathering,
            north_south_pile: fields.north_south_pile,
            east_west_pile: fields.east_west_pile,
            north_south_on_top: fields.north_south_on_top,
        })
    }
}

/// The deck once cut, top card first, and how many cards were lifted from the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cut {
    pub lifted: usize,
    pub cards: Vec<Card>,
}

pub const SIZE: usize = 32;
/// Each part of a cut keeps at least this many cards.
pub const MIN_CUT: usize = 3;

const NOT_A_DECK: &str = "A deck holds the 32 cards once each.";

fn holds_every_card_once(cards: &[Card]) -> bool {
    let mut sorted = cards.to_vec();
    sorted.sort_by_key(|card| (card.suit.ordinal(), card.rank as usize));
    sorted.dedup();
    cards.len() == SIZE && sorted.len() == SIZE
}

impl Deck {
    /// # Panics
    ///
    /// When the cards are not the 32 cards once each.
    fn new(
        cards: Vec<Card>,
        gathering: Gathering,
        north_south_pile: usize,
        east_west_pile: usize,
        north_south_on_top: bool,
    ) -> Self {
        assert!(holds_every_card_once(&cards), "{NOT_A_DECK}");
        Deck {
            cards,
            gathering,
            north_south_pile,
            east_west_pile,
            north_south_on_top,
        }
    }

    /// A shuffled deck, for the first deal of a match: the same seeded generator gives the same
    /// deck.
    pub fn shuffled(rng: &mut dyn RandomSource) -> Self {
        let mut cards = Card::deck();
        shuffle(&mut cards, rng);
        Deck::new(cards, Gathering::Shuffled, 0, 0, false)
    }

    /// The hands put together after everyone passed, in the given order.
    ///
    /// # Panics
    ///
    /// When the cards are not the 32 cards once each.
    pub fn gathered_hands(cards: Vec<Card>) -> Self {
        Deck::new(cards, Gathering::Hands, 0, 0, false)
    }

    /// Each team counts its pile of tricks, which reverses it, then one counted pile goes on the
    /// other.
    ///
    /// # Panics
    ///
    /// When the piles do not hold the 32 cards once each.
    pub fn gathered_piles(
        north_south_pile: &[Card],
        east_west_pile: &[Card],
        north_south_on_top: bool,
    ) -> Self {
        let (top, bottom) = if north_south_on_top {
            (north_south_pile, east_west_pile)
        } else {
            (east_west_pile, north_south_pile)
        };
        let mut cards = count(top);
        cards.extend(count(bottom));
        Deck::new(
            cards,
            Gathering::Tricks,
            north_south_pile.len(),
            east_west_pile.len(),
            north_south_on_top,
        )
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn gathering(&self) -> Gathering {
        self.gathering
    }

    pub fn north_south_pile(&self) -> usize {
        self.north_south_pile
    }

    pub fn east_west_pile(&self) -> usize {
        self.east_west_pile
    }

    pub fn north_south_on_top(&self) -> bool {
        self.north_south_on_top
    }

    /// Cuts at a random point, keeping at least three cards in each part.
    pub fn cut(&self, rng: &mut dyn RandomSource) -> Cut {
        let lifted = MIN_CUT + rng.next_int((SIZE - 2 * MIN_CUT + 1) as i32) as usize;
        self.cut_at(lifted)
    }

    /// Lifts the given number of cards from the top and puts the bottom part on top of them.
    ///
    /// # Panics
    ///
    /// When either part would keep fewer than three cards.
    pub fn cut_at(&self, lifted: usize) -> Cut {
        assert!(
            (MIN_CUT..=SIZE - MIN_CUT).contains(&lifted),
            "Each part of a cut keeps at least {MIN_CUT} cards."
        );
        let mut cards = self.cards[lifted..].to_vec();
        cards.extend_from_slice(&self.cards[..lifted]);
        Cut { lifted, cards }
    }
}

/// A trick won goes face down on top of the team's pile. The trick lay face up with its first card
/// at the bottom, so once turned over its first card is on top.
pub fn pile_trick(pile: &[Card], trick: &[Card]) -> Vec<Card> {
    let mut piled = trick.to_vec();
    piled.extend_from_slice(pile);
    piled
}

/// Counting turns the cards over one by one onto a new pile, so the pile ends up in reverse order.
pub fn count(pile: &[Card]) -> Vec<Card> {
    pile.iter().rev().copied().collect()
}
