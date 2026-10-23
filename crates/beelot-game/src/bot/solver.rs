//! Finds the best play of a round when all four hands are known (a double-dummy solver):
//! North–South maximise their points and East–West minimise them, by alpha-beta search with a
//! transposition table at the start of each trick. The result counts the card points, the
//! last-trick bonus (dix de der) and the belote. A solver keeps its table between calls, so it is
//! borrowed mutably. Port of the Java `DoubleDummySolver`, with the same search order, so the
//! same best card.

use crate::board::SeatCard;
use crate::card::{Card, Rank, Seat, Suit};
use crate::rules;

const TABLE_BITS: u32 = 20;
const TABLE_SIZE: usize = 1 << TABLE_BITS;
const KEY_BITS: u32 = 34;
const GOLDEN_GAMMA: i64 = 0x9E3779B97F4A7C15_u64 as i64;

/// A round in progress: the cards each seat still holds (seats numbered in play order from 0,
/// North–South holding the even seats), the trump suit, the completed tricks, the cards of the
/// trick being played, and the seat that led the first trick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position {
    pub hands: [Vec<Card>; 4],
    pub trump: Suit,
    pub tricks: Vec<Vec<SeatCard>>,
    pub current_trick: Vec<SeatCard>,
    pub first_leader: Seat,
}

impl Position {
    /// A round before its first card, led by the given seat.
    pub fn start(hands: [Vec<Card>; 4], trump: Suit, leader: Seat) -> Self {
        Self {
            hands,
            trump,
            tricks: Vec::new(),
            current_trick: Vec::new(),
            first_leader: leader,
        }
    }
}

/// The points each team scores when both play their best from the position, counting the points
/// already won, and a card that achieves it for the seat to play (`None` once the round is over).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Solution {
    pub north_south_points: i32,
    pub east_west_points: i32,
    pub best_card: Option<Card>,
}

/// Card sets are bit masks: the card at bit (8 × suit + n) is the n-th weakest card of that suit
/// under the current trump.
pub struct DoubleDummySolver {
    // On the heap: 12 MiB of tables would overflow a thread's stack.
    table_keys: Vec<i64>,
    table_lower: Vec<i16>,
    table_upper: Vec<i16>,
    cards: [Card; 32],
    points: [i32; 32],
    hands: [u32; 4],
    trick: [u32; 4],
    trump_suit: u32,
    nodes: u64,
    /// Table entries carry the number of the solve that wrote them, so the table never needs
    /// clearing.
    generation: i64,
}

impl Default for DoubleDummySolver {
    fn default() -> Self {
        Self::new()
    }
}

impl DoubleDummySolver {
    pub fn new() -> Self {
        Self {
            table_keys: vec![0; TABLE_SIZE],
            table_lower: vec![0; TABLE_SIZE],
            table_upper: vec![0; TABLE_SIZE],
            cards: [Card::new(Rank::Seven, Suit::Clubs); 32],
            points: [0; 32],
            hands: [0; 4],
            trick: [0; 4],
            trump_suit: 0,
            nodes: 0,
            generation: 0,
        }
    }

    /// The position must be consistent: `current_trick` holds fewer than four cards, and the
    /// hands are sized to match the trick state, each seat that has played to the current trick
    /// holding one card fewer than each seat yet to play, which all hold as many as one another.
    /// The completed tricks are not checked against the hands, so a caller may leave out tricks
    /// it does not need. Checked in debug builds.
    pub fn solve(&mut self, position: &Position) -> Solution {
        debug_assert!(
            position.current_trick.len() < 4,
            "a complete trick is no current trick"
        );
        debug_assert!(
            {
                let played = |seat: Seat| position.current_trick.iter().any(|c| c.seat == seat);
                let waiting = (0..4)
                    .find(|&seat| !played(seat))
                    .map(|seat| position.hands[seat].len());
                (0..4).all(|seat| {
                    let size = position.hands[seat].len();
                    waiting.is_some_and(|n| size + usize::from(played(seat)) == n)
                })
            },
            "hand sizes must match the trick state"
        );
        self.prepare(position.trump);
        for seat in 0..4 {
            self.hands[seat] = self.mask(&position.hands[seat]);
        }
        let won = points_won(position);
        let belote = belote_seat(position);
        let belote_for = |team: Seat| {
            if belote.is_some_and(|seat| seat % 2 == team) {
                20
            } else {
                0
            }
        };

        let current = &position.current_trick;
        let remaining = self.remaining();
        let mut remaining_points = self.points_of(remaining) + if remaining == 0 { 0 } else { 10 };
        for played in current {
            remaining_points += self.points[self.index(played.card) as usize];
        }
        let mut best = None;
        let mut value = 0;
        if remaining != 0 {
            let leader = match current.first() {
                Some(first) => first.seat,
                None => leader_of(position),
            };
            for (pos, played) in current.iter().enumerate() {
                self.trick[pos] = self.index(played.card);
            }
            let mut best_index = None;
            value = self.play(
                current.len(),
                leader,
                0,
                remaining_points,
                Some(&mut best_index),
            );
            best = best_index.map(|index| self.cards[index as usize]);
        }
        Solution {
            north_south_points: won[0] + value + belote_for(0),
            east_west_points: won[1] + remaining_points - value + belote_for(1),
            best_card: best,
        }
    }

