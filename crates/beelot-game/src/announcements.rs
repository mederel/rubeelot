//! Finds the announcements of a hand and the best of a round's declarations (US-060). Carrés come
//! first: sequences are formed from the cards left over, so a card counts in one announcement at
//! most.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::card::{Card, Rank, Seat, Suit, Trump};

/// The order of sequences, whatever the trump; also the order of top cards when announcements are
/// compared.
const SEQUENCE_ORDER: [Rank; 8] = [
    Rank::Seven,
    Rank::Eight,
    Rank::Nine,
    Rank::Ten,
    Rank::Jack,
    Rank::Queen,
    Rank::King,
    Rank::Ace,
];

const CARRE_VALUES: [(Rank, i32); 6] = [
    (Rank::Jack, 200),
    (Rank::Nine, 150),
    (Rank::Ace, 100),
    (Rank::Ten, 100),
    (Rank::King, 100),
    (Rank::Queen, 100),
];

/// Under no trumps, carrés follow the plain card order (US-063).
const NO_TRUMPS_CARRE_VALUES: [(Rank, i32); 6] = [
    (Rank::Ace, 200),
    (Rank::Ten, 150),
    (Rank::King, 100),
    (Rank::Queen, 100),
    (Rank::Jack, 100),
    (Rank::Nine, 100),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnnouncementKind {
    Tierce,
    Cinquante,
    Cent,
    Carre,
}

/// A tierce, cinquante, cent or carré. A sequence lists its cards from the lowest; a carré has no
/// suit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announcement {
    pub kind: AnnouncementKind,
    pub cards: Vec<Card>,
    pub value: i32,
    pub suit: Option<Suit>,
}

impl Announcement {
    /// The highest card, in the order A, K, Q, J, 10, 9, 8, 7; the first one on a tie.
    pub fn top_card(&self) -> Card {
        let mut top = self.cards[0];
        for &card in &self.cards[1..] {
            if rank_height(card.rank) > rank_height(top.rank) {
                top = card;
            }
        }
        top
    }
}

/// A seat's declared announcements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declared {
    pub seat: Seat,
    pub announcements: Vec<Announcement>,
}

/// Every announcement of the hand, strongest first; carrés are worth more in other ranks under no
/// trumps.
pub fn find(hand: &[Card], trump: Trump) -> Vec<Announcement> {
    let mut found = Vec::new();
    let mut left: Vec<Card> = hand.to_vec();
    let carre_values = if trump == Trump::NoTrumps {
        NO_TRUMPS_CARRE_VALUES
    } else {
        CARRE_VALUES
    };
    for (rank, value) in carre_values {
        let four: Vec<Card> = left.iter().copied().filter(|c| c.rank == rank).collect();
        if four.len() == 4 {
            left.retain(|card| !four.contains(card));
            found.push(Announcement {
                kind: AnnouncementKind::Carre,
                cards: four,
                value,
                suit: None,
            });
        }
    }
    for suit in Suit::ALL {
        let mut run = Vec::new();
        for rank in SEQUENCE_ORDER {
            let card = Card::new(rank, suit);
            if left.contains(&card) {
                run.push(card);
            } else {
                add_sequence(&mut found, std::mem::take(&mut run), suit);
            }
        }
        add_sequence(&mut found, run, suit);
    }
    found.sort_by(|a, b| compare_strength(a, b, Some(trump)).reverse());
    found
}

/// Weakest first: the value, then the length of the run (a carré counts 4), then the top card,
/// then the trump suit, which only counts in a suit contract. Declaring first, the last tie-break,
/// depends on the round and is left to [`best_seat`].
pub fn compare_strength(a: &Announcement, b: &Announcement, trump: Option<Trump>) -> Ordering {
    let trump_suit = trump.and_then(Trump::suit);
    let in_trump =
        |announcement: &Announcement| trump_suit.is_some() && announcement.suit == trump_suit;
    a.value
        .cmp(&b.value)
        .then(a.cards.len().cmp(&b.cards.len()))
        .then(rank_height(a.top_card().rank).cmp(&rank_height(b.top_card().rank)))
        .then(in_trump(a).cmp(&in_trump(b)))
}

/// The seat holding the strongest announcement; on a full tie, the one that declared first.
pub fn best_seat(in_declaration_order: &[Declared], trump: Trump) -> Option<Seat> {
    let mut best: Option<(Seat, &Announcement)> = None;
    for declared in in_declaration_order {
        for announcement in &declared.announcements {
            let stronger = best.is_none_or(|(_, current)| {
                compare_strength(announcement, current, Some(trump)) == Ordering::Greater
            });
            if stronger {
                best = Some((declared.seat, announcement));
            }
        }
    }
    best.map(|(seat, _)| seat)
}

