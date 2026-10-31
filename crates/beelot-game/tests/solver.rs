//! Port of the Java `DoubleDummySolverTest`. Seats are numbered in play order from 0; North–South
//! hold the even seats.

use std::collections::HashMap;

use beelot_game::board::{GameBoard, SeatCard};
use beelot_game::bot::solver::{DoubleDummySolver, Position, Solution};
use beelot_game::card::{Card, Seat, Suit, Trump};
use beelot_game::rng::{RandomSource, SplitMix64, shuffle};
use beelot_game::rules;

fn card(code: &str) -> Card {
    code.parse().unwrap()
}

fn cards(codes: &str) -> Vec<Card> {
    codes.split_whitespace().map(card).collect()
}

fn seat_card(seat: Seat, code: &str) -> SeatCard {
    SeatCard {
        seat,
        card: card(code),
    }
}

fn trick_cards(trick: &[SeatCard]) -> Vec<Card> {
    trick.iter().map(|played| played.card).collect()
}

#[test]
fn the_last_trick_scores_its_cards_and_the_dix_de_der() {
    let position = Position::start(
        [cards("AS"), cards("7S"), cards("8S"), cards("10C")],
        Trump::Hearts,
        0,
    );

    let solution = DoubleDummySolver::new().solve(&position);

    // North's ace wins the ace, West's discarded ten and the dix de der.
    assert_eq!(
        solution,
        Solution {
            north_south_points: 11 + 10 + 10,
            east_west_points: 0,
            best_card: Some(card("AS")),
        }
    );
}

#[test]
fn cashes_the_ace_before_the_king() {
    // Hearts are trump and North leads. Leading the king lets East's ten win it, and East then
    // trumps the ace: East–West would score all 38 points. Leading the ace forces out the ten, so
    // North–South win 21.
    let position = Position::start(
        [
            cards("AS KS"),
            cards("10S 7H"),
            cards("7S 8S"),
            cards("9S QS"),
        ],
        Trump::Hearts,
        0,
    );

    let solution = DoubleDummySolver::new().solve(&position);

    assert_eq!(
        solution,
        Solution {
            north_south_points: 21,
            east_west_points: 17,
            best_card: Some(card("AS")),
        }
    );
}

#[test]
fn counts_the_belote_of_the_player_dealt_the_king_and_queen_of_trumps() {
    // North played the king of trumps in an earlier trick and still holds the queen.
    let tricks = vec![vec![
        seat_card(0, "KH"),
        seat_card(1, "7H"),
        seat_card(2, "8H"),
        seat_card(3, "9C"),
    ]];
    let position = Position {
        hands: [cards("QH"), cards("7C"), cards("8C"), cards("10C")],
        trump: Trump::Hearts,
        tricks,
        current_trick: Vec::new(),
        first_leader: 0,
    };

    let solution = DoubleDummySolver::new().solve(&position);

    // North's king won the first trick (4); the queen wins the last with West's ten and the dix
    // de der, and the belote adds 20.
    assert_eq!(
        solution,
        Solution {
            north_south_points: 4 + (3 + 10 + 10) + 20,
            east_west_points: 0,
            best_card: Some(card("QH")),
        }
    );
}

#[test]
fn solves_from_the_middle_of_a_trick() {
    // North led the ace of spades; East, out of spades and with the partner losing, must trump.
    let position = Position {
        hands: [Vec::new(), cards("7H"), cards("8S"), cards("9S")],
        trump: Trump::Hearts,
        tricks: Vec::new(),
        current_trick: vec![seat_card(0, "AS")],
        first_leader: 0,
    };

    let solution = DoubleDummySolver::new().solve(&position);

    assert_eq!(
        solution,
        Solution {
            north_south_points: 0,
            east_west_points: 11 + 10,
            best_card: Some(card("7H")),
        }
    );
}

#[test]
fn reports_the_score_of_a_finished_round() {
    let position = Position {
        hands: Default::default(),
        trump: Trump::Hearts,
        tricks: Vec::new(),
        current_trick: Vec::new(),
        first_leader: 0,
    };

    let solution = DoubleDummySolver::new().solve(&position);

    assert_eq!(solution.north_south_points + solution.east_west_points, 0);
    assert_eq!(solution.best_card, None);
}

/// The plain minimax of the Java test, remembering the value of each trick start.
struct Exhaustive {
    memo: HashMap<(Trump, Seat, [Vec<Card>; 4]), i32>,
}

impl Exhaustive {
    fn new() -> Self {
        Self {
            memo: HashMap::new(),
        }
    }

