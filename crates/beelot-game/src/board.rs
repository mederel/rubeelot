//! Card play of one round: turn order, legal cards, belote, announcements, tricks and scoring.

use serde::{Deserialize, Serialize};

use crate::announcements::{self, Declared};
use crate::card::{Card, Rank, Seat, Suit, Team, Trump, Variant};
use crate::deck::{self, Deck};
use crate::error::GameError;
use crate::rules;
use crate::scoring::{self, Declaration, RoundResult, RoundSummary, Totals};

/// A card played from the given seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeatCard {
    pub seat: Seat,
    pub card: Card,
}

/// A seat's view of the card play, for bots: the hand in dealt order, its legal cards, the
/// contract and every card played so far.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayView {
    pub seat: Seat,
    pub hand: Vec<Card>,
    pub legal_cards: Vec<Card>,
    pub trump: Trump,
    pub variant: Variant,
    pub contract_value: i32,
    pub coinched: bool,
    pub declaring_seat: Seat,
    pub tricks: Vec<Vec<SeatCard>>,
    pub current_trick: Vec<SeatCard>,
    pub announcements: bool,
    pub revealed_announcements: Vec<Declaration>,
}

impl PlayView {
    /// Whether this seat is on the declaring team.
    pub fn declaring(&self) -> bool {
        self.seat % 2 == self.declaring_seat % 2
    }
}

/// A seat's King and Queen of a trump suit: a Belote, declared on the first card and scored on the
/// second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Belote {
    pub seat: Seat,
    pub suit: Suit,
}

/// The latest Belote card played: the first of a pair declares Belote, the second Rebelote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeloteDeclaration {
    pub seat: Seat,
    pub rebelote: bool,
}

#[derive(Debug, Clone)]
pub struct GameBoard {
    hands: [Vec<Card>; 4],
    trump: Trump,
    declarer: Seat,
    variant: Variant,
    contract_value: i32,
    coinched: bool,
    dealer: Seat,
    announcements: bool,
    current_trick: Vec<SeatCard>,
    completed_trick: Vec<SeatCard>,
    tricks: Vec<Vec<SeatCard>>,
    /// Each team's tricks as a face-down pile, top card first (US-085).
    north_south_pile: Vec<Card>,
    east_west_pile: Vec<Card>,
    active: Seat,
    completed_tricks: usize,
    next_leader: Seat,
    north_south_score: i32,
    east_west_score: i32,
    north_south_card_points: i32,
    east_west_card_points: i32,
    north_south_dix_de_der: i32,
    east_west_dix_de_der: i32,
    north_south_tricks: i32,
    reviewing: bool,
    /// One per trump suit in which a seat holds both the King and the Queen.
    belotes: Vec<Belote>,
    /// How many cards of each Belote have been played, in the order of `belotes`.
    belote_cards_played: Vec<u32>,
    belote_bonus_by_team: [i32; 2],
    /// The Belote bonus of the team that made the latest Rebelote.
    belote_bonus: i32,
    last_belote: Option<BeloteDeclaration>,
    round_result: Option<RoundResult>,
    declared: Vec<Declared>,
    revealed: Vec<Declaration>,
    north_south_announcements: i32,
    east_west_announcements: i32,
}

fn has_belote(hand: &[Card], suit: Suit) -> bool {
    hand.contains(&Card::new(Rank::King, suit)) && hand.contains(&Card::new(Rank::Queen, suit))
}

impl GameBoard {
    /// A classic Belote round; the seat after the dealer leads. With `announcements`, seats may
    /// declare them on their first card.
    pub fn from_bidding(
        hands: [Vec<Card>; 4],
        trump: Suit,
        declarer: Seat,
        dealer: Seat,
        announcements: bool,
    ) -> Self {
        assert!(
            hands.iter().all(|hand| hand.len() == 8),
            "Every player must have eight cards."
        );
        Self::new(
            hands,
            trump.into(),
            declarer,
            Variant::Classic,
            82,
            false,
            dealer,
            announcements,
        )
    }

