//! Tests of the rule-based strategy's card play, plus a check of the bot turns with the
//! `RecordingStrategy` helper. Hearts are trump; Ana (seat 0) leads the first trick, Ben is seat
//! 1, Chloe seat 2 and David seat 3.

mod common;

use beelot_game::bidding::BiddingState;
use beelot_game::board::GameBoard;
use beelot_game::bot::rule_based::RuleBasedStrategy;
use beelot_game::bot::turns::{play_turn, take_auction_turn};
use beelot_game::card::{Card, Suit, Variant};
use beelot_game::rng::SplitMix64;
use common::RecordingStrategy;

fn card(code: &str) -> Card {
    code.parse().unwrap()
}

/// Pads the given cards to eight with low clubs and diamonds, none of them spades, trumps or aces.
fn hand(codes: &str) -> Vec<Card> {
    let mut hand: Vec<Card> = codes.split_whitespace().map(card).collect();
    for rank in ["7", "8", "9", "J", "Q", "K", "10"] {
        for suit in ["C", "D"] {
            let filler = card(&format!("{rank}{suit}"));
            if hand.len() < 8 && !hand.contains(&filler) {
                hand.push(filler);
            }
        }
    }
    hand
}

/// Hearts are trump; Ana leads the first trick.
fn board(declarer: usize, ana: &str, ben: &str, chloe: &str, david: &str) -> GameBoard {
    GameBoard::from_bidding(
        [hand(ana), hand(ben), hand(chloe), hand(david)],
        Suit::Hearts,
        declarer,
        3,
        false,
    )
}

fn play(board: &mut GameBoard, seat: usize, code: &str) {
    board.play(seat, card(code), false).unwrap();
}

fn trick(board: &GameBoard) -> Vec<Card> {
    board
        .visible_trick()
        .iter()
        .map(|played| played.card)
        .collect()
}

fn cards(codes: &str) -> Vec<Card> {
    codes.split_whitespace().map(card).collect()
}

fn bot_plays(board: &mut GameBoard) {
    play_turn(board, &RuleBasedStrategy::new()).unwrap();
}

/// Ana leads the ace of spades, Ben follows, and Chloe's bot, void in spades, must answer with the
/// given hand.
fn partner_wins_with_the_ace(chloe: &str) -> GameBoard {
    let mut board = board(1, "AS", "7S", chloe, "");
    play(&mut board, 0, "AS");
    play(&mut board, 1, "7S");
    board
}

/// Ana leads a spade, Ben wins it with the ace, and Chloe's bot must answer with the given hand.
fn won_by_opponent(chloe: &str) -> GameBoard {
    let mut board = board(0, "7S", "AS", chloe, "8S");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "AS");
    board
}

/// Ana leads a spade, Ben trumps it, and Chloe's bot must answer with the given hand.
fn trumped_by_opponent(chloe: &str) -> GameBoard {
    let mut board = board(0, "7S", "7H", chloe, "9S");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "7H");
    board
}

#[test]
fn follows_suit_without_an_ace_when_the_opponents_have_trumped() {
    let mut board = trumped_by_opponent("AS 8S");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7S 7H 8S"));
}

#[test]
fn discards_another_card_than_an_ace_when_the_opponents_have_trumped() {
    let mut board = trumped_by_opponent("AC 7D");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7S 7H 7D"));
}

#[test]
fn plays_the_ace_when_it_is_the_only_legal_card() {
    let mut board = trumped_by_opponent("AS");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7S 7H AS"));
}

#[test]
fn leads_a_low_card_rather_than_a_trump() {
    let mut board = board(0, "7H 8S", "", "", "");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7C"));
}

#[test]
fn leads_its_master_card() {
    let mut board = board(0, "7D AS", "", "", "");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("AS"));
}

#[test]
fn avoids_leading_a_suit_with_the_ten_without_the_ace() {
    let mut board = board(0, "10S 7S", "", "", "");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7C"));
}