    /// North–South points from the remaining cards.
    fn value(
        &mut self,
        hands: &[Vec<Card>; 4],
        trick: &[SeatCard],
        leader: Seat,
        trump: Trump,
    ) -> i32 {
        if trick.len() == 4 {
            let played = trick_cards(trick);
            let winner = trick[rules::winning_index(&played, trump)].seat;
            let last = hands.iter().all(Vec::is_empty);
            let trick_points: i32 = played
                .iter()
                .map(|&card| rules::points(card, trump))
                .sum::<i32>()
                + if last { 10 } else { 0 };
            let gain = if winner.is_multiple_of(2) {
                trick_points
            } else {
                0
            };
            if last {
                return gain;
            }
            let key = (trump, winner, hands.clone());
            let next = match self.memo.get(&key) {
                Some(&next) => next,
                None => {
                    let next = self.value(hands, &[], winner, trump);
                    self.memo.insert(key, next);
                    next
                }
            };
            return gain + next;
        }
        let seat = (leader + trick.len()) % 4;
        let north_south = seat.is_multiple_of(2);
        let mut best = if north_south { i32::MIN } else { i32::MAX };
        for card in rules::legal_cards(&hands[seat], &trick_cards(trick), trump) {
            let value = self.value_after(hands, trick, leader, trump, card);
            best = if north_south {
                best.max(value)
            } else {
                best.min(value)
            };
        }
        best
    }

    fn value_after(
        &mut self,
        hands: &[Vec<Card>; 4],
        trick: &[SeatCard],
        leader: Seat,
        trump: Trump,
        card: Card,
    ) -> i32 {
        let seat = (leader + trick.len()) % 4;
        let mut next = hands.clone();
        remove(&mut next[seat], card);
        let mut next_trick = trick.to_vec();
        next_trick.push(SeatCard { seat, card });
        let leader = if trick.is_empty() { seat } else { leader };
        self.value(&next, &next_trick, leader, trump)
    }
}

fn remove(hand: &mut Vec<Card>, card: Card) {
    let index = hand.iter().position(|&held| held == card).unwrap();
    hand.remove(index);
}

/// 20 for each king and queen of a trump suit held or played by one player of the team (0
/// North–South, 1 East–West).
fn belote_for(team: usize, hands: &[Vec<Card>; 4], trick: &[SeatCard], trump: Trump) -> i32 {
    let mut points = 0;
    for seat in (team..4).step_by(2) {
        let mut dealt = hands[seat].clone();
        dealt.extend(
            trick
                .iter()
                .filter(|played| played.seat == seat)
                .map(|played| played.card),
        );
        for suit in Suit::ALL {
            let king = Card::new(beelot_game::Rank::King, suit);
            let queen = Card::new(beelot_game::Rank::Queen, suit);
            if trump.is_trump(suit) && dealt.contains(&king) && dealt.contains(&queen) {
                points += 20;
            }
        }
    }
    points
}

fn belote(hands: &[Vec<Card>; 4], trick: &[SeatCard], trump: Trump) -> i32 {
    belote_for(0, hands, trick, trump) + belote_for(1, hands, trick, trump)
}

fn deck_points(hands: &[Vec<Card>; 4], trick: &[SeatCard], trump: Trump) -> i32 {
    trick_cards(trick)
        .iter()
        .chain(hands.iter().flatten())
        .map(|&card| rules::points(card, trump))
        .sum()
}

fn shuffled_deck(random: &mut SplitMix64) -> Vec<Card> {
    let mut deck = Card::deck();
    shuffle(&mut deck, random);
    deck
}

#[test]
fn matches_an_exhaustive_search_on_random_endgames() {
    let mut solver = DoubleDummySolver::new();
    let mut exhaustive = Exhaustive::new();
    let mut random = SplitMix64::new(7);
    for test in 0..500 {
        let cards_each = 1 + test % 5;
        let trump = Trump::ALL[random.next_int(Trump::ALL.len() as i32) as usize];
        let deck = shuffled_deck(&mut random);
        let mut hands: [Vec<Card>; 4] =
            std::array::from_fn(|seat| deck[seat * cards_each..(seat + 1) * cards_each].to_vec());
        let leader = random.next_int(4) as Seat;
        // Start some positions in the middle of the first trick, with random legal cards.
        let mut current = Vec::new();
        for pos in 0..test % 3 {
            let seat = (leader + pos) % 4;
            let legal = rules::legal_cards(&hands[seat], &trick_cards(&current), trump);
            let card = legal[random.next_int(legal.len() as i32) as usize];
            remove(&mut hands[seat], card);
            current.push(SeatCard { seat, card });
        }
        let position = Position {
            hands: hands.clone(),
            trump,
            tricks: Vec::new(),
            current_trick: current.clone(),
            first_leader: leader,
        };

        let solution = solver.solve(&position);

        let total = deck_points(&hands, &current, trump) + 10 + belote(&hands, &current, trump);
        let expected = exhaustive.value(&hands, &current, leader, trump)
            + belote_for(0, &hands, &current, trump);
        assert_eq!(solution.north_south_points, expected, "test {test}");
        assert_eq!(
            solution.north_south_points + solution.east_west_points,
            total,
            "test {test}"
        );
        let seat = (leader + current.len()) % 4;
        let best = solution.best_card.unwrap();
        assert!(
            rules::legal_cards(&hands[seat], &trick_cards(&current), trump).contains(&best),
            "test {test}"
        );
        assert_eq!(
            exhaustive.value_after(&hands, &current, leader, trump, best)
                + belote_for(0, &hands, &current, trump),
            expected,
            "the best card achieves the result, test {test}"
        );
    }
}