    /// A Contrée round with a contract from 80 to 160 in steps of ten.
    pub fn from_contract(
        hands: [Vec<Card>; 4],
        trump: Trump,
        declarer: Seat,
        contract_value: i32,
        coinched: bool,
        dealer: Seat,
        announcements: bool,
    ) -> Self {
        assert!(
            (80..=160).contains(&contract_value) && contract_value % 10 == 0,
            "Invalid Contrée contract."
        );
        assert!(
            hands.iter().all(|hand| hand.len() == 8),
            "A Contrée table needs four hands of eight cards."
        );
        Self::new(
            hands,
            trump,
            declarer,
            Variant::Contree,
            contract_value,
            coinched,
            dealer,
            announcements,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        hands: [Vec<Card>; 4],
        trump: Trump,
        declarer: Seat,
        variant: Variant,
        contract_value: i32,
        coinched: bool,
        dealer: Seat,
        announcements: bool,
    ) -> Self {
        assert!(declarer < 4 && dealer < 4, "Seats are numbered 0 to 3.");
        let mut belotes = Vec::new();
        for suit in Suit::ALL.into_iter().filter(|&suit| trump.is_trump(suit)) {
            for (seat, hand) in hands.iter().enumerate() {
                if has_belote(hand, suit) {
                    belotes.push(Belote { seat, suit });
                }
            }
        }
        Self {
            hands,
            trump,
            declarer,
            variant,
            contract_value,
            coinched,
            dealer,
            announcements,
            current_trick: Vec::new(),
            completed_trick: Vec::new(),
            tricks: Vec::new(),
            north_south_pile: Vec::new(),
            east_west_pile: Vec::new(),
            active: (dealer + 1) % 4,
            completed_tricks: 0,
            next_leader: 0,
            north_south_score: 0,
            east_west_score: 0,
            north_south_card_points: 0,
            east_west_card_points: 0,
            north_south_dix_de_der: 0,
            east_west_dix_de_der: 0,
            north_south_tricks: 0,
            reviewing: false,
            belote_cards_played: vec![0; belotes.len()],
            belotes,
            belote_bonus_by_team: [0; 2],
            belote_bonus: 0,
            last_belote: None,
            round_result: None,
            declared: Vec::new(),
            revealed: Vec::new(),
            north_south_announcements: 0,
            east_west_announcements: 0,
        }
    }

    /// Plays the card. With `declare` on the seat's first card, and announcements on, the
    /// announcements of the hand are declared before the card leaves it; otherwise `declare` is
    /// ignored.
    pub fn play(&mut self, seat: Seat, card: Card, declare: bool) -> Result<(), GameError> {
        if seat != self.active {
            return Err(GameError::NotYourTurn);
        }
        if !self.legal_cards(seat).contains(&card) {
            return Err(GameError::IllegalCard);
        }
        self.register_belote(seat, card);
        if self.announcements && declare && self.hands[seat].len() == 8 {
            let found = announcements::find(&self.hands[seat], self.trump);
            if !found.is_empty() {
                self.declared.push(Declared {
                    seat,
                    announcements: found,
                });
            }
        }
        let position = self.hands[seat]
            .iter()
            .position(|held| *held == card)
            .expect("a legal card is in the hand");
        self.hands[seat].remove(position);
        self.current_trick.push(SeatCard { seat, card });
        self.active = (self.active + 1) % 4;
        if self.current_trick.len() == 4 {
            self.resolve_trick();
        }
        Ok(())
    }

    /// Moves on from a reviewed trick. Once each seat holds a single card, the last trick is
    /// played for them, starting with its leader.
    pub fn continue_after_trick(&mut self) -> Result<(), GameError> {
        if self.round_result.is_some() {
            return Err(GameError::RoundEnded);
        }
        if !self.reviewing {
            return Err(GameError::NoCompletedTrick);
        }
        self.current_trick.clear();
        self.reviewing = false;
        self.active = self.next_leader;
        if self.completed_tricks == 7 {
            while !self.reviewing {
                let seat = self.active;
                let card = self.hands[seat][0];
                self.play(seat, card, false)?;
            }
        }
        Ok(())
    }

    pub fn legal_cards(&self, seat: Seat) -> Vec<Card> {
        if self.reviewing || seat != self.active {
            return Vec::new();
        }
        let trick: Vec<Card> = self
            .current_trick
            .iter()
            .map(|played| played.card)
            .collect();
        rules::legal_cards(&self.hands[seat], &trick, self.trump)
    }

    /// What the seat can see when choosing a card: its own hand in dealt order, its legal cards,
    /// the contract and every card played so far with the seat that played it.
    pub fn play_view(&self, seat: Seat) -> PlayView {
        assert!(seat < 4, "Seats are numbered 0 to 3.");
        PlayView {
            seat,
            hand: self.hands[seat].clone(),
            legal_cards: self.legal_cards(seat),
            trump: self.trump,
            variant: self.variant,
            contract_value: self.contract_value,
            coinched: self.coinched,
            declaring_seat: self.declarer,
            tricks: self.tricks.clone(),
            current_trick: if self.reviewing {
                Vec::new()
            } else {
                self.current_trick.clone()
            },
            announcements: self.announcements,
            revealed_announcements: self.revealed.clone(),
        }
    }

    /// The finished round's contract and result, or `None` while the round is being played.
    pub fn round_summary(&self) -> Option<RoundSummary> {
        self.round_result.as_ref().map(|result| RoundSummary {
            declaring_team: Team::of(self.declarer),
            trump: self.trump,
            contract_value: (self.variant == Variant::Contree).then_some(self.contract_value),
            coinched: self.coinched,
            result: result.clone(),
        })
    }

    pub fn active_seat(&self) -> Seat {
        self.active
    }

    pub fn reviewing_completed_trick(&self) -> bool {
        self.reviewing
    }

    /// The tricks a team has won, as a face-down pile listed from its top card: each trick goes on
    /// top of the earlier ones, its first card on top (US-085).
    pub fn trick_pile(&self, north_south: bool) -> &[Card] {
        if north_south {
            &self.north_south_pile
        } else {
            &self.east_west_pile
        }
    }

    /// The cards of the finished deal, gathered for the next one without a shuffle: each team
    /// counts its pile, then the next dealer's team puts its counted pile on top of the other.
    /// `None` until the round is played.
    pub fn gathered_deck(&self, next_dealer: Seat) -> Option<Deck> {
        self.round_result.as_ref()?;
        Some(Deck::gathered_piles(
            &self.north_south_pile,
            &self.east_west_pile,
            next_dealer.is_multiple_of(2),
        ))
    }

    pub fn round_result(&self) -> Option<&RoundResult> {
        self.round_result.as_ref()
    }

    pub fn trump(&self) -> Trump {
        self.trump
    }

    pub fn variant(&self) -> Variant {
        self.variant
    }

    pub fn contract_value(&self) -> i32 {
        self.contract_value
    }

    pub fn coinched(&self) -> bool {
        self.coinched
    }

    pub fn declarer(&self) -> Seat {
        self.declarer
    }

    pub fn dealer(&self) -> Seat {
        self.dealer
    }

    pub fn hand(&self, seat: Seat) -> &[Card] {
        &self.hands[seat]
    }

    /// The completed trick while it is being reviewed, otherwise the trick in progress.
    pub fn visible_trick(&self) -> &[SeatCard] {
        if self.reviewing {
            &self.completed_trick
        } else {
            &self.current_trick
        }
    }

    pub fn completed_tricks(&self) -> usize {
        self.completed_tricks
    }

    pub fn next_leader(&self) -> Seat {
        self.next_leader
    }

    /// The live scores of North-South and East-West.
    pub fn scores(&self) -> (i32, i32) {
        (self.north_south_score, self.east_west_score)
    }

    /// The round's Belotes: at most one in a suit contract, up to four under all trumps, none under
    /// no trumps.
    pub fn belotes(&self) -> &[Belote] {
        &self.belotes
    }

    pub fn last_belote(&self) -> Option<BeloteDeclaration> {
        self.last_belote
    }

    /// The Belote bonus scored so far by the team of the latest Rebelote.
    pub fn belote_bonus(&self) -> i32 {
        self.belote_bonus
    }

    /// The announcements declared on the first trick, in declaration order.
    pub fn declared(&self) -> &[Declared] {
        &self.declared
    }

    /// Every declaration, once the first trick is over; empty until then.
    pub fn revealed(&self) -> &[Declaration] {
        &self.revealed
    }

    fn trick_points(&self, trick: &[SeatCard]) -> i32 {
        trick
            .iter()
            .map(|played| rules::points(played.card, self.trump))
            .sum()
    }

    fn resolve_trick(&mut self) {
        let cards: Vec<Card> = self
            .current_trick
            .iter()
            .map(|played| played.card)
            .collect();
        let winner = self.current_trick[rules::winning_index(&cards, self.trump)];
        let mut points = self.trick_points(&self.current_trick);
        let last_trick = self.completed_tricks == 7;
        if last_trick {
            points += 10;
        }
        let card_points = points - if last_trick { 10 } else { 0 };
        self.next_leader = winner.seat;
        let pile = if self.next_leader.is_multiple_of(2) {
            &mut self.north_south_pile
        } else {
            &mut self.east_west_pile
        };
        *pile = deck::pile_trick(pile, &cards);
        if self.next_leader.is_multiple_of(2) {
            self.north_south_tricks += 1;
            self.north_south_score += points;
            self.north_south_card_points += card_points;
            if last_trick {
                self.north_south_dix_de_der = 10;
            }
        } else {
            self.east_west_score += points;
            self.east_west_card_points += card_points;
            if last_trick {
                self.east_west_dix_de_der = 10;
            }
        }
        self.completed_trick = self.current_trick.clone();
        self.tricks.push(self.completed_trick.clone());
        self.completed_tricks += 1;
        self.reviewing = true;
        if self.completed_tricks == 1 {
            self.reveal_announcements();
        }
        if self.completed_tricks == 8 {
            self.calculate_round_result();
        }
    }

    fn calculate_round_result(&mut self) {
        let [north_south_belote, east_west_belote] = self.belote_bonus_by_team;
        let totals = Totals {
            variant: self.variant,
            north_south_declares: self.declarer.is_multiple_of(2),
            contract_value: self.contract_value,
            coinched: self.coinched,
            north_south_card_points: self.north_south_card_points,
            east_west_card_points: self.east_west_card_points,
            north_south_dix_de_der: self.north_south_dix_de_der,
            east_west_dix_de_der: self.east_west_dix_de_der,
            north_south_belote,
            east_west_belote,
            north_south_tricks: self.north_south_tricks,
            north_south_announcements: self.north_south_announcements,
            east_west_announcements: self.east_west_announcements,
        };
        self.round_result = Some(scoring::score(&totals, &self.revealed));
    }

    /// Only the team with the best announcement scores, all of its own; its points join the live
    /// score at once.
    fn reveal_announcements(&mut self) {
        let Some(best) = announcements::best_seat(&self.declared, self.trump) else {
            return;
        };
        let scoring_parity = best % 2;
        let mut declarations = Vec::new();
        for entry in &self.declared {
            let scored = entry.seat % 2 == scoring_parity;
            declarations.push(Declaration {
                seat: entry.seat,
                announcements: entry.announcements.clone(),
                scored,
            });
            if !scored {
                continue;
            }
            let points: i32 = entry.announcements.iter().map(|a| a.value).sum();
            if scoring_parity == 0 {
                self.north_south_announcements =
                    self.north_south_announcements.wrapping_add(points);
            } else {
                self.east_west_announcements = self.east_west_announcements.wrapping_add(points);
            }
        }
        self.north_south_score = self
            .north_south_score
            .wrapping_add(self.north_south_announcements);
        self.east_west_score = self
            .east_west_score
            .wrapping_add(self.east_west_announcements);
        self.revealed = declarations;
    }

    fn register_belote(&mut self, seat: Seat, card: Card) {
        if !matches!(card.rank, Rank::King | Rank::Queen) {
            return;
        }
        let Some(index) = self
            .belotes
            .iter()
            .position(|belote| belote.seat == seat && belote.suit == card.suit)
        else {
            return;
        };
        self.belote_cards_played[index] += 1;
        let rebelote = self.belote_cards_played[index] == 2;
        self.last_belote = Some(BeloteDeclaration { seat, rebelote });
        if !rebelote {
            return;
        }
        if seat.is_multiple_of(2) {
            self.north_south_score += 20;
        } else {
            self.east_west_score += 20;
        }
        self.belote_bonus_by_team[seat % 2] += 20;
        self.belote_bonus = self.belote_bonus_by_team[seat % 2];
    }
}
