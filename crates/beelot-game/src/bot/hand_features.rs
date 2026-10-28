//! The features of a hand that the all trumps and no trumps estimates weigh (US-064). The arena's
//! fitting tool fits the weights on bot play, and the rule-based bot applies them, so both count
//! the hand the same way. Port of the Java `HandFeatures`.

use crate::card::{Card, Rank, Suit, Trump};

/// A named count over a hand.
#[derive(Clone, Copy)]
pub struct Feature {
    pub name: &'static str,
    pub count: fn(&[Card]) -> i32,
}

const NO_TRUMPS_OFFENCE: [Feature; 5] = [
    Feature {
        name: "aces",
        count: |hand| count(hand, Rank::Ace),
    },
    Feature {
        name: "tens with their ace",
        count: |hand| with_all(hand, Rank::Ten, &[Rank::Ace]),
    },
    Feature {
        name: "tens without their ace",
        count: |hand| count(hand, Rank::Ten) - with_all(hand, Rank::Ten, &[Rank::Ace]),
    },
    Feature {
        name: "kings with the ace and ten",
        count: |hand| with_all(hand, Rank::King, &[Rank::Ace, Rank::Ten]),
    },
    Feature {
        name: "long cards",
        count: long_cards,
    },
];

const ALL_TRUMPS_OFFENCE: [Feature; 6] = [
    Feature {
        name: "jacks",
        count: |hand| count(hand, Rank::Jack),
    },
    Feature {
        name: "nines with their jack",
        count: |hand| with_all(hand, Rank::Nine, &[Rank::Jack]),
    },
    Feature {
        name: "nines without their jack",
        count: |hand| count(hand, Rank::Nine) - with_all(hand, Rank::Nine, &[Rank::Jack]),
    },
    Feature {
        name: "aces",
        count: |hand| count(hand, Rank::Ace),
    },
    Feature {
        name: "tens",
        count: |hand| count(hand, Rank::Ten),
    },
    Feature {
        name: "belotes",
        count: |hand| with_all(hand, Rank::King, &[Rank::Queen]),
    },
];

const ALL_TRUMPS_DEFENCE: [Feature; 3] = [
    Feature {
        name: "tens",
        count: |hand| count(hand, Rank::Ten),
    },
    Feature {
        name: "jacks",
        count: |hand| count(hand, Rank::Jack),
    },
    Feature {
        name: "nines",
        count: |hand| count(hand, Rank::Nine),
    },
];

const NO_TRUMPS_DEFENCE: [Feature; 3] = [
    Feature {
        name: "aces",
        count: |hand| count(hand, Rank::Ace),
    },
    Feature {
        name: "tens",
        count: |hand| count(hand, Rank::Ten),
    },
    Feature {
        name: "jacks",
        count: |hand| count(hand, Rank::Jack),
    },
];

/// The features of the taker's hand for an all trumps or no trumps contract.
///
/// # Panics
///
/// For a suit contract, which has no fitted features.
pub fn offence(trump: Trump) -> &'static [Feature] {
    match trump {
        Trump::NoTrumps => &NO_TRUMPS_OFFENCE,
        Trump::AllTrumps => &ALL_TRUMPS_OFFENCE,
        _ => panic!("A suit contract has no fitted features."),
    }
}

/// The features of a defender's hand against an all trumps or no trumps contract.
///
/// # Panics
///
/// For a suit contract, which has no fitted features.
pub fn defence(trump: Trump) -> &'static [Feature] {
    match trump {
        Trump::NoTrumps => &NO_TRUMPS_DEFENCE,
        Trump::AllTrumps => &ALL_TRUMPS_DEFENCE,
        _ => panic!("A suit contract has no fitted features."),
    }
}

fn count(hand: &[Card], rank: Rank) -> i32 {
    hand.iter().filter(|card| card.rank == rank).count() as i32
}

/// The cards of the given rank whose suit also holds every other given rank.
fn with_all(hand: &[Card], rank: Rank, others: &[Rank]) -> i32 {
    Suit::ALL
        .into_iter()
        .filter(|&suit| {
            hand.contains(&Card::new(rank, suit))
                && others
                    .iter()
                    .all(|&other| hand.contains(&Card::new(other, suit)))
        })
        .count() as i32
}

/// The cards beyond the fourth in each suit that holds the ace.
fn long_cards(hand: &[Card]) -> i32 {
    Suit::ALL
        .into_iter()
        .map(|suit| {
            let length = hand.iter().filter(|card| card.suit == suit).count() as i32;
            if length > 4 && hand.contains(&Card::new(Rank::Ace, suit)) {
                length - 4
            } else {
                0
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    //! Port of the Java `HandFeaturesTest`.

    use super::*;

    fn cards(codes: &str) -> Vec<Card> {
        codes
            .split_whitespace()
            .map(|code| code.parse().unwrap())
            .collect()
    }

    const HAND: &str = "AS 10S KS 9S 8S JH 9H QD";

    fn counts(features: &[Feature], hand: &[Card]) -> Vec<i32> {
        features
            .iter()
            .map(|feature| (feature.count)(hand))
            .collect()
    }

    #[test]
    fn counts_the_no_trumps_features() {
        assert_eq!(
            counts(offence(Trump::NoTrumps), &cards(HAND)),
            [1, 1, 0, 1, 1]
        );
    }

    #[test]
    fn counts_the_all_trumps_features() {
        assert_eq!(
            counts(offence(Trump::AllTrumps), &cards(HAND)),
            [1, 1, 1, 1, 1, 0]
        );
    }

    #[test]
    fn counts_a_belote_under_all_trumps() {
        let mut hand = cards(HAND);
        hand[4] = "KD".parse().unwrap();

        assert_eq!(counts(offence(Trump::AllTrumps), &hand).last(), Some(&1));
    }

    #[test]
    fn counts_the_defence_features() {
        assert_eq!(
            counts(defence(Trump::NoTrumps), &cards(HAND)),
            [1, 1, 1],
            "aces, tens and jacks"
        );
        assert_eq!(
            counts(defence(Trump::AllTrumps), &cards(HAND)),
            [1, 1, 2],
            "tens, jacks and nines"
        );
    }

    #[test]
    #[should_panic(expected = "A suit contract has no fitted features.")]
    fn a_suit_contract_has_no_offence_features() {
        offence(Trump::Hearts);
    }

    #[test]
    #[should_panic(expected = "A suit contract has no fitted features.")]
    fn a_suit_contract_has_no_defence_features() {
        defence(Trump::Hearts);
    }
}
