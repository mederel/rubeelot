//! Ports of the Java GameBoard tests: rules, belote, capot, last trick and announcements.
//! Dropped as having no counterpart in the Rust API: English messages, player names, the
//! "not seated" view error, and view fields that are not in `PlayView` (declarers, own
//! announcements), which are covered through `declared()` and `announcements::find`.

use beelot_game::announcements::{self, AnnouncementKind};
use beelot_game::board::{Belote, BeloteDeclaration, GameBoard};
use beelot_game::card::{Card, Suit, Team, Trump, Variant};
use beelot_game::error::GameError;
use beelot_game::scoring::RoundResult;

fn cards(codes: &str) -> Vec<Card> {
    codes.split(' ').map(|code| code.parse().unwrap()).collect()
}

fn card(code: &str) -> Card {
    code.parse().unwrap()
}

/// Deals the deck one card at a time around the table: two cards of each suit per seat.
fn round_robin_deal() -> [Vec<Card>; 4] {
    let mut hands: [Vec<Card>; 4] = Default::default();
    for (index, card) in Card::deck().into_iter().enumerate() {
        hands[index % 4].push(card);
    }
    hands
}

/// Plays every seat's first legal card until the round ends.
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

// ---- rules ----

fn rules_board() -> GameBoard {
    GameBoard::from_bidding(round_robin_deal(), Suit::Hearts, 0, 3, false)
}

#[test]
fn only_the_player_whose_turn_it_is_may_play() {
    let mut board = rules_board();
    let bens_card = board.hand(1)[0];

    assert_eq!(board.play(1, bens_card, false), Err(GameError::NotYourTurn));
    assert_eq!(board.hand(1).len(), 8);
    assert!(board.play_view(1).legal_cards.is_empty());
}

#[test]
fn a_player_cannot_play_a_card_they_do_not_hold() {
    let mut board = rules_board();
    let bens_card = board.hand(1)[0];

    assert_eq!(board.play(0, bens_card, false), Err(GameError::IllegalCard));
}

#[test]
fn a_player_holding_the_led_suit_must_follow_it() {
    let mut board = rules_board();
    let lead = *board
        .hand(0)
        .iter()
        .find(|c| c.suit == Suit::Spades)
        .unwrap();
    board.play(0, lead, false).unwrap();
    let off_suit = *board
        .hand(1)
        .iter()
        .find(|c| c.suit == Suit::Clubs)
        .unwrap();

    assert_eq!(board.play(1, off_suit, false), Err(GameError::IllegalCard));
    assert!(
        board
            .play_view(1)
            .legal_cards
            .iter()
            .all(|c| c.suit == Suit::Spades)
    );
    assert_eq!(board.play_view(1).current_trick.len(), 1);
}

#[test]
fn the_game_moves_on_from_a_trick_only_once_it_is_complete() {
    let mut board = rules_board();

    assert_eq!(
        board.continue_after_trick(),
        Err(GameError::NoCompletedTrick)
    );
}

#[test]
fn nothing_moves_on_once_the_round_has_ended() {
    let mut board = rules_board();
    play_out(&mut board);
    assert!(board.round_summary().is_some());

    assert_eq!(board.continue_after_trick(), Err(GameError::RoundEnded));
}

#[test]
#[should_panic]
fn a_round_needs_eight_cards_each_bidding() {
    let mut hands = round_robin_deal();
    hands[1].truncate(7);
    GameBoard::from_bidding(hands, Suit::Hearts, 0, 3, false);
}

#[test]
#[should_panic]
fn a_round_needs_eight_cards_each_contract() {
    let mut hands = round_robin_deal();
    hands[1].truncate(7);
    GameBoard::from_contract(hands, Trump::Hearts, 0, 80, false, 3, false);
}

#[test]
fn a_contree_contract_is_from_80_to_160_in_steps_of_ten() {
    for invalid in [70, 85, 170] {
        let result = std::panic::catch_unwind(|| {
            GameBoard::from_contract(
                round_robin_deal(),
                Trump::Hearts,
                0,
                invalid,
                false,
                3,
                false,
            )
        });
        assert!(result.is_err(), "{invalid} should be refused");
    }

    let board =
        GameBoard::from_contract(round_robin_deal(), Trump::Hearts, 0, 160, false, 3, false);
    assert_eq!(board.contract_value(), 160);
    assert_eq!(board.variant(), Variant::Contree);
}

#[test]
fn a_classic_board_has_contract_value_82_and_the_dealers_left_leads() {
    let board = rules_board();
    assert_eq!(board.contract_value(), 82);
    assert_eq!(board.variant(), Variant::Classic);
    assert_eq!(board.active_seat(), 0);
    let board = GameBoard::from_bidding(round_robin_deal(), Suit::Hearts, 0, 1, false);
    assert_eq!(board.active_seat(), 2);
}

