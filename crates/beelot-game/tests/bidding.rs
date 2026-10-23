//! Ports of the Java BiddingStateTest and ContreeBiddingStateTest, plus redeal and seed checks.
//! Dropped as having no counterpart in the Rust API: English messages and player names, the
//! `auctionId` and `AuctionCall` log assertions (the seat-call log is checked instead), the
//! "four players" constructor check (a table is always four seats), the null-suit bid (the suit
//! is a required parameter), and `viewFor` fields that are player-facing (`complete`,
//! `playerTurn`, `activePlayer`), covered through `completed_board()` and `active_seat()`.

use beelot_game::bidding::{AuctionDecision, BiddingState, CallType, SeatCall};
use beelot_game::board::GameBoard;
use beelot_game::card::{Card, Suit, Variant};
use beelot_game::error::GameError;
use beelot_game::rng::JavaRandom;
use beelot_game::scoring::RoundResult;

fn bidding(variant: Variant, dealer: usize, seed: i64) -> BiddingState {
    BiddingState::new(variant, dealer, Box::new(JavaRandom::new(seed)))
}

fn contree() -> BiddingState {
    bidding(Variant::Contree, 3, 11)
}

fn pass_active(state: &mut BiddingState) {
    let seat = state.active_seat();
    state.pass(seat).unwrap();
}

fn other_suit(suit: Suit) -> Suit {
    if suit == Suit::Hearts {
        Suit::Spades
    } else {
        Suit::Hearts
    }
}

#[test]
fn bots_see_whether_the_round_plays_with_announcements() {
    let on = bidding(Variant::Contree, 0, 1).with_announcements(true);
    let off = bidding(Variant::Contree, 0, 1);
    assert!(on.auction_view(0).announcements);
    assert!(!off.auction_view(0).announcements);
}

#[test]
fn passing_twice_around_the_table_redeals_the_cards() {
    let mut state = bidding(Variant::Classic, 3, 2);
    for _ in 0..4 {
        pass_active(&mut state);
    }
    assert_eq!(state.round(), 2);
    for _ in 0..4 {
        pass_active(&mut state);
    }
    assert_eq!(state.round(), 1);
    assert_eq!(state.hand(0).len(), 5);
    assert_eq!(state.deals(), 2);
}

#[test]
fn the_auction_log_keeps_every_call_in_order_until_the_cards_are_redealt() {
    let mut state = bidding(Variant::Classic, 3, 3);
    for _ in 0..5 {
        pass_active(&mut state);
    }
    let expected: Vec<SeatCall> = [0, 1, 2, 3, 0]
        .into_iter()
        .map(|seat| SeatCall {
            seat,
            r#type: CallType::Pass,
            value: 0,
            suit: None,
        })
        .collect();
    assert_eq!(state.auction(), expected.as_slice());
    for _ in 0..3 {
        pass_active(&mut state);
    }
    assert!(state.auction().is_empty());
}

#[test]
fn player_to_the_dealers_left_speaks_first_and_leads_the_first_trick() {
    let mut state = bidding(Variant::Classic, 1, 4);
    assert_eq!(state.dealer(), 1);
    assert_eq!(state.active_seat(), 2);
    let taker = state.active_seat();
    let suit = state.upturned_card().unwrap().suit;
    state.choose_trump(taker, suit).unwrap();
    assert_eq!(state.completed_board().unwrap().active_seat(), 2);
}

#[test]
fn redealing_passes_the_deal_to_the_next_player() {
    let mut state = bidding(Variant::Classic, 3, 5);
    for _ in 0..8 {
        pass_active(&mut state);
    }
    assert_eq!(state.dealer(), 0);
    assert_eq!(state.active_seat(), 1);
}

#[test]
fn classic_double_pass_redeals_and_moves_dealer() {
    let mut state = bidding(Variant::Classic, 3, 6);
    for _ in 0..8 {
        pass_active(&mut state);
    }
    assert_eq!(state.deals(), 2);
    assert_eq!(state.dealer(), 0);
    assert_eq!(state.active_seat(), 1);
}

#[test]
fn the_same_seed_gives_the_same_deals() {
    let first = bidding(Variant::Contree, 3, 5);
    let second = bidding(Variant::Contree, 3, 5);
    let other = bidding(Variant::Contree, 3, 6);
    for seat in 0..4 {
        assert_eq!(first.hand(seat), second.hand(seat));
    }
    assert_ne!(first.hand(0), other.hand(0));
}

#[test]
fn same_seed_same_hands() {
    let first = bidding(Variant::Classic, 0, 7);
    let second = bidding(Variant::Classic, 0, 7);
    for seat in 0..4 {
        assert_eq!(first.hand(seat), second.hand(seat));
    }
    assert_eq!(first.upturned_card(), second.upturned_card());
}