#[test]
fn defender_does_not_trump_its_partners_trick_on_the_first_trick() {
    let mut board = board(1, "AS", "7S", "7H 7D", "");
    play(&mut board, 0, "AS");
    play(&mut board, 1, "7S");
    bot_plays(&mut board);
    assert_ne!(trick(&board).last().unwrap().suit, Suit::Hearts);
}

#[test]
fn adds_its_ten_without_the_ace_to_a_trick_its_partner_is_sure_to_win() {
    let mut board = partner_wins_with_the_ace("7H 10C");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("AS 7S 10C"));
}

#[test]
fn keeps_its_master_cards_when_adding_points_to_its_partners_trick() {
    let mut board = partner_wins_with_the_ace("AC KD");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("AS 7S KD"));
}

#[test]
fn plays_low_when_its_partner_may_lose_the_trick() {
    let mut board = board(0, "KS", "7S", "10S 8S", "");
    play(&mut board, 0, "KS");
    play(&mut board, 1, "7S");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("KS 7S 8S"));
}

#[test]
fn overtrumps_with_its_cheapest_winning_trump_when_playing_last() {
    let mut board = board(0, "7S", "8S", "7H", "JH 8H");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "8S");
    play(&mut board, 2, "7H");
    bot_plays(&mut board);
    assert_eq!(*trick(&board).last().unwrap(), card("8H"));
}

#[test]
fn defender_trumps_on_the_first_trick_when_it_has_no_other_legal_card() {
    let mut board = board(1, "7S", "8S", "7H", "");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "8S");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7S 8S 7H"));
}

#[test]
fn follows_suit_with_its_lowest_value_card_on_a_trick_the_opponents_win() {
    let mut board = won_by_opponent("10S 9S");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7S AS 9S"));
}

#[test]
fn discards_its_lowest_value_card_on_a_trick_the_opponents_win() {
    let mut board = won_by_opponent("AC 10D");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7S AS 7C"));
}

#[test]
fn wins_with_a_card_sure_to_hold_when_it_can_win_the_trick() {
    let mut board = board(0, "7S", "10S", "AS 9S", "8S");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "10S");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7S 10S AS"));
}

#[test]
fn leads_an_ace_whose_suit_is_nearly_exhausted() {
    let mut board = board(0, "7S 8S 9S 10S JS AS", "", "", "");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("AS"));
}

/// Ben discarded a diamond instead of trumping Ana's trick, so Ben has no trumps left to beat the
/// ace.
#[test]
fn leads_an_ace_an_opponent_void_in_the_suit_cannot_trump() {
    let mut board = board(0, "7S", "7D", "10S KS AS", "8S");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "7D");
    play(&mut board, 2, "10S");
    play(&mut board, 3, "8S");
    board.continue_after_trick().unwrap();
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("AS"));
}

#[test]
fn wins_with_its_cheapest_card_sure_to_win_and_keeps_its_ace() {
    let mut board = board(0, "7S", "8S", "9S 10S JS AS", "");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "8S");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7S 8S 10S"));
}

#[test]
fn declarer_leads_its_master_trump_while_the_opponents_may_hold_trumps() {
    let mut board = board(0, "JH AS", "", "", "");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("JH"));
}

#[test]
fn declarer_does_not_lead_a_trump_that_is_not_master() {
    let mut board = board(0, "9H AS", "", "", "");
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("AS"));
}

/// Ben and David discarded instead of trumping their opponents' trick, so neither has trumps
/// left.
#[test]
fn declarer_stops_leading_trumps_once_the_opponents_have_none() {
    let mut board = board(0, "7S", "7D", "10S JH AS", "");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "7D");
    play(&mut board, 2, "10S");
    play(&mut board, 3, "7C");
    board.continue_after_trick().unwrap();
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("AS"));
}