#[test]
fn play_view_keeps_dealt_order() {
    let mut board = rules_board();
    let dealt = round_robin_deal();
    let mut expected = dealt[0].clone();
    // Seat 0 leads, then three more cards, so seat 0 plays its 3rd card on the third trick.
    while board.hand(0).len() > 5 {
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            let seat = board.active_seat();
            let card = board.legal_cards(seat)[0];
            if seat == 0 {
                expected.retain(|c| *c != card);
            }
            board.play(seat, card, false).unwrap();
        }
    }
    assert_eq!(board.play_view(0).hand, expected);
    assert_eq!(board.play_view(0).hand.len(), 5);
}

#[test]
fn a_completed_trick_moves_from_the_current_trick_to_the_tricks() {
    let mut board = rules_board();
    let mut played = Vec::new();
    for _ in 0..4 {
        let seat = board.active_seat();
        let card = board.legal_cards(seat)[0];
        board.play(seat, card, false).unwrap();
        played.push((seat, card));
    }
    assert!(board.reviewing_completed_trick());
    for seat in 0..4 {
        let view = board.play_view(seat);
        assert!(view.current_trick.is_empty());
        let last = view.tricks.last().expect("the completed trick");
        let seats_and_cards: Vec<_> = last.iter().map(|c| (c.seat, c.card)).collect();
        assert_eq!(seats_and_cards, played);
    }
}

// ---- belote ----

#[test]
fn declares_belote_then_awards_rebelote_bonus_once() {
    // Seat 0 holds every trump, so it wins the first trick and leads the queen next.
    let hands = [
        cards("KH QH 7H 8H 9H 10H JH AH"),
        cards("7C 8C 9C 10C JC QC KC AC"),
        cards("7D 8D 9D 10D JD QD KD AD"),
        cards("7S 8S 9S 10S JS QS KS AS"),
    ];
    let mut board = GameBoard::from_bidding(hands, Suit::Hearts, 0, 3, false);
    assert_eq!(
        board.belotes(),
        [Belote {
            seat: 0,
            suit: Suit::Hearts
        }]
    );

    board.play(0, card("KH"), false).unwrap();
    assert_eq!(
        board.last_belote(),
        Some(BeloteDeclaration {
            seat: 0,
            rebelote: false
        })
    );
    assert_eq!(board.belote_bonus(), 0);
    finish_first_trick(&mut board, false);
    board.continue_after_trick().unwrap();
    assert_eq!(board.active_seat(), 0);
    let before = board.scores();
    board.play(0, card("QH"), false).unwrap();

    assert_eq!(
        board.last_belote(),
        Some(BeloteDeclaration {
            seat: 0,
            rebelote: true
        })
    );
    assert_eq!(board.belote_bonus(), 20);
    assert_eq!(board.scores().0, before.0 + 20);
    // The bonus is awarded once: playing on does not add it again.
    let result = play_out(&mut board);
    assert_eq!(result.north_south_belote_bonus, 20);
}

// ---- capot ----

const DEALER: usize = 3;

/// North leads and holds six trumps and two aces; East holds the king and queen of trump.
fn north_south_capot_deal() -> [Vec<Card>; 4] {
    [
        cards("JH 9H AH 10H 8H 7H AS AC"),
        cards("KH QH KS QS JS KC QC JC"),
        cards("10S 9S 10C 9C AD 10D KD QD"),
        cards("8S 7S 8C 7C JD 9D 8D 7D"),
    ]
}

/// Each team holds its own aces, so both teams take tricks.
fn shared_tricks_deal() -> [Vec<Card>; 4] {
    [
        cards("JH 9H 7S 8S 9S 7C 8C 9C"),
        cards("AS 10S KS AC 10C KC 7H 8H"),
        cards("AH 10H QS JS QC JC 7D 8D"),
        cards("KH QH AD 10D KD QD JD 9D"),
    ]
}

fn bidding_result(hands: [Vec<Card>; 4], declarer: usize) -> RoundResult {
    play_out(&mut GameBoard::from_bidding(
        hands,
        Suit::Hearts,
        declarer,
        DEALER,
        false,
    ))
}

fn contract_result(declarer: usize, value: i32, coinched: bool) -> RoundResult {
    play_out(&mut GameBoard::from_contract(
        north_south_capot_deal(),
        Trump::Hearts,
        declarer,
        value,
        coinched,
        DEALER,
        false,
    ))
}