#[test]
fn the_first_round_only_accepts_the_upturned_suit_and_the_second_any_other() {
    let mut state = bidding(Variant::Classic, 3, 8);
    let first = state.active_seat();
    let upturned = state.upturned_card().unwrap().suit;
    let other = other_suit(upturned);

    assert_eq!(
        state.choose_trump(first, other),
        Err(GameError::MustTakeUpturnedSuit)
    );
    for _ in 0..4 {
        pass_active(&mut state);
    }
    assert_eq!(
        state.choose_trump(first, upturned),
        Err(GameError::MustChooseAnotherSuit)
    );

    state.choose_trump(first, other).unwrap();
    assert_eq!(state.completed_board().unwrap().trump(), other);
}

#[test]
fn only_the_player_whose_turn_it_is_may_speak_until_the_auction_ends() {
    let mut state = bidding(Variant::Classic, 3, 9);
    let first = state.active_seat();
    assert_eq!(state.pass(1), Err(GameError::NotYourTurn));

    let suit = state.upturned_card().unwrap().suit;
    state.choose_trump(first, suit).unwrap();

    let active = state.active_seat();
    assert_eq!(state.pass(active), Err(GameError::AuctionEnded));
    assert!(state.completed_board().is_some());
}

#[test]
fn each_variant_only_accepts_its_own_calls() {
    let mut classic = bidding(Variant::Classic, 3, 10);
    let mut contree = bidding(Variant::Contree, 3, 10);
    let classic_turn = classic.active_seat();
    let contree_turn = contree.active_seat();

    assert_eq!(
        classic.bid(classic_turn, 80, Suit::Hearts),
        Err(GameError::ContractOnlyInContree)
    );
    assert_eq!(
        classic.coinche(classic_turn),
        Err(GameError::ContractOnlyInContree)
    );
    assert_eq!(
        contree.choose_trump(contree_turn, Suit::Hearts),
        Err(GameError::TrumpChoiceOnlyInClassic)
    );
}

#[test]
fn nobody_may_coinche_before_the_first_bid() {
    let mut state = contree();
    let first = state.active_seat();
    assert_eq!(state.coinche(first), Err(GameError::CoincheNotAllowed));
    assert!(!state.auction_view(first).coinche_allowed);
}

#[test]
fn completed_board_keeps_the_announcements_flag() {
    let mut state = bidding(Variant::Contree, 3, 12).with_announcements(true);
    let seat = state.active_seat();
    state.bid(seat, 80, Suit::Hearts).unwrap();
    for _ in 0..3 {
        pass_active(&mut state);
    }
    assert!(state.announcements());
    assert!(state.completed_board().unwrap().play_view(0).announcements);
}

#[test]
fn deals_all_cards_and_closes_a_contract_after_three_passes() {
    let mut state = contree();
    assert_eq!(state.hand(0).len(), 8);
    assert_eq!(state.upturned_card(), None);

    state.bid(0, 90, Suit::Hearts).unwrap();
    state.pass(1).unwrap();
    state.pass(2).unwrap();
    state.pass(3).unwrap();

    let board = state.completed_board().unwrap();
    assert_eq!(board.variant(), Variant::Contree);
    assert_eq!(board.contract_value(), 90);
    assert_eq!(board.trump(), Suit::Hearts);
    assert!(!board.coinched());
}

#[test]
fn opponent_can_coinche_but_declarers_partner_cannot() {
    let mut coinched = contree();
    coinched.bid(0, 80, Suit::Clubs).unwrap();
    assert!(coinched.auction_view(1).coinche_allowed);
    assert!(coinched.can_coinche(1));
    coinched.coinche(1).unwrap();
    assert!(coinched.completed_board().unwrap().coinched());

    let mut teammate_turn = contree();
    teammate_turn.bid(0, 80, Suit::Clubs).unwrap();
    teammate_turn.pass(1).unwrap();
    assert_eq!(teammate_turn.coinche(2), Err(GameError::CoincheNotAllowed));
}

#[test]
fn bids_must_increase_by_ten_within_the_supported_range() {
    let mut state = contree();
    assert_eq!(state.bid(0, 85, Suit::Spades), Err(GameError::InvalidBid));
    assert_eq!(state.bid(0, 70, Suit::Spades), Err(GameError::InvalidBid));
    assert_eq!(state.bid(0, 170, Suit::Spades), Err(GameError::InvalidBid));
    state.bid(0, 100, Suit::Spades).unwrap();
    assert_eq!(state.bid(1, 100, Suit::Hearts), Err(GameError::BidTooLow));
}

