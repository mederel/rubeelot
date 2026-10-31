//! Hints (US-065): the card or bid the Challenging bot would choose, and a reason for it. The sampling
//! search only returns a choice, so the reason is derived afterwards from the situation.

use serde::Serialize;

use super::memory::CardMemory;
use super::rule_based::by_value;
use super::strategies;
use crate::bidding::{AuctionDecision, AuctionView};
use crate::board::PlayView;
use crate::card::{Card, Difficulty};
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

/// Why a bid or pass is the hint. Serialized as the code the client translates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuctionReason {
    Take,
    SupportPartner,
    BidHandValue,
    Coinche,
    PassWeak,
}

pub(crate) fn auction_reason(view: &AuctionView, decision: &AuctionDecision) -> AuctionReason {
    match decision {
        AuctionDecision::Trump { .. } => AuctionReason::Take,
        AuctionDecision::Bid { trump, .. }
            if view.partner_holds_contract() && Some(*trump) == view.highest_bid_suit =>
        {
            AuctionReason::SupportPartner
        }
        AuctionDecision::Bid { .. } => AuctionReason::BidHandValue,
        AuctionDecision::Coinche => AuctionReason::Coinche,
        AuctionDecision::Pass => AuctionReason::PassWeak,
    }
}

/// The card the Challenging bot would play from this view, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardHint {
    pub card: Card,
    pub reason: CardReason,
}

/// The call the Challenging bot would make from this view, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuctionHint {
    pub decision: AuctionDecision,
    pub reason: AuctionReason,
}

pub fn card_hint(view: &PlayView) -> CardHint {
    let card = strategies::for_difficulty(Difficulty::Challenging).choose_card(view);
    CardHint {
        card,
        reason: card_reason(view, card),
    }
}

pub fn auction_hint(view: &AuctionView) -> AuctionHint {
    let decision = strategies::for_difficulty(Difficulty::Challenging).decide_auction(view);
    AuctionHint {
        decision,
        reason: auction_reason(view, &decision),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidding::BiddingState;
    use crate::board::{GameBoard, SeatCard};
    use crate::bot::turns::{play_turn, take_auction_turn};
    use crate::card::{Seat, Suit, Trump, Variant};
    use crate::rng::SplitMix64;

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

    fn auction_view(variant: Variant, partner_holds: Option<Trump>) -> AuctionView {
        AuctionView {
            seat: 1,
            hand: cards("JS 9S AS 10S AH AC 7D 8D"),
            upturned_card: (variant == Variant::Classic).then(|| card("7H")),
            round: 1,
            variant,
            dealer_index: 0,
            auction: Vec::new(),
            highest_bid: partner_holds.map_or(0, |_| 80),
            highest_bid_suit: partner_holds,
            highest_bidder_seat: partner_holds.map(|_| 3),
            coinche_allowed: false,
            announcements: false,
            all_trumps: true,
        }
    }

    #[test]
    fn choosing_a_trump_takes() {
        let first_round = auction_view(Variant::Classic, None);
        let take = AuctionDecision::Trump { suit: Suit::Hearts };
        assert_eq!(auction_reason(&first_round, &take), AuctionReason::Take);

        let second_round = AuctionView {
            round: 2,
            ..auction_view(Variant::Classic, None)
        };
        let other = AuctionDecision::Trump { suit: Suit::Spades };
        assert_eq!(auction_reason(&second_round, &other), AuctionReason::Take);
    }

    #[test]
    fn bidding_the_trump_the_partner_holds_supports_them() {
        let view = auction_view(Variant::Contree, Some(Trump::Hearts));
        let bid = AuctionDecision::Bid {
            value: 90,
            trump: Trump::Hearts,
        };
        assert_eq!(auction_reason(&view, &bid), AuctionReason::SupportPartner);
    }

    #[test]
    fn any_other_bid_is_worth_the_hand() {
        let spades = AuctionDecision::Bid {
            value: 90,
            trump: Trump::Spades,
        };
        let over_partner = auction_view(Variant::Contree, Some(Trump::Hearts));
        assert_eq!(
            auction_reason(&over_partner, &spades),
            AuctionReason::BidHandValue
        );
        let opening = auction_view(Variant::Contree, None);
        assert_eq!(
            auction_reason(&opening, &spades),
            AuctionReason::BidHandValue
        );
    }

    #[test]
    fn coinching_coinches() {
        let view = auction_view(Variant::Contree, None);
        assert_eq!(
            auction_reason(&view, &AuctionDecision::Coinche),
            AuctionReason::Coinche
        );
    }

    #[test]
    fn passing_passes_weak() {
        let view = auction_view(Variant::Contree, None);
        assert_eq!(
            auction_reason(&view, &AuctionDecision::Pass),
            AuctionReason::PassWeak
        );
    }

    fn dealt_board(seed: i64) -> GameBoard {
        let relaxed = strategies::for_difficulty(Difficulty::Relaxed);
        let mut bidding = BiddingState::new(Variant::Contree, 3, Box::new(SplitMix64::new(seed)));
        while bidding.completed_board().is_none() {
            take_auction_turn(&mut bidding, relaxed).unwrap();
        }
        bidding.into_board().unwrap()
    }

    #[test]
    fn a_card_hint_is_a_legal_card() {
        let relaxed = strategies::for_difficulty(Difficulty::Relaxed);
        let mut board = dealt_board(7);
        // The search starts at the third trick, so hint a few plays from there.
        for turn in 0..32 {
            if board.reviewing_completed_trick() {
                board.continue_after_trick().unwrap();
            }
            if board.round_result().is_some() {
                break;
            }
            let view = board.play_view(board.active_seat());
            if (12..16).contains(&turn) {
                let hint = card_hint(&view);
                assert!(
                    view.legal_cards.contains(&hint.card),
                    "turn {turn}: {hint:?}"
                );
            }
            play_turn(&mut board, relaxed).unwrap();
        }
    }

    #[test]
    fn an_auction_hint_is_a_legal_call() {
        for variant in [Variant::Contree, Variant::Classic] {
            for seed in 0..10 {
                let mut bidding = BiddingState::new(variant, 3, Box::new(SplitMix64::new(seed)));
                let seat = bidding.active_seat();
                let hint = auction_hint(&bidding.auction_view(seat));
                assert_eq!(
                    bidding.apply(seat, hint.decision),
                    Ok(()),
                    "{variant:?} {seed}"
                );
            }
        }
    }

    #[test]
    fn a_forced_card_is_the_hint() {
        let view = classic("7S 8D", &[(3, "AS")]);
        assert_eq!(
            card_hint(&view),
            CardHint {
                card: card("7S"),
                reason: CardReason::ThrowCheapest
            }
        );
    }
}
