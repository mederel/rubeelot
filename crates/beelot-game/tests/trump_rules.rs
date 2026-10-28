//! Card rules under all trumps and no trumps (US-063), porting Java's `TrumpRulesTest`.

use beelot_game::card::{Card, Rank, Suit, Trump};
use beelot_game::rules;

fn card(code: &str) -> Card {
    code.parse().unwrap()
}

fn cards(codes: &str) -> Vec<Card> {
    codes.split_whitespace().map(card).collect()
}

fn points(code: &str, trump: Trump) -> i32 {
    rules::points(card(code), trump)
}

#[test]
fn a_trump_names_its_suit_and_which_cards_are_trumps() {
    assert_eq!(Trump::from(Suit::Hearts), Trump::Hearts);
    assert!(Trump::Hearts.is_trump(Suit::Hearts));
    assert!(!Trump::Hearts.is_trump(Suit::Spades));
    for suit in Suit::ALL {
        assert!(Trump::AllTrumps.is_trump(suit));
        assert!(!Trump::NoTrumps.is_trump(suit));
    }
    assert_eq!(Trump::AllTrumps.suit(), None);
    assert_eq!(Trump::NoTrumps.suit(), None);
    assert_eq!(Trump::Hearts.suit(), Some(Suit::Hearts));
    for trump in Trump::ALL {
        assert_eq!(trump.suit().is_some(), trump.suit_contract(), "{trump:?}");
    }
    assert_eq!(Trump::AllTrumps.display_name(), "All trumps");
    assert_eq!(Trump::NoTrumps.display_name(), "No trumps");
    assert_eq!(Trump::Clubs.display_name(), "Clubs");
}

#[test]
fn each_contract_totals_152_card_points() {
    for trump in [Trump::Hearts, Trump::AllTrumps, Trump::NoTrumps] {
        let total: i32 = Card::deck()
            .into_iter()
            .map(|card| rules::points(card, trump))
            .sum();
        assert_eq!(total, 152, "{trump:?}");
    }
}

#[test]
fn no_trumps_values() {
    for (code, value) in [
        ("AC", 19),
        ("10C", 10),
        ("KC", 4),
        ("QC", 3),
        ("JC", 2),
        ("9C", 0),
        ("8C", 0),
    ] {
        assert_eq!(points(code, Trump::NoTrumps), value, "{code}");
    }
}

#[test]
fn all_trumps_values() {
    for (code, value) in [
        ("JC", 13),
        ("9D", 9),
        ("AH", 6),
        ("10H", 5),
        ("KS", 3),
        ("QS", 2),
        ("7S", 0),
    ] {
        assert_eq!(points(code, Trump::AllTrumps), value, "{code}");
    }
}

#[test]
fn no_trumps_order() {
    assert_eq!(rules::winning_index(&cards("10H AH"), Trump::NoTrumps), 1);
    assert_eq!(rules::winning_index(&cards("JH 9H"), Trump::NoTrumps), 0);
}

#[test]
fn all_trumps_order() {
    assert_eq!(rules::winning_index(&cards("AH 9H"), Trump::AllTrumps), 1);
    assert_eq!(rules::winning_index(&cards("9H JH"), Trump::AllTrumps), 1);
}

#[test]
fn another_suit_never_wins() {
    assert_eq!(rules::winning_index(&cards("7H JS"), Trump::AllTrumps), 0);
    assert_eq!(rules::winning_index(&cards("7H AS"), Trump::NoTrumps), 0);
}

#[test]
fn no_trumps_follows_without_overtaking() {
    let hand = cards("7H AH AS");
    assert_eq!(
        rules::legal_cards(&hand, &cards("KH"), Trump::NoTrumps),
        cards("7H AH")
    );
}

#[test]
fn no_trumps_discards_when_void() {
    let hand = cards("AS 7C");
    assert_eq!(
        rules::legal_cards(&hand, &cards("KH"), Trump::NoTrumps),
        hand
    );
}

#[test]
fn all_trumps_must_overtake() {
    let hand = cards("7H 9H JS");
    assert_eq!(
        rules::legal_cards(&hand, &cards("AH"), Trump::AllTrumps),
        cards("9H")
    );
}

#[test]
fn all_trumps_follows_low_when_it_cannot_beat() {
    let hand = cards("7H 8H AS");
    assert_eq!(
        rules::legal_cards(&hand, &cards("JH"), Trump::AllTrumps),
        cards("7H 8H")
    );
}

#[test]
fn all_trumps_discards_when_void() {
    let hand = cards("JS 7C");
    assert_eq!(
        rules::legal_cards(&hand, &cards("AH"), Trump::AllTrumps),
        hand
    );
}

#[test]
fn all_trumps_overtakes_the_winner_not_the_last_card() {
    let hand = cards("AH JH");
    assert_eq!(
        rules::legal_cards(&hand, &cards("9H 10H"), Trump::AllTrumps),
        cards("JH")
    );
}

#[test]
fn the_display_order_follows_the_contract() {
    let suit_by_suit = |order: [Rank; 8]| -> Vec<Card> {
        Suit::ALL
            .into_iter()
            .flat_map(|suit| order.map(|rank| Card::new(rank, suit)))
            .collect()
    };
    let mut deck = Card::deck();
    deck.reverse();
    deck.sort_by_key(|card| card.display_key(Some(Trump::AllTrumps)));
    assert_eq!(
        deck,
        suit_by_suit([
            Rank::Seven,
            Rank::Eight,
            Rank::Queen,
            Rank::King,
            Rank::Ten,
            Rank::Ace,
            Rank::Nine,
            Rank::Jack
        ])
    );
    deck.sort_by_key(|card| card.display_key(Some(Trump::NoTrumps)));
    assert_eq!(
        deck,
        suit_by_suit([
            Rank::Seven,
            Rank::Eight,
            Rank::Nine,
            Rank::Jack,
            Rank::Queen,
            Rank::King,
            Rank::Ten,
            Rank::Ace
        ])
    );
}