#[test]
fn declaring_team_taking_every_trick_scores_the_capot() {
    let result = bidding_result(north_south_capot_deal(), 0);

    assert_eq!(result.capot_team, Some(Team::NorthSouth));
    assert!(result.contract_made);
    assert_eq!(result.north_south_awarded, 250);
    assert_eq!(result.east_west_belote_bonus, 20);
    assert_eq!(result.east_west_awarded, 20);
}

#[test]
fn defenders_taking_every_trick_score_the_capot() {
    let result = bidding_result(north_south_capot_deal(), 1);

    assert_eq!(result.capot_team, Some(Team::NorthSouth));
    assert!(!result.contract_made);
    assert_eq!(result.north_south_awarded, 250);
    assert_eq!(result.east_west_awarded, 20);
}

#[test]
fn contree_capot_also_scores_the_bid() {
    let result = contract_result(0, 100, false);

    assert_eq!(result.north_south_awarded, 350);
    assert_eq!(result.east_west_awarded, 20);
}

#[test]
fn coinche_doubles_the_capot_score() {
    let result = contract_result(0, 100, true);

    assert_eq!(result.north_south_awarded, 700);
    assert_eq!(result.east_west_awarded, 40);
}

#[test]
fn contree_defenders_capot_does_not_score_the_declarers_bid() {
    let result = contract_result(1, 100, false);

    assert!(!result.contract_made);
    assert_eq!(result.north_south_awarded, 250);
}

#[test]
fn a_round_without_capot_has_no_capot_team() {
    let result = bidding_result(shared_tricks_deal(), 0);

    assert_eq!(result.capot_team, None);
    assert_eq!(
        162,
        result.north_south_card_points
            + result.east_west_card_points
            + result.north_south_dix_de_der
            + result.east_west_dix_de_der
    );
}

// ---- last trick ----

fn sequential_deal() -> [Vec<Card>; 4] {
    let deck = Card::deck();
    [0, 1, 2, 3].map(|i| deck[i * 8..i * 8 + 8].to_vec())
}

#[test]
fn plays_the_last_trick_automatically_once_each_player_has_one_card_left() {
    let mut board = GameBoard::from_bidding(sequential_deal(), Suit::Hearts, 0, 3, false);
    while board.completed_tricks() < 7 {
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            let seat = board.active_seat();
            let card = board.legal_cards(seat)[0];
            board.play(seat, card, false).unwrap();
        }
    }
    assert!(board.round_result().is_none());

    board.continue_after_trick().unwrap();

    assert_eq!(board.completed_tricks(), 8);
    assert!(board.reviewing_completed_trick());
    assert_eq!(board.visible_trick().len(), 4);
    assert!((0..4).all(|seat| board.hand(seat).is_empty()));
    let result = board.round_result().unwrap();
    assert_eq!(
        162,
        result.north_south_card_points
            + result.east_west_card_points
            + result.north_south_dix_de_der
            + result.east_west_dix_de_der
    );
}

#[test]
fn last_trick_plays_itself() {
    let mut board = GameBoard::from_bidding(sequential_deal(), Suit::Hearts, 0, 3, false);
    while board.completed_tricks() < 7 {
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            let seat = board.active_seat();
            let card = board.legal_cards(seat)[0];
            board.play(seat, card, false).unwrap();
        }
    }
    board.continue_after_trick().unwrap();
    assert!(board.round_result().is_some());
    assert!(board.round_summary().is_some());
    assert!((0..4).all(|seat| board.hand(seat).is_empty()));
}

// ---- announcements (hearts are trump, seat 0 leads) ----

/// North leads; North holds a tierce in clubs and East a cinquante in spades; nobody holds Belote.
fn announcement_board(announcements: bool) -> GameBoard {
    let hands = [
        cards("8C 9C 10C 7D 8D 7S 8S 7H"),
        cards("JS QS KS AS 9D 7C 9H AH"),
        cards("JC KC 10D QD AD 8H 10H KH"),
        cards("QC AC JD KD 9S JH QH 10S"),
    ];
    GameBoard::from_bidding(hands, Suit::Hearts, 0, 3, announcements)
}

/// Plays the rest of the first trick, every other seat declaring or not.
fn finish_first_trick(board: &mut GameBoard, declare: bool) {
    while !board.reviewing_completed_trick() {
        let seat = board.active_seat();
        let card = board.legal_cards(seat)[0];
        board.play(seat, card, declare).unwrap();
    }
}

