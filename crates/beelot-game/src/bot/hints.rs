//! Hints (US-065): the card or bid the Challenging bot would choose, and a reason for it. The sampling
//! search only returns a choice, so the reason is derived afterwards from the situation.

use serde::Serialize;

use super::memory::CardMemory;
use super::rule_based::by_value;
use crate::board::PlayView;
use crate::card::Card;
use crate::rules;

/// Why a card is the hint. Serialized as the code the client translates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CardReason {
    LeadTrumps,
    CashMaster,
    LeadLow,
    GivePoints,
    KeepPartnersTrick,
    WinCheaply,
    ThrowCheapest,
    BestForRound,
}

/// The first reason that holds for the card, or `BestForRound` when none does.
pub(crate) fn card_reason(view: &PlayView, card: Card) -> CardReason {
    let memory = CardMemory::new(view);
    if view.current_trick.is_empty() {
        lead_reason(view, &memory, card)
    } else {
        follow_reason(view, &memory, card)
    }
}

fn lead_reason(view: &PlayView, memory: &CardMemory, card: Card) -> CardReason {
    let by_value = by_value(view.trump);
    if view.trump.suit_contract() && view.trump.is_trump(card.suit) {
        CardReason::LeadTrumps
    } else if memory.master(card) {
        CardReason::CashMaster
    } else if !view
        .hand
        .iter()
        .any(|other| other.suit == card.suit && by_value(other, &card).is_lt())
    {
        CardReason::LeadLow
    } else {
        CardReason::BestForRound
    }
}

fn follow_reason(view: &PlayView, memory: &CardMemory, card: Card) -> CardReason {
    let by_value = by_value(view.trump);
    let cards: Vec<Card> = view
        .current_trick
        .iter()
        .map(|played| played.card)
        .collect();
    let winning = &view.current_trick[rules::winning_index(&cards, view.trump)];
    let lead = cards[0].suit;
    let beats = |contender: Card| rules::beats(contender, winning.card, lead, view.trump);
    let partner_winning = winning.seat == (view.seat + 2) % 4;
    let overtakes = beats(card);

    if partner_winning && !overtakes {
        let still_to_play = 3 - view.current_trick.len();
        let sure = (1..=still_to_play)
            .map(|offset| (view.seat + offset) % 4)
            .all(|seat| !memory.may_beat(seat, winning.card, lead));
        return if sure && rules::points(card, view.trump) > 0 {
            CardReason::GivePoints
        } else {
            CardReason::KeepPartnersTrick
        };
    }
    let cheaper = |other: &&Card| by_value(other, &card).is_lt();
    if overtakes {
        let cheaper_winner = view
            .legal_cards
            .iter()
            .filter(cheaper)
            .any(|&other| beats(other));
        if !cheaper_winner {
            return CardReason::WinCheaply;
        }
    } else if !view.legal_cards.iter().any(|other| cheaper(&other)) {
        return CardReason::ThrowCheapest;
    }
    CardReason::BestForRound
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::SeatCard;
    use crate::card::{Seat, Trump, Variant};

    fn card(code: &str) -> Card {
        code.parse().unwrap()
    }

    fn cards(codes: &str) -> Vec<Card> {
        codes.split_whitespace().map(card).collect()
    }

    /// The viewer sits at seat 0. `trick` lists the seat and card of each card already played.
    fn play_view(variant: Variant, trump: Trump, hand: &str, trick: &[(Seat, &str)]) -> PlayView {
        let hand = cards(hand);
        let current_trick: Vec<SeatCard> = trick
            .iter()
            .map(|&(seat, code)| SeatCard {
                seat,
                card: card(code),
            })
            .collect();
        let played: Vec<Card> = current_trick.iter().map(|played| played.card).collect();
        PlayView {
            seat: 0,
            legal_cards: rules::legal_cards(&hand, &played, trump),
            hand,
            trump,
            variant,
            contract_value: 0,
            coinched: false,
            declaring_seat: 0,
            tricks: Vec::new(),
            current_trick,
            announcements: false,
            revealed_announcements: Vec::new(),
        }
    }

    fn classic(hand: &str, trick: &[(Seat, &str)]) -> PlayView {
        play_view(Variant::Classic, Trump::Hearts, hand, trick)
    }

    #[test]
    fn leading_a_trump_leads_trumps() {
        let view = classic("JH 7C 8D", &[]);
        assert_eq!(card_reason(&view, card("JH")), CardReason::LeadTrumps);
    }

    #[test]
    fn leading_the_master_of_a_plain_suit_cashes_it() {
        let view = classic("AS 7C 8D", &[]);
        assert_eq!(card_reason(&view, card("AS")), CardReason::CashMaster);
    }

    #[test]
    fn leading_the_lowest_card_of_a_suit_leads_low() {
        let view = classic("7D KD 8C", &[]);
        assert_eq!(card_reason(&view, card("7D")), CardReason::LeadLow);
    }

    #[test]
    fn following_a_sure_trick_of_the_partner_gives_points() {
        let view = classic("10S 8S 7C", &[(2, "AS"), (3, "7S")]);
        assert_eq!(card_reason(&view, card("10S")), CardReason::GivePoints);
    }

    #[test]
    fn following_a_partner_who_may_be_beaten_keeps_the_trick() {
        let view = classic("7S 8S 7C", &[(2, "10S"), (3, "9S")]);
        assert_eq!(
            card_reason(&view, card("7S")),
            CardReason::KeepPartnersTrick
        );
    }

    #[test]
    fn winning_with_the_cheapest_winner_wins_cheaply() {
        let view = classic("KS AS 7C", &[(3, "9S")]);
        assert_eq!(card_reason(&view, card("KS")), CardReason::WinCheaply);
    }

    #[test]
    fn discarding_the_cheapest_card_throws_it() {
        let view = classic("8S 9S", &[(3, "AS")]);
        assert_eq!(card_reason(&view, card("8S")), CardReason::ThrowCheapest);
    }

    #[test]
    fn a_dearer_winner_than_necessary_falls_back() {
        let view = classic("10S QS 7C", &[(3, "9S")]);
        assert_eq!(card_reason(&view, card("10S")), CardReason::BestForRound);
    }

    #[test]
    fn a_forced_card_behind_an_opponent_is_thrown() {
        let view = classic("7S 8D", &[(3, "AS")]);
        assert_eq!(view.legal_cards, cards("7S"));
        assert_eq!(card_reason(&view, card("7S")), CardReason::ThrowCheapest);
    }

    #[test]
    fn no_suit_is_trump_under_sans_atout_or_tout_atout() {
        let no_trumps = play_view(Variant::Contree, Trump::NoTrumps, "AH 7C", &[]);
        assert_eq!(card_reason(&no_trumps, card("AH")), CardReason::CashMaster);

        let all_trumps = play_view(Variant::Contree, Trump::AllTrumps, "JH 7C", &[]);
        assert_eq!(card_reason(&all_trumps, card("JH")), CardReason::CashMaster);
    }
}