#[test]
fn stays_exact_when_the_search_window_falls_outside_the_remaining_points() {
    // A position from a played round in which the transposition table once stored a lower bound
    // as an upper one.
    let hands = [
        cards("KC 7S 10D 7H KS 7D"),
        cards("AC 10C QH AH 8H 8D"),
        cards("9H 9D QD JD 9C 10H"),
        cards("7C KH QC AD JC 8C KD"),
    ];
    let current = vec![seat_card(0, "8S"), seat_card(1, "9S"), seat_card(2, "JH")];
    let position = Position {
        hands: hands.clone(),
        trump: Trump::Spades,
        tricks: Vec::new(),
        current_trick: current.clone(),
        first_leader: 0,
    };

    let solution = DoubleDummySolver::new().solve(&position);

    assert_eq!(
        solution.north_south_points,
        Exhaustive::new().value(&hands, &current, 0, Trump::Spades)
    );
}

fn deal_eight(deck: &[Card]) -> [Vec<Card>; 4] {
    std::array::from_fn(|seat| deck[seat * 8..seat * 8 + 8].to_vec())
}

#[test]
fn solves_a_full_deal_consistently() {
    let deck = shuffled_deck(&mut SplitMix64::new(3));
    let hands = deal_eight(&deck);

    let solution =
        DoubleDummySolver::new().solve(&Position::start(hands.clone(), Trump::Spades, 1));

    assert_eq!(
        solution.north_south_points + solution.east_west_points,
        162 + belote(&hands, &[], Trump::Spades)
    );
    assert!(hands[1].contains(&solution.best_card.unwrap()));
}

#[test]
fn a_full_deal_totals_162_points_and_its_belotes_in_every_contract() {
    let deck = shuffled_deck(&mut SplitMix64::new(5));
    let hands = deal_eight(&deck);
    let mut solver = DoubleDummySolver::new();
    for trump in Trump::ALL {
        let solution = solver.solve(&Position::start(hands.clone(), trump, 1));

        assert_eq!(
            solution.north_south_points + solution.east_west_points,
            162 + belote(&hands, &[], trump),
            "{trump:?}"
        );
        assert!(hands[1].contains(&solution.best_card.unwrap()), "{trump:?}");
    }
}

#[test]
fn playing_the_solvers_cards_in_a_real_round_gives_the_predicted_result() {
    let mut solver = DoubleDummySolver::new();
    let mut random = SplitMix64::new(11);
    for deal in 0..12 {
        let deck = shuffled_deck(&mut random);
        let hands = deal_eight(&deck);
        let trump = Trump::ALL[deal % Trump::ALL.len()];
        let dealer = deal % 4;
        let mut board =
            GameBoard::from_contract(hands.clone(), trump, deal % 2, 80, false, dealer, false);
        let predicted = solver.solve(&Position::start(hands, trump, (dealer + 1) % 4));

        while board.round_result().is_none() {
            if board.reviewing_completed_trick() {
                board.continue_after_trick().unwrap();
                continue;
            }
            let seat = board.active_seat();
            let view = board.play_view(seat);
            let position = Position {
                hands: std::array::from_fn(|other| board.hand(other).to_vec()),
                trump,
                tricks: view.tricks.clone(),
                current_trick: view.current_trick.clone(),
                first_leader: (dealer + 1) % 4,
            };
            let solution = solver.solve(&position);
            assert_eq!(
                solution.north_south_points, predicted.north_south_points,
                "deal {deal}"
            );
            board
                .play(seat, solution.best_card.unwrap(), false)
                .unwrap();
        }
        let result = board.round_result().unwrap();
        assert_eq!(
            result.north_south_card_points
                + result.north_south_dix_de_der
                + result.north_south_belote_bonus,
            predicted.north_south_points,
            "deal {deal}"
        );
        assert_eq!(
            result.east_west_card_points
                + result.east_west_dix_de_der
                + result.east_west_belote_bonus,
            predicted.east_west_points,
            "deal {deal}"
        );
    }
}

#[test]
fn the_tables_live_on_the_heap() {
    // Four solvers at once would overflow a test thread's stack if the tables were arrays.
    let solvers: Vec<DoubleDummySolver> = (0..4).map(|_| DoubleDummySolver::new()).collect();

    assert!(solvers.iter().all(|solver| solver.nodes() == 0));
}