#[test]
fn declaring_on_the_first_card_scores_after_the_first_trick() {
    let mut board = announcement_board(true);

    board.play(0, card("8C"), true).unwrap();
    finish_first_trick(&mut board, false);

    let trick_points: i32 = board
        .visible_trick()
        .iter()
        .map(|played| beelot_game::rules::points(played.card, Trump::Hearts))
        .sum();
    let (north_south, east_west) = board.scores();
    assert_eq!(north_south + east_west, trick_points + 20);
    assert_eq!(board.revealed().len(), 1);
    let declaration = &board.revealed()[0];
    assert_eq!(declaration.seat, 0);
    assert_eq!(declaration.announcements[0].kind, AnnouncementKind::Tierce);
    assert!(declaration.scored);
    assert_eq!(play_out(&mut board).north_south_announcements, 20);
}

#[test]
fn not_declaring_loses_the_points() {
    let mut board = announcement_board(true);

    board.play(0, card("8C"), false).unwrap();
    finish_first_trick(&mut board, false);

    assert!(board.revealed().is_empty());
    assert_eq!(play_out(&mut board).north_south_announcements, 0);
}

#[test]
fn declare_ignored_when_off() {
    let mut board = announcement_board(false);

    board.play(0, card("8C"), true).unwrap();
    finish_first_trick(&mut board, true);

    assert!(!board.play_view(0).announcements);
    assert!(board.revealed().is_empty());
    assert_eq!(play_out(&mut board).north_south_announcements, 0);
}

#[test]
fn declare_ignored_after_the_first_card() {
    let mut board = announcement_board(true);

    board.play(0, card("8C"), false).unwrap();
    finish_first_trick(&mut board, false);
    board.continue_after_trick().unwrap();
    let leader = board.active_seat();
    let lead = board.legal_cards(leader)[0];
    board.play(leader, lead, true).unwrap();

    assert!(board.declared().is_empty());
    let result = play_out(&mut board);
    assert_eq!(
        result.north_south_announcements + result.east_west_announcements,
        0
    );
}

#[test]
fn only_the_best_team_scores() {
    let mut board = announcement_board(true);

    board.play(0, card("8C"), true).unwrap();
    board.play(1, card("7C"), true).unwrap();
    finish_first_trick(&mut board, false);

    let revealed = board.revealed().to_vec();
    assert_eq!(revealed.len(), 2);
    assert!(!revealed[0].scored);
    assert!(revealed[1].scored);
    let result = play_out(&mut board);
    assert_eq!(result.north_south_announcements, 0);
    assert_eq!(result.east_west_announcements, 50);
    assert_eq!(revealed, result.announcements);
}

#[test]
fn opponents_see_only_who_declared_during_the_first_trick() {
    let mut board = announcement_board(true);

    board.play(0, card("8C"), true).unwrap();

    let seats: Vec<usize> = board.declared().iter().map(|d| d.seat).collect();
    assert_eq!(seats, [0]);
    assert!(board.revealed().is_empty());
    assert!(board.play_view(1).revealed_announcements.is_empty());
    let own = announcements::find(board.hand(1), board.trump());
    assert_eq!(own[0].kind, AnnouncementKind::Cinquante);
}

#[test]
fn nobody_declares_no_reveal() {
    let mut board = announcement_board(true);

    board.play(0, card("8C"), false).unwrap();
    finish_first_trick(&mut board, false);

    assert!(board.revealed().is_empty());
    assert!(board.declared().is_empty());
    assert!(play_out(&mut board).announcements.is_empty());
}

#[test]
fn bots_see_the_reveal_only_after_the_first_trick() {
    let mut board = announcement_board(true);
    board.play(0, card("8C"), true).unwrap();

    assert!(board.play_view(1).announcements);
    assert!(board.play_view(1).revealed_announcements.is_empty());
    finish_first_trick(&mut board, false);
    board.continue_after_trick().unwrap();

    let next = board.active_seat();
    assert_eq!(
        board.play_view(next).revealed_announcements,
        board.revealed()
    );
    assert_eq!(board.play_view(next).revealed_announcements.len(), 1);
}

#[test]
fn bots_see_announcements_off() {
    assert!(!announcement_board(false).play_view(0).announcements);
}

// ---- all trumps and no trumps (US-063) ----

