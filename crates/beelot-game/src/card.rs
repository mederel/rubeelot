use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A seat at the table, 0 to 3, clockwise.
pub type Seat = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    pub fn ordinal(self) -> usize {
        self as usize
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Suit::Clubs => "Clubs",
            Suit::Diamonds => "Diamonds",
            Suit::Hearts => "Hearts",
            Suit::Spades => "Spades",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Suit::Clubs => "♣",
            Suit::Diamonds => "♦",
            Suit::Hearts => "♥",
            Suit::Spades => "♠",
        }
    }

    fn initial(self) -> char {
        match self {
            Suit::Clubs => 'C',
            Suit::Diamonds => 'D',
            Suit::Hearts => 'H',
            Suit::Spades => 'S',
        }
    }
}

/// What a round is played in: one of the four suits as trump, or, in Contrée, all trumps or no
/// trumps (US-063).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Trump {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
    AllTrumps,
    NoTrumps,
}

impl From<Suit> for Trump {
    fn from(suit: Suit) -> Self {
        match suit {
            Suit::Clubs => Trump::Clubs,
            Suit::Diamonds => Trump::Diamonds,
            Suit::Hearts => Trump::Hearts,
            Suit::Spades => Trump::Spades,
        }
    }
}

impl Trump {
    pub const ALL: [Trump; 6] = [
        Trump::Clubs,
        Trump::Diamonds,
        Trump::Hearts,
        Trump::Spades,
        Trump::AllTrumps,
        Trump::NoTrumps,
    ];

    /// The trump suit, or none under all trumps and no trumps.
    pub fn suit(self) -> Option<Suit> {
        match self {
            Trump::Clubs => Some(Suit::Clubs),
            Trump::Diamonds => Some(Suit::Diamonds),
            Trump::Hearts => Some(Suit::Hearts),
            Trump::Spades => Some(Suit::Spades),
            Trump::AllTrumps | Trump::NoTrumps => None,
        }
    }

    /// Whether cards of the given suit are trumps.
    pub fn is_trump(self, suit: Suit) -> bool {
        self == Trump::AllTrumps || self.suit() == Some(suit)
    }

    /// Whether one suit is trump, as in classic Belote.
    pub fn suit_contract(self) -> bool {
        self.suit().is_some()
    }

    pub fn ordinal(self) -> usize {
        self as usize
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Trump::AllTrumps => "All trumps",
            Trump::NoTrumps => "No trumps",
            suit => suit.suit().expect("a suit contract").display_name(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Rank {
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    pub const ALL: [Rank; 8] = [
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
            Rank::Ace => "A",
        }
    }

    /// Java's `String.hashCode` of the label.
    pub fn java_hash(self) -> i32 {
        match self {
            Rank::Seven => 55,
            Rank::Eight => 56,
            Rank::Nine => 57,
            Rank::Ten => 1567,
            Rank::Jack => 74,
            Rank::Queen => 81,
            Rank::King => 75,
            Rank::Ace => 65,
        }
    }

    fn from_label(label: &str) -> Option<Rank> {
        Rank::ALL.into_iter().find(|rank| rank.label() == label)
    }

    /// Position in the hand display order, trump suit or not.
    fn display_position(self, trump_suit: bool) -> usize {
        let order: [Rank; 8] = if trump_suit {
            [
                Rank::Seven,
                Rank::Eight,
                Rank::Queen,
                Rank::King,
                Rank::Ten,
                Rank::Ace,
                Rank::Nine,
                Rank::Jack,
            ]
        } else {
            [
                Rank::Seven,
                Rank::Eight,
                Rank::Nine,
                Rank::Jack,
                Rank::Queen,
                Rank::King,
                Rank::Ten,
                Rank::Ace,
            ]
        };
        order
            .iter()
            .position(|&rank| rank == self)
            .unwrap_or(usize::MAX)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub fn new(rank: Rank, suit: Suit) -> Self {
        Self { rank, suit }
    }

    /// The 32 cards: suits in order, then ranks from 7 to ace.
    pub fn deck() -> Vec<Card> {
        Suit::ALL
            .into_iter()
            .flat_map(|suit| Rank::ALL.into_iter().map(move |rank| Card::new(rank, suit)))
            .collect()
    }

    /// Sort key of `GameCard.displayOrder(trump)`: the trump suit first when there is one, then
    /// the other suits in order, each in its display order. No trump is the auction's natural
    /// order.
    pub fn display_key(self, trump: Option<Trump>) -> (usize, usize) {
        let suit_order = match trump.and_then(Trump::suit) {
            None => self.suit.ordinal(),
            Some(suit) if suit == self.suit => 0,
            Some(suit) => self.suit.ordinal() + usize::from(self.suit.ordinal() < suit.ordinal()),
        };
        let trump_card = trump.is_some_and(|trump| trump.is_trump(self.suit));
        (suit_order, self.rank.display_position(trump_card))
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank.label(), self.suit.initial())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseCardError(pub String);

impl fmt::Display for ParseCardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid card: {:?}", self.0)
    }
}

impl std::error::Error for ParseCardError {}

impl FromStr for Card {
    type Err = ParseCardError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseCardError(s.to_string());
        let initial = s.chars().next_back().ok_or_else(err)?;
        let label = &s[..s.len() - initial.len_utf8()];
        let suit = Suit::ALL
            .into_iter()
            .find(|suit| suit.initial() == initial)
            .ok_or_else(err)?;
        let rank = Rank::from_label(label).ok_or_else(err)?;
        Ok(Card::new(rank, suit))
    }
}

impl Serialize for Card {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Card {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Variant {
    Classic,
    Contree,
}

impl Variant {
    pub fn display_name(self) -> &'static str {
        match self {
            Variant::Classic => "Classic Belote",
            Variant::Contree => "Contrée",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Difficulty {
    Relaxed,
    Challenging,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    #[serde(rename = "North–South")]
    NorthSouth,
    #[serde(rename = "East–West")]
    EastWest,
}

impl Team {
    pub fn of(seat: Seat) -> Team {
        if seat.is_multiple_of(2) {
            Team::NorthSouth
        } else {
            Team::EastWest
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Team::NorthSouth => "North–South",
            Team::EastWest => "East–West",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut cards: Vec<Card>, trump: Option<Trump>) -> Vec<Card> {
        cards.sort_by_key(|card| card.display_key(trump));
        cards
    }

    #[test]
    fn orders_normal_suit_from_seven_through_ace() {
        use Rank::*;
        let cards = [Ace, Ten, King, Queen, Jack, Nine, Eight, Seven]
            .map(|rank| Card::new(rank, Suit::Clubs))
            .to_vec();

        let ranks: Vec<Rank> = sorted(cards, None).iter().map(|c| c.rank).collect();

        assert_eq!(ranks, [Seven, Eight, Nine, Jack, Queen, King, Ten, Ace]);
    }

    #[test]
    fn puts_trump_first_and_uses_trump_display_order() {
        use Rank::*;
        let mut cards = vec![Card::new(Ace, Suit::Clubs)];
        cards.extend(
            [Jack, Seven, Nine, Ace, Ten, King, Queen, Eight]
                .map(|rank| Card::new(rank, Suit::Hearts)),
        );

        let sorted = sorted(cards, Some(Trump::Hearts));

        let ranks: Vec<Rank> = sorted.iter().map(|c| c.rank).collect();
        assert_eq!(
            ranks,
            [Seven, Eight, Queen, King, Ten, Ace, Nine, Jack, Ace]
        );
        assert_eq!(sorted.last().unwrap().suit, Suit::Clubs);
    }
}
