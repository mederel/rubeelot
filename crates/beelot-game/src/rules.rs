//! Card ranking and points under a trump suit, shared by the rules engine and the bots.

use crate::card::{Card, Rank, Suit};

/// The card's rank within its suit: a higher value beats a lower one of the same suit.
pub fn strength(card: Card, trump: Suit) -> i32 {
    const TRUMP_ORDER: [Rank; 8] = [
        Rank::Seven,
        Rank::Eight,
        Rank::Queen,
        Rank::King,
        Rank::Ten,
        Rank::Ace,
        Rank::Nine,
        Rank::Jack,
    ];
    const NORMAL_ORDER: [Rank; 8] = [
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ten,
        Rank::Ace,
    ];
    let order = if card.suit == trump {
        TRUMP_ORDER
    } else {
        NORMAL_ORDER
    };
    order
        .iter()
        .position(|&rank| rank == card.rank)
        .map_or(-1, |index| index as i32)
}

pub fn points(card: Card, trump: Suit) -> i32 {
    let is_trump = card.suit == trump;
    match card.rank {
        Rank::Ace => 11,
        Rank::Ten => 10,
        Rank::King => 4,
        Rank::Queen => 3,
        Rank::Jack if is_trump => 20,
        Rank::Jack => 2,
        Rank::Nine if is_trump => 14,
        _ => 0,
    }
}

/// Whether the contender beats the card currently winning a trick led in the given suit.
pub fn beats(contender: Card, winner: Card, lead: Suit, trump: Suit) -> bool {
    if contender.suit == winner.suit {
        return strength(contender, trump) > strength(winner, trump);
    }
    contender.suit == trump || (winner.suit != trump && contender.suit == lead)
}

/// The cards of the hand that may be played on the given trick, in hand order. A player must
/// follow the suit led, overtrumping when trumps are led. A player who cannot follow must trump,
/// overtrumping when possible, unless their partner is winning the trick.
pub fn legal_cards(hand: &[Card], trick: &[Card], trump: Suit) -> Vec<Card> {
    let Some(first) = trick.first() else {
        return hand.to_vec();
    };
    let lead = first.suit;
    let winner = winning_index(trick, trump);
    let of_suit = |suit: Suit| -> Vec<Card> {
        hand.iter()
            .copied()
            .filter(|card| card.suit == suit)
            .collect()
    };
    let lead_cards = of_suit(lead);
    if !lead_cards.is_empty() {
        if lead != trump {
            return lead_cards;
        }
        let higher = higher_trumps(&lead_cards, trick[winner], trump);
        return if higher.is_empty() {
            lead_cards
        } else {
            higher
        };
    }
    let partner_winning = winner + 2 == trick.len();
    if partner_winning {
        return hand.to_vec();
    }
    let trumps = of_suit(trump);
    if trumps.is_empty() {
        return hand.to_vec();
    }
    let higher = higher_trumps(&trumps, trick[winner], trump);
    if higher.is_empty() { trumps } else { higher }
}

fn higher_trumps(trumps: &[Card], winner: Card, trump: Suit) -> Vec<Card> {
    if winner.suit != trump {
        return trumps.to_vec();
    }
    trumps
        .iter()
        .copied()
        .filter(|&card| strength(card, trump) > strength(winner, trump))
        .collect()
}

/// The position, in play order, of the card winning the given non-empty trick.
pub fn winning_index(trick: &[Card], trump: Suit) -> usize {
    let lead = trick[0].suit;
    let mut winner = 0;
    for index in 1..trick.len() {
        if beats(trick[index], trick[winner], lead, trump) {
            winner = index;
        }
    }
    winner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank::*, Suit::*};

    #[test]
    fn rules_legal_cards_follow_overtrump() {
        let hand = [
            Card::new(Seven, Hearts),
            Card::new(Ace, Hearts),
            Card::new(Eight, Clubs),
            Card::new(Nine, Hearts),
        ];
        let trick = [Card::new(King, Hearts)];

        assert_eq!(
            legal_cards(&hand, &trick, Hearts),
            vec![Card::new(Ace, Hearts), Card::new(Nine, Hearts)]
        );
        // Following a plain suit: only that suit, in hand order.
        let trick = [Card::new(Ace, Clubs)];
        assert_eq!(
            legal_cards(&hand, &trick, Hearts),
            vec![Card::new(Eight, Clubs)]
        );
        // Empty trick: everything.
        assert_eq!(legal_cards(&hand, &[], Hearts), hand.to_vec());
    }
}