#[test]
fn keeps_its_jack_of_trumps_out_of_a_low_value_trick_its_partner_may_win() {
    let mut board = board(0, "KS", "7S", "JH 7H", "");
    play(&mut board, 0, "KS");
    play(&mut board, 1, "7S");
    bot_plays(&mut board);
    assert_ne!(trick(&board).last().unwrap().suit, Suit::Hearts);
}

#[test]
fn trumps_a_low_value_trick_with_a_small_trump_rather_than_the_jack() {
    let mut board = board(0, "7S", "8S", "JH 7H", "");
    play(&mut board, 0, "7S");
    play(&mut board, 1, "8S");
    bot_plays(&mut board);
    assert_eq!(*trick(&board).last().unwrap(), card("7H"));
}

#[test]
fn wins_a_valuable_trick_with_its_jack_of_trumps() {
    let mut board = board(0, "10S", "AS", "JH 7H", "");
    play(&mut board, 0, "10S");
    play(&mut board, 1, "AS");
    bot_plays(&mut board);
    assert_eq!(*trick(&board).last().unwrap(), card("JH"));
}

#[test]
fn without_trump_control_the_declarer_cashes_its_master_card_first() {
    let mut board = board(0, "JH AS", "", "", "");
    play_turn(&mut board, &RuleBasedStrategy::without_trump_control()).unwrap();
    assert_eq!(trick(&board), cards("AS"));
}

/// The bot turns ask the strategy once per call or card, until the round
/// ends.
#[test]
fn bot_turns_ask_the_strategy_once_per_decision() {
    let strategy = RecordingStrategy::new();
    let mut bidding = BiddingState::new(Variant::Contree, 3, Box::new(SplitMix64::new(7)));
    let mut auction_turns = 0;
    while bidding.completed_board().is_none() && bidding.deals() == 1 {
        take_auction_turn(&mut bidding, &strategy).unwrap();
        auction_turns += 1;
    }
    assert_eq!(strategy.decisions(), auction_turns);
    let mut board = bidding.into_board().expect("a seat bids with this seed");
    while board.round_result().is_none() {
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            play_turn(&mut board, &strategy).unwrap();
        }
    }
    assert_eq!(
        strategy.card_decisions(),
        28,
        "the last trick is played for the seats"
    );
    assert_eq!(strategy.decisions(), auction_turns + 28);
    assert!(play_turn(&mut board, &strategy).is_err());
}

/// Bots play tout-atout and sans-atout when a human takes one (US-063): every card they choose is
/// legal, as the defence and as the human's partner, and the round totals 162 points.
#[test]
fn bots_play_all_trumps_and_no_trumps_legally() {
    use beelot_game::bot::strategies;
    use beelot_game::card::{Difficulty, Trump};
    use beelot_game::rng::shuffle;
    use beelot_game::rules;

    for trump in [Trump::AllTrumps, Trump::NoTrumps] {
        for difficulty in [Difficulty::Relaxed, Difficulty::Challenging] {
            let strategy = strategies::for_difficulty(difficulty);
            for human_at_north in [false, true] {
                for seed in 1..=3 {
                    let mut deck = Card::deck();
                    shuffle(&mut deck, &mut SplitMix64::new(seed));
                    let hands = std::array::from_fn(|seat| deck[seat * 8..seat * 8 + 8].to_vec());
                    let mut board = GameBoard::from_contract(hands, trump, 0, 80, false, 3, false);
                    while board.round_result().is_none() {
                        if board.reviewing_completed_trick() {
                            board.continue_after_trick().unwrap();
                            continue;
                        }
                        let seat = board.active_seat();
                        let view = board.play_view(seat);
                        assert_eq!(view.trump, trump);
                        let card = if human_at_north && seat == 0 {
                            let trick: Vec<Card> = view
                                .current_trick
                                .iter()
                                .map(|played| played.card)
                                .collect();
                            rules::legal_cards(&view.hand, &trick, trump)[0]
                        } else {
                            strategy.choose_card(&view)
                        };
                        assert!(
                            view.legal_cards.contains(&card),
                            "{trump:?} {difficulty:?} chose {card}"
                        );
                        board.play(seat, card, false).unwrap();
                    }
                    let result = board.round_result().unwrap();
                    let total = result.north_south_card_points
                        + result.east_west_card_points
                        + result.north_south_dix_de_der
                        + result.east_west_dix_de_der;
                    assert_eq!(total, 162, "{trump:?} {difficulty:?} seed {seed}");
                }
            }
        }
    }
}