    /// Positions searched by the last calls, for benchmarks.
    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    fn prepare(&mut self, trump: Suit) {
        self.trump_suit = trump.ordinal() as u32;
        for suit in Suit::ALL {
            for rank in Rank::ALL {
                let card = Card::new(rank, suit);
                let bit = 8 * suit.ordinal() + rules::strength(card, trump) as usize;
                self.cards[bit] = card;
                self.points[bit] = rules::points(card, trump);
            }
        }
        self.generation = self.generation.wrapping_add(1);
    }

    fn remaining(&self) -> u32 {
        self.hands[0] | self.hands[1] | self.hands[2] | self.hands[3]
    }

    /// The best North–South points from the remaining cards, with `position` cards already in the
    /// trick led by `leader`. When `best_index` is given, it receives the best card for the seat to
    /// play.
    fn play(
        &mut self,
        position: usize,
        leader: Seat,
        mut alpha: i32,
        mut beta: i32,
        mut best_index: Option<&mut Option<u32>>,
    ) -> i32 {
        self.nodes += 1;
        let seat = (leader + position) & 3;
        let mut moves = self.legal(seat, position);
        let north_south = seat & 1 == 0;
        let mut best = if north_south { i32::MIN } else { i32::MAX };
        while moves != 0 {
            let card = 31 - moves.leading_zeros();
            moves &= !(1 << card);
            self.hands[seat] &= !(1 << card);
            self.trick[position] = card;
            let value = if position < 3 {
                self.play(position + 1, leader, alpha, beta, None)
            } else {
                let winner = (leader + self.winning_position(4)) & 3;
                let remaining = self.remaining();
                let trick_points = self
                    .trick
                    .iter()
                    .map(|&played| self.points[played as usize])
                    .sum::<i32>()
                    + if remaining == 0 { 10 } else { 0 };
                let gain = if winner & 1 == 0 { trick_points } else { 0 };
                if remaining == 0 {
                    gain
                } else {
                    // The next trick reuses the trick buffer: keep this trick's cards for the
                    // remaining moves.
                    let [first, second, third, _] = self.trick;
                    let value = gain + self.start_trick(winner, alpha - gain, beta - gain);
                    self.trick[0] = first;
                    self.trick[1] = second;
                    self.trick[2] = third;
                    value
                }
            };
            self.hands[seat] |= 1 << card;
            let improves = if north_south {
                value > best
            } else {
                value < best
            };
            if improves {
                best = value;
                if let Some(best_index) = best_index.as_deref_mut() {
                    *best_index = Some(card);
                }
            }
            if north_south {
                alpha = alpha.max(value);
            } else {
                beta = beta.min(value);
            }
            if alpha >= beta {
                break;
            }
        }
        best
    }

    /// The best North–South points from the remaining cards when the given seat leads the next
    /// trick.
    fn start_trick(&mut self, leader: Seat, alpha: i32, beta: i32) -> i32 {
        let remaining = self.remaining();
        let key = (self.generation << KEY_BITS) | (i64::from(remaining) << 2) | leader as i64;
        let slot = ((key.wrapping_mul(GOLDEN_GAMMA) as u64) >> (64 - TABLE_BITS)) as usize;
        let mut lower = 0;
        let mut upper = self.points_of(remaining) + 10;
        if self.table_keys[slot] == key {
            lower = i32::from(self.table_lower[slot]);
            upper = i32::from(self.table_upper[slot]);
        }
        // Cutting off here also keeps the search window below non-empty, so its result is an
        // unambiguous bound.
        if lower >= beta || lower == upper {
            return lower;
        }
        if upper <= alpha {
            return upper;
        }
        let low = alpha.max(lower);
        let high = beta.min(upper);
        let value = self.play(0, leader, low, high, None);
        if value <= low {
            upper = value;
        } else if value >= high {
            lower = value;
        } else {
            lower = value;
            upper = value;
        }
        self.table_keys[slot] = key;
        self.table_lower[slot] = lower as i16;
        self.table_upper[slot] = upper as i16;
        value
    }