#[test]
fn four_opening_passes_redeal_with_the_next_dealer_and_the_turn_moves_on() {
    let mut state = contree();
    for seat in 0..4 {
        state.pass(seat).unwrap();
    }
    assert_eq!(state.hand(0).len(), 8);
    assert_eq!(state.highest_bid(), 0);
    assert_eq!(state.auction_view(0).highest_bidder_seat, None);
    assert_eq!(state.dealer(), 0);
    assert_eq!(state.active_seat(), 1);
    assert!(state.completed_board().is_none());
    assert_eq!(state.deals(), 2);
}

#[test]
fn a_raise_restarts_the_three_pass_closure_count() {
    let mut state = contree();
    state.bid(0, 80, Suit::Clubs).unwrap();
    state.pass(1).unwrap();
    state.bid(2, 100, Suit::Hearts).unwrap();
    state.pass(3).unwrap();
    state.pass(0).unwrap();

    assert!(state.completed_board().is_none());
    state.pass(1).unwrap();
    let board = state.completed_board().unwrap();
    assert_eq!(board.contract_value(), 100);
    assert_eq!(board.declarer() % 2, 0);
}

fn complete_hands() -> [Vec<Card>; 4] {
    let deck = Card::deck();
    std::array::from_fn(|seat| deck[seat * 8..seat * 8 + 8].to_vec())
}

fn play_out(board: &mut GameBoard) -> RoundResult {
    while board.round_result().is_none() {
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            let seat = board.active_seat();
            let card = board.legal_cards(seat)[0];
            board.play(seat, card, false).unwrap();
        }
    }
    board.round_result().unwrap().clone()
}

#[test]
fn coinche_doubles_the_awarded_round_scores() {
    let mut normal =
        GameBoard::from_contract(complete_hands(), Suit::Hearts, 0, 100, false, 3, false);
    let mut doubled =
        GameBoard::from_contract(complete_hands(), Suit::Hearts, 0, 100, true, 3, false);

    let normal_result = play_out(&mut normal);
    let doubled_result = play_out(&mut doubled);

    assert_eq!(
        normal_result.north_south_awarded * 2,
        doubled_result.north_south_awarded
    );
    assert_eq!(
        normal_result.east_west_awarded * 2,
        doubled_result.east_west_awarded
    );
}

#[test]
fn contract_outcome_uses_the_called_value_and_declaring_team() {
    let mut north_south =
        GameBoard::from_contract(complete_hands(), Suit::Hearts, 0, 160, false, 3, false);
    let mut east_west =
        GameBoard::from_contract(complete_hands(), Suit::Hearts, 1, 80, false, 3, false);

    assert!(play_out(&mut north_south).contract_made);
    assert!(!play_out(&mut east_west).contract_made);
}

#[test]
fn apply_dispatches_decisions_and_serde_matches_the_fixtures() {
    let mut state = contree();
    state
        .apply(
            0,
            AuctionDecision::Bid {
                value: 80,
                suit: Suit::Hearts,
            },
        )
        .unwrap();
    let view = state.auction_view(1);
    assert_eq!(view.highest_bidder_seat, Some(0));
    assert!(!view.partner_holds_contract());
    assert!(state.auction_view(2).partner_holds_contract());
    assert!(state.auction_view(0).has_bid());
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["highestBidderSeat"], 0);
    assert_eq!(json["auction"][0]["type"], "BID");
    assert_eq!(
        serde_json::from_value::<beelot_game::bidding::AuctionView>(json).unwrap(),
        view
    );

    let fresh = serde_json::to_value(contree().auction_view(0)).unwrap();
    assert_eq!(fresh["highestBidderSeat"], -1);

    for (text, decision) in [
        (r#"{"type":"PASS"}"#, AuctionDecision::Pass),
        (r#"{"type":"COINCHE"}"#, AuctionDecision::Coinche),
        (
            r#"{"type":"BID","value":90,"suit":"HEARTS"}"#,
            AuctionDecision::Bid {
                value: 90,
                suit: Suit::Hearts,
            },
        ),
        (
            r#"{"type":"TRUMP","suit":"SPADES"}"#,
            AuctionDecision::Trump { suit: Suit::Spades },
        ),
    ] {
        assert_eq!(
            serde_json::from_str::<AuctionDecision>(text).unwrap(),
            decision
        );
    }

    let mut classic = bidding(Variant::Classic, 3, 13);
    let suit = classic.upturned_card().unwrap().suit;
    classic.apply(0, AuctionDecision::Trump { suit }).unwrap();
    assert_eq!(classic.completed_board().unwrap().hand(0).len(), 8);
    assert!(classic.into_board().is_some());
}