/// An all trumps contract declared by Ben, with Ana's full hand and the given cards for the
/// others, who receive the rest of the deck in turn. The player after the dealer leads.
fn all_trumps_board(dealer: usize, ana: &str, ben: &str, chloe: &str, david: &str) -> GameBoard {
    use beelot_game::card::{Rank, Trump};

    let mut hands = [cards(ana), cards(ben), cards(chloe), cards(david)];
    let mut seat = 1;
    for suit in Suit::ALL {
        for rank in Rank::ALL {
            let card = Card::new(rank, suit);
            if hands.iter().any(|hand| hand.contains(&card)) {
                continue;
            }
            while hands[seat].len() == 8 {
                seat = seat % 3 + 1;
            }
            hands[seat].push(card);
            seat = seat % 3 + 1;
        }
    }
    GameBoard::from_contract(hands, Trump::AllTrumps, 1, 80, false, dealer, false)
}

/// All trumps, Ben declares and Ana leads the first trick holding the jack of spades and low cards.
fn master_lead_in_defence() -> GameBoard {
    all_trumps_board(3, "JS 7H 8H 7D 8D 7C 8C QH", "", "", "")
}

/// All trumps: Ben leads the seven of clubs, Chloe wins with the jack, David follows, and Ana is
/// void in clubs.
fn partner_wins_with_the_jack_of_clubs() -> GameBoard {
    let mut board = all_trumps_board(0, "9D 7D 8D 7H 8H 7S 8S QS", "7C", "JC", "8C");
    play(&mut board, 1, "7C");
    play(&mut board, 2, "JC");
    play(&mut board, 3, "8C");
    board
}

/// All trumps: David leads the seven of hearts, and Ana holds the jack and eight of hearts.
fn seven_of_hearts_led() -> GameBoard {
    let mut board = all_trumps_board(2, "JH 8H 7S 8S 7D 8D 7C 8C", "", "", "7H");
    play(&mut board, 3, "7H");
    board
}

#[test]
fn cashes_a_master_card_in_defence_under_all_trumps() {
    let mut board = master_lead_in_defence();
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("JS"));
}

#[test]
fn adds_points_to_its_partners_sure_trick_under_all_trumps() {
    let mut board = partner_wins_with_the_jack_of_clubs();
    bot_plays(&mut board);
    assert_eq!(*trick(&board).last().unwrap(), card("9D"));
}

#[test]
fn wins_a_cheap_trick_with_a_jack_under_all_trumps() {
    let mut board = seven_of_hearts_led();
    bot_plays(&mut board);
    assert_eq!(trick(&board), cards("7H JH"));
}

#[test]
fn without_all_trumps_tactics_the_bot_plays_as_in_us_063() {
    use beelot_game::card::Trump;
    use beelot_game::rules;

    let previous = RuleBasedStrategy::without_all_trumps_tactics();

    let mut lead = master_lead_in_defence();
    play_turn(&mut lead, &previous).unwrap();
    assert_ne!(trick(&lead)[0], card("JS"));

    let mut partner = partner_wins_with_the_jack_of_clubs();
    play_turn(&mut partner, &previous).unwrap();
    assert_eq!(
        rules::points(*trick(&partner).last().unwrap(), Trump::AllTrumps),
        0
    );

    let mut cheap = seven_of_hearts_led();
    play_turn(&mut cheap, &previous).unwrap();
    assert_eq!(trick(&cheap), cards("7H 8H"));
}