    /// The cards the seat may play, as in [`rules::legal_cards`], on the bits of this solver.
    fn legal(&self, seat: Seat, position: usize) -> u32 {
        let hand = self.hands[seat];
        if position == 0 {
            return hand;
        }
        let lead_suit = self.trick[0] >> 3;
        let winner = self.winning_position(position);
        let winning_card = self.trick[winner];
        let follow = hand & suit_mask(lead_suit);
        if follow != 0 {
            if lead_suit != self.trump_suit {
                return follow;
            }
            let higher = follow & above(winning_card);
            return if higher != 0 { higher } else { follow };
        }
        // Java compares with `position - 2`, which is -1 for the second card.
        if winner + 2 == position {
            return hand;
        }
        let trumps = hand & suit_mask(self.trump_suit);
        if trumps == 0 {
            return hand;
        }
        if winning_card >> 3 != self.trump_suit {
            return trumps;
        }
        let higher = trumps & above(winning_card);
        if higher != 0 { higher } else { trumps }
    }

    fn winning_position(&self, size: usize) -> usize {
        let mut winner = 0;
        for pos in 1..size {
            let contender = self.trick[pos];
            let current = self.trick[winner];
            let beats = if contender >> 3 == current >> 3 {
                contender > current
            } else {
                contender >> 3 == self.trump_suit
            };
            if beats {
                winner = pos;
            }
        }
        winner
    }

    fn points_of(&self, mut mask: u32) -> i32 {
        let mut total = 0;
        while mask != 0 {
            total += self.points[mask.trailing_zeros() as usize];
            mask &= mask - 1;
        }
        total
    }

    fn index(&self, card: Card) -> u32 {
        let trump = Suit::ALL[self.trump_suit as usize];
        8 * card.suit.ordinal() as u32 + rules::strength(card, trump) as u32
    }

    fn mask(&self, hand: &[Card]) -> u32 {
        hand.iter()
            .fold(0, |mask, &card| mask | 1 << self.index(card))
    }
}

/// The cards of the same suit stronger than the given card.
fn above(card: u32) -> u32 {
    let higher_bits = if card == 31 {
        0
    } else {
        u32::MAX << (card + 1)
    };
    higher_bits & suit_mask(card >> 3)
}

fn suit_mask(suit: u32) -> u32 {
    0xFF << (8 * suit)
}

fn trick_cards(trick: &[SeatCard]) -> Vec<Card> {
    trick.iter().map(|played| played.card).collect()
}

/// Points each team has already won in the completed tricks, with the dix de der once all are
/// played.
fn points_won(position: &Position) -> [i32; 2] {
    let mut won = [0; 2];
    let trump = position.trump;
    for (number, completed) in position.tricks.iter().enumerate() {
        let cards = trick_cards(completed);
        let winner = completed[rules::winning_index(&cards, trump)].seat;
        let mut trick_points: i32 = cards.iter().map(|&card| rules::points(card, trump)).sum();
        if number == 7 {
            trick_points += 10;
        }
        won[winner & 1] += trick_points;
    }
    won
}

fn leader_of(position: &Position) -> Seat {
    match position.tricks.last() {
        None => position.first_leader,
        Some(last) => last[rules::winning_index(&trick_cards(last), position.trump)].seat,
    }
}

/// The seat dealt both the king and the queen of trumps.
fn belote_seat(position: &Position) -> Option<Seat> {
    let king = Card::new(Rank::King, position.trump);
    let queen = Card::new(Rank::Queen, position.trump);
    (0..4).find(|&seat| {
        let dealt: Vec<Card> = position.hands[seat]
            .iter()
            .copied()
            .chain(
                position
                    .tricks
                    .iter()
                    .flatten()
                    .chain(&position.current_trick)
                    .filter(|played| played.seat == seat)
                    .map(|played| played.card),
            )
            .collect();
        dealt.contains(&king) && dealt.contains(&queen)
    })
}