/// A deal where the given seats hold the given cards, and the rest of the deck is shuffled out
/// with no other Belote.
fn deal_with(fixed: &[(usize, &str)], seed: i64) -> [Vec<Card>; 4] {
    let belotes = |hand: &[Card]| {
        Suit::ALL
            .into_iter()
            .filter(|&suit| {
                hand.contains(&Card::new(beelot_game::Rank::King, suit))
                    && hand.contains(&Card::new(beelot_game::Rank::Queen, suit))
            })
            .count()
    };
    for seed in seed.. {
        let mut hands: [Vec<Card>; 4] = Default::default();
        for &(seat, codes) in fixed {
            hands[seat] = cards(codes);
        }
        let mut rest: Vec<Card> = Card::deck()
            .into_iter()
            .filter(|card| !hands.iter().flatten().any(|held| held == card))
            .collect();
        beelot_game::java_shuffle(&mut rest, &mut beelot_game::JavaRandom::new(seed));
        let asked: Vec<usize> = hands.iter().map(|hand| belotes(hand)).collect();
        for hand in &mut hands {
            while hand.len() < 8 {
                hand.push(rest.pop().unwrap());
            }
        }
        if hands
            .iter()
            .zip(&asked)
            .all(|(hand, &asked)| belotes(hand) == asked)
        {
            return hands;
        }
    }
    unreachable!()
}

fn contract_board(trump: Trump, hands: [Vec<Card>; 4]) -> GameBoard {
    GameBoard::from_contract(hands, trump, 0, 80, false, 3, false)
}

#[test]
fn two_belotes_in_all_trumps_both_score() {
    let hands = deal_with(&[(0, "KH QH"), (1, "KS QS")], 0);
    let result = play_out(&mut contract_board(Trump::AllTrumps, hands));

    assert_eq!(result.north_south_belote_bonus, 20);
    assert_eq!(result.east_west_belote_bonus, 20);
}

#[test]
fn one_player_with_two_belotes_scores_forty() {
    let hands = deal_with(&[(0, "KH QH KC QC")], 0);
    let result = play_out(&mut contract_board(Trump::AllTrumps, hands));

    assert_eq!(result.north_south_belote_bonus, 40);
    assert_eq!(result.east_west_belote_bonus, 0);
}

/// The Belote bonus shown after each Rebelote, until the round ends.
fn bonuses_after_each_rebelote(board: &mut GameBoard) -> Vec<(usize, i32)> {
    let mut bonuses = Vec::new();
    while board.round_result().is_none() {
        let before = (board.last_belote(), board.belote_bonus());
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            let seat = board.active_seat();
            let card = board.legal_cards(seat)[0];
            board.play(seat, card, false).unwrap();
        }
        // A Rebelote always changes the bonus or the seat it counts for.
        if let Some(declaration) = board.last_belote()
            && declaration.rebelote
            && (board.last_belote(), board.belote_bonus()) != before
        {
            bonuses.push((declaration.seat, board.belote_bonus()));
        }
    }
    bonuses
}

#[test]
fn the_belote_bonus_counts_the_team_of_the_latest_rebelote() {
    let hands = deal_with(&[(0, "KH QH"), (1, "KS QS")], 0);
    let bonuses = bonuses_after_each_rebelote(&mut contract_board(Trump::AllTrumps, hands));

    assert_eq!(bonuses.len(), 2, "{bonuses:?}");
    assert!(bonuses.iter().all(|&(_, bonus)| bonus == 20), "{bonuses:?}");
}

#[test]
fn a_player_with_two_belotes_sees_forty_after_the_second_rebelote() {
    let hands = deal_with(&[(0, "KH QH KC QC")], 0);
    let bonuses = bonuses_after_each_rebelote(&mut contract_board(Trump::AllTrumps, hands));

    assert_eq!(bonuses, [(0, 20), (0, 40)]);
}

#[test]
fn no_belote_in_no_trumps() {
    let hands = deal_with(&[(0, "KH QH")], 0);
    let mut board = contract_board(Trump::NoTrumps, hands);
    assert!(board.belotes().is_empty());

    let result = play_out(&mut board);

    assert_eq!(board.last_belote(), None);
    assert_eq!(result.north_south_belote_bonus, 0);
    assert_eq!(result.east_west_belote_bonus, 0);
}

#[test]
fn a_full_deal_of_all_trumps_or_no_trumps_totals_162() {
    for trump in [Trump::AllTrumps, Trump::NoTrumps] {
        for seed in 0..5 {
            let result = play_out(&mut contract_board(trump, deal_with(&[], seed)));
            let total = result.north_south_card_points
                + result.east_west_card_points
                + result.north_south_dix_de_der
                + result.east_west_dix_de_der;
            assert_eq!(total, 162, "{trump:?} seed {seed}");
        }
    }
}

#[test]
fn the_round_summary_and_play_view_carry_the_contract() {
    let mut board = contract_board(Trump::NoTrumps, deal_with(&[], 1));
    assert_eq!(board.play_view(0).trump, Trump::NoTrumps);
    play_out(&mut board);
    assert_eq!(board.round_summary().unwrap().trump, Trump::NoTrumps);
}
