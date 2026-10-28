//! Card ranking and points under a trump suit, all trumps or no trumps, shared by the rules engine
//! and the bots.

use crate::card::{Card, Rank, Suit, Trump};

/// The card's rank within its suit: a higher value beats a lower one of the same suit.
pub fn strength(card: Card, trump: Trump) -> i32 {
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
    let order = if trump.is_trump(card.suit) {
        TRUMP_ORDER
    } else {
        NORMAL_ORDER
    };
    order
        .iter()
        .position(|&rank| rank == card.rank)
        .map_or(-1, |index| index as i32)
}

pub fn points(card: Card, trump: Trump) -> i32 {
    match trump {
        Trump::AllTrumps => {
            return match card.rank {
                Rank::Jack => 13,
                Rank::Nine => 9,
                Rank::Ace => 6,
                Rank::Ten => 5,
                Rank::King => 3,
                Rank::Queen => 2,
                _ => 0,
            };
        }
        Trump::NoTrumps => {
            return match card.rank {
                Rank::Ace => 19,
                Rank::Ten => 10,
                Rank::King => 4,
                Rank::Queen => 3,
                Rank::Jack => 2,
                _ => 0,
            };
        }
        _ => {}
    }
    let is_trump = trump.is_trump(card.suit);
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
pub fn beats(contender: Card, winner: Card, lead: Suit, trump: Trump) -> bool {
    if contender.suit == winner.suit {
        return strength(contender, trump) > strength(winner, trump);
    }
    if !trump.suit_contract() {
        return false;
    }
    trump.is_trump(contender.suit) || (!trump.is_trump(winner.suit) && contender.suit == lead)
}

/// The cards of the hand that may be played on the given trick, in hand order. A player must
/// follow the suit led, overtrumping when trumps are led. A player who cannot follow must trump,
/// overtrumping when possible, unless their partner is winning the trick. Under all trumps a player
/// must follow and overtake the winning card; under no trumps a player must follow, and otherwise
/// plays any card.
pub fn legal_cards(hand: &[Card], trick: &[Card], trump: Trump) -> Vec<Card> {
    let Some(first) = trick.first() else {
        return hand.to_vec();
    };
    let lead = first.suit;
    let winner = winning_index(trick, trump);
    let lead_cards: Vec<Card> = hand
        .iter()
        .copied()
        .filter(|card| card.suit == lead)
        .collect();
    if !lead_cards.is_empty() {
        if !trump.is_trump(lead) {
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
    if partner_winning || !trump.suit_contract() {
        return hand.to_vec();
    }
    let trumps: Vec<Card> = hand
        .iter()
        .copied()
        .filter(|card| trump.is_trump(card.suit))
        .collect();
    if trumps.is_empty() {
        return hand.to_vec();
    }
    let higher = higher_trumps(&trumps, trick[winner], trump);
    if higher.is_empty() { trumps } else { higher }
}

fn higher_trumps(trumps: &[Card], winner: Card, trump: Trump) -> Vec<Card> {
    if !trump.is_trump(winner.suit) {
        return trumps.to_vec();
    }
    trumps
        .iter()
        .copied()
        .filter(|&card| strength(card, trump) > strength(winner, trump))
        .collect()
}

/// The position, in play order, of the card winning the given non-empty trick.
pub fn winning_index(trick: &[Card], trump: Trump) -> usize {
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
            legal_cards(&hand, &trick, Trump::Hearts),
            vec![Card::new(Ace, Hearts), Card::new(Nine, Hearts)]
        );
        // Following a plain suit: only that suit, in hand order.
        let trick = [Card::new(Ace, Clubs)];
        assert_eq!(
            legal_cards(&hand, &trick, Trump::Hearts),
            vec![Card::new(Eight, Clubs)]
        );
        // Empty trick: everything.
        assert_eq!(legal_cards(&hand, &[], Trump::Hearts), hand.to_vec());
    }
}