pub fn rank_height(rank: Rank) -> i32 {
    SEQUENCE_ORDER
        .iter()
        .position(|&r| r == rank)
        .map_or(-1, |index| index as i32)
}

fn add_sequence(found: &mut Vec<Announcement>, run: Vec<Card>, suit: Suit) {
    if run.len() < 3 {
        return;
    }
    let (kind, value) = match run.len() {
        3 => (AnnouncementKind::Tierce, 20),
        4 => (AnnouncementKind::Cinquante, 50),
        _ => (AnnouncementKind::Cent, 100),
    };
    found.push(Announcement {
        kind,
        cards: run,
        value,
        suit: Some(suit),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Rank::*, Suit::*};

    fn card(rank: Rank, suit: Suit) -> Card {
        Card::new(rank, suit)
    }

    fn single(hand: &[Card]) -> Announcement {
        let found = find(hand, Trump::AllTrumps);
        assert_eq!(found.len(), 1, "announcements in {hand:?}: {found:?}");
        found.into_iter().next().unwrap()
    }

    fn carre(rank: Rank) -> Announcement {
        single(&[
            card(rank, Clubs),
            card(rank, Diamonds),
            card(rank, Hearts),
            card(rank, Spades),
        ])
    }

    fn assert_carre(rank: Rank, value: i32) {
        let carre = carre(rank);
        assert_eq!(carre.kind, AnnouncementKind::Carre, "{rank:?}");
        assert_eq!(carre.value, value, "{rank:?}");
    }

    #[test]
    fn carres_score_by_rank() {
        assert_carre(Jack, 200);
        assert_carre(Nine, 150);
        for rank in [Ace, Ten, King, Queen] {
            assert_carre(rank, 100);
        }
    }

    #[test]
    fn carres_of_eights_and_sevens_dont_count() {
        assert!(
            find(
                &[
                    card(Eight, Clubs),
                    card(Eight, Diamonds),
                    card(Eight, Hearts),
                    card(Eight, Spades),
                    card(Seven, Clubs),
                    card(Seven, Diamonds),
                    card(Seven, Hearts),
                    card(Seven, Spades),
                ],
                Trump::AllTrumps
            )
            .is_empty()
        );
    }

    #[test]
    fn sequences_by_length() {
        let tierce = single(&[
            card(Eight, Hearts),
            card(Nine, Hearts),
            card(Ten, Hearts),
            card(Ace, Clubs),
        ]);
        assert_eq!(tierce.kind, AnnouncementKind::Tierce);
        assert_eq!(tierce.value, 20);
        assert_eq!(tierce.top_card(), card(Ten, Hearts));
        assert_eq!(tierce.suit, Some(Hearts));

        let cinquante = single(&[
            card(Nine, Spades),
            card(Ten, Spades),
            card(Jack, Spades),
            card(Queen, Spades),
        ]);
        assert_eq!(cinquante.kind, AnnouncementKind::Cinquante);
        assert_eq!(cinquante.value, 50);

        let cent = single(&[
            card(Seven, Diamonds),
            card(Eight, Diamonds),
            card(Nine, Diamonds),
            card(Ten, Diamonds),
            card(Jack, Diamonds),
        ]);
        assert_eq!(cent.kind, AnnouncementKind::Cent);
        assert_eq!(cent.value, 100);

        let all_clubs: Vec<Card> = [Seven, Eight, Nine, Ten, Jack, Queen, King, Ace]
            .map(|rank| card(rank, Clubs))
            .to_vec();
        let eight = single(&all_clubs);
        assert_eq!(eight.kind, AnnouncementKind::Cent);
        assert_eq!(eight.cards.len(), 8);
        assert_eq!(eight.top_card(), card(Ace, Clubs));
    }

    #[test]
    fn sequences_follow_the_natural_order_whatever_the_trump() {
        assert_eq!(
            single(&[card(Nine, Hearts), card(Ten, Hearts), card(Jack, Hearts)]).kind,
            AnnouncementKind::Tierce
        );
        assert_eq!(
            single(&[card(Queen, Hearts), card(King, Hearts), card(Ace, Hearts)]).top_card(),
            card(Ace, Hearts)
        );
    }

    #[test]
    fn carre_takes_a_card_a_sequence_would_use() {
        let found = find(
            &[
                card(King, Clubs),
                card(King, Diamonds),
                card(King, Hearts),
                card(King, Spades),
                card(Queen, Hearts),
                card(Jack, Hearts),
            ],
            Trump::AllTrumps,
        );

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, AnnouncementKind::Carre);
        assert_eq!(found[0].suit, None);
    }

    #[test]
    fn several_announcements_in_one_hand() {
        let found = find(
            &[
                card(Seven, Spades),
                card(Eight, Spades),
                card(Nine, Spades),
                card(Jack, Diamonds),
                card(Queen, Diamonds),
                card(King, Diamonds),
                card(Ace, Clubs),
                card(Seven, Hearts),
            ],
            Trump::AllTrumps,
        );

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].top_card(), card(King, Diamonds));
        assert_eq!(found[1].top_card(), card(Nine, Spades));
    }

    #[test]
    fn strength_order() {
        let cent_to_the_jack = single(&[
            card(Seven, Diamonds),
            card(Eight, Diamonds),
            card(Nine, Diamonds),
            card(Ten, Diamonds),
            card(Jack, Diamonds),
        ]);
        let carre_of_kings = carre(King);
        let carre_of_jacks = carre(Jack);
        let tierce_to_the_ace = single(&[card(Queen, Clubs), card(King, Clubs), card(Ace, Clubs)]);
        let tierce_to_the_king =
            single(&[card(Jack, Clubs), card(Queen, Clubs), card(King, Clubs)]);
        let trump_tierce_to_the_king =
            single(&[card(Jack, Hearts), card(Queen, Hearts), card(King, Hearts)]);
        let strength =
            |a: &Announcement, b: &Announcement| compare_strength(a, b, Some(Trump::Hearts));

        assert_eq!(
            strength(&cent_to_the_jack, &carre_of_kings),
            Ordering::Greater
        );
        assert_eq!(
            strength(&tierce_to_the_ace, &tierce_to_the_king),
            Ordering::Greater
        );
        assert_eq!(
            strength(&trump_tierce_to_the_king, &tierce_to_the_king),
            Ordering::Greater
        );
        assert_eq!(
            strength(&carre_of_jacks, &cent_to_the_jack),
            Ordering::Greater
        );
    }

    #[test]
    fn best_seat_picks_the_strongest_and_the_first_on_a_tie() {
        let clubs_tierce = single(&[card(Jack, Clubs), card(Queen, Clubs), card(King, Clubs)]);
        let diamonds_tierce = single(&[
            card(Jack, Diamonds),
            card(Queen, Diamonds),
            card(King, Diamonds),
        ]);
        let cinquante = single(&[
            card(Nine, Spades),
            card(Ten, Spades),
            card(Jack, Spades),
            card(Queen, Spades),
        ]);
        let first = Declared {
            seat: 1,
            announcements: vec![clubs_tierce],
        };
        let second = Declared {
            seat: 2,
            announcements: vec![diamonds_tierce],
        };
        let third = Declared {
            seat: 3,
            announcements: vec![cinquante],
        };

        assert_eq!(
            best_seat(&[first.clone(), second.clone()], Trump::Hearts),
            Some(1)
        );
        assert_eq!(best_seat(&[first, second, third], Trump::Hearts), Some(3));
        assert_eq!(best_seat(&[], Trump::Hearts), None);
    }

    fn assert_carre_in(rank: Rank, value: i32, trump: Trump) {
        let found = find(
            &[
                card(rank, Clubs),
                card(rank, Diamonds),
                card(rank, Hearts),
                card(rank, Spades),
            ],
            trump,
        );
        assert_eq!(found.len(), 1, "{rank:?} {trump:?}");
        assert_eq!(found[0].value, value, "{rank:?} {trump:?}");
    }

    #[test]
    fn no_trumps_carres() {
        assert_carre_in(Ace, 200, Trump::NoTrumps);
        assert_carre_in(Ten, 150, Trump::NoTrumps);
        assert_carre_in(Nine, 100, Trump::NoTrumps);
        assert_carre_in(Jack, 100, Trump::NoTrumps);
        assert_carre_in(Jack, 200, Trump::AllTrumps);
        assert_carre_in(Nine, 150, Trump::AllTrumps);
    }

    #[test]
    fn no_trump_tie_break_outside_suit_contracts() {
        let clubs_tierce = single(&[card(Queen, Clubs), card(King, Clubs), card(Ace, Clubs)]);
        let hearts_tierce = single(&[card(Queen, Hearts), card(King, Hearts), card(Ace, Hearts)]);
        let declared = [
            Declared {
                seat: 1,
                announcements: vec![clubs_tierce],
            },
            Declared {
                seat: 2,
                announcements: vec![hearts_tierce],
            },
        ];

        assert_eq!(best_seat(&declared, Trump::AllTrumps), Some(1));
        assert_eq!(best_seat(&declared, Trump::NoTrumps), Some(1));
        assert_eq!(best_seat(&declared, Trump::Hearts), Some(2));
    }
}
