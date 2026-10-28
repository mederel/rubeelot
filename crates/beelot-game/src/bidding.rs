//! The auction before a round: Classic accept-or-pass on an upturned card, Contrée bidding.

use serde::{Deserialize, Serialize};

use crate::board::GameBoard;
use crate::card::{Card, Seat, Suit, Trump, Variant};
use crate::error::GameError;
use crate::rng::{RandomSource, java_shuffle};

/// Serializes `Option<Seat>` as the seat, or `-1` for `None`.
mod minus_one_none {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use crate::card::Seat;

    pub fn serialize<S: Serializer>(
        value: &Option<Seat>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(seat) => (*seat as i64).serialize(serializer),
            None => (-1_i64).serialize(serializer),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Seat>, D::Error> {
        let value = i64::deserialize(deserializer)?;
        Ok(usize::try_from(value).ok())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CallType {
    Pass,
    Bid,
    Trump,
    Coinche,
}

/// A call made from the given seat; `value` is set for a bid, `trump` for a bid or a chosen trump.
/// The fixtures recorded before US-063 name the trump `suit`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatCall {
    pub seat: Seat,
    #[serde(rename = "type")]
    pub r#type: CallType,
    pub value: i32,
    #[serde(rename = "suit", alias = "trump")]
    pub trump: Option<Trump>,
}

/// A seat's view of the auction, for bots. `highest_bidder_seat` is `-1` in JSON before the first bid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionView {
    pub seat: Seat,
    pub hand: Vec<Card>,
    pub upturned_card: Option<Card>,
    pub round: i32,
    pub variant: Variant,
    pub dealer_index: Seat,
    pub auction: Vec<SeatCall>,
    pub highest_bid: i32,
    pub highest_bid_suit: Option<Trump>,
    #[serde(with = "minus_one_none")]
    pub highest_bidder_seat: Option<Seat>,
    pub coinche_allowed: bool,
    pub announcements: bool,
    /// Whether bids may name all trumps or no trumps (US-063); off in fixtures recorded before.
    #[serde(default)]
    pub all_trumps: bool,
}

impl AuctionView {
    /// Whether this player's partner holds the highest bid.
    pub fn partner_holds_contract(&self) -> bool {
        self.highest_bidder_seat
            .is_some_and(|bidder| bidder != self.seat && bidder % 2 == self.seat % 2)
    }

    /// Whether this player has already made a contract bid in this auction.
    pub fn has_bid(&self) -> bool {
        self.auction
            .iter()
            .any(|call| call.seat == self.seat && call.r#type == CallType::Bid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuctionDecision {
    Pass,
    /// A Contrée bid in a suit, all trumps or no trumps.
    Bid {
        value: i32,
        #[serde(rename = "suit")]
        trump: Trump,
    },
    Coinche,
    Trump {
        suit: Suit,
    },
}

pub struct BiddingState {
    variant: Variant,
    rng: Box<dyn RandomSource + Send>,
    deals: u32,
    hands: [Vec<Card>; 4],
    remaining_deck: Vec<Card>,
    upturned_card: Option<Card>,
    dealer: Seat,
    active: Seat,
    round: i32,
    consecutive_passes: usize,
    highest_bid: i32,
    highest_bidder: Option<Seat>,
    highest_bid_suit: Option<Trump>,
    coinched: bool,
    completed_board: Option<GameBoard>,
    auction: Vec<SeatCall>,
    announcements: bool,
    all_trumps: bool,
}

impl std::fmt::Debug for BiddingState {
    /// Leaves out the random source.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BiddingState")
            .field("variant", &self.variant)
            .field("deals", &self.deals)
            .field("hands", &self.hands)
            .field("remaining_deck", &self.remaining_deck)
            .field("upturned_card", &self.upturned_card)
            .field("dealer", &self.dealer)
            .field("active", &self.active)
            .field("round", &self.round)
            .field("consecutive_passes", &self.consecutive_passes)
            .field("highest_bid", &self.highest_bid)
            .field("highest_bidder", &self.highest_bidder)
            .field("highest_bid_suit", &self.highest_bid_suit)
            .field("coinched", &self.coinched)
            .field("completed_board", &self.completed_board)
            .field("auction", &self.auction)
            .field("announcements", &self.announcements)
            .field("all_trumps", &self.all_trumps)
            .finish_non_exhaustive()
    }
}

impl BiddingState {
    /// The dealer deals, then the seat to the dealer's left speaks first and leads the first trick.
    pub fn new(variant: Variant, dealer: Seat, rng: Box<dyn RandomSource + Send>) -> Self {
        let mut state = BiddingState {
            variant,
            rng,
            deals: 0,
            hands: Default::default(),
            remaining_deck: Vec::new(),
            upturned_card: None,
            dealer: dealer % 4,
            active: 0,
            round: 1,
            consecutive_passes: 0,
            highest_bid: 0,
            highest_bidder: None,
            highest_bid_suit: None,
            coinched: false,
            completed_board: None,
            auction: Vec::new(),
            announcements: false,
            all_trumps: false,
        };
        state.deal_again();
        state
    }

    /// Whether the board of this deal lets players declare announcements; off unless set.
    pub fn with_announcements(mut self, on: bool) -> Self {
        self.announcements = on;
        self
    }

    pub fn announcements(&self) -> bool {
        self.announcements
    }

    /// Whether Contrée bids may name all trumps or no trumps (US-063); off unless set.
    pub fn with_all_trumps(mut self, on: bool) -> Self {
        self.all_trumps = on;
        self
    }

    pub fn all_trumps(&self) -> bool {
        self.all_trumps
    }

    pub fn pass(&mut self, seat: Seat) -> Result<(), GameError> {
        self.require_active(seat)?;
        self.record_call(seat, CallType::Pass, 0, None);
        if self.variant == Variant::Contree {
            self.pass_contree();
            return Ok(());
        }
        self.consecutive_passes += 1;
        self.active = (self.active + 1) % 4;
        if self.consecutive_passes != 4 {
            return Ok(());
        }
        if self.round == 1 {
            self.round = 2;
            self.consecutive_passes = 0;
            self.active = self.first_bidder();
            return Ok(());
        }
        self.dealer = (self.dealer + 1) % 4;
        self.deal_again();
        Ok(())
    }

    pub fn choose_trump(&mut self, seat: Seat, trump: Suit) -> Result<(), GameError> {
        if self.variant != Variant::Classic {
            return Err(GameError::TrumpChoiceOnlyInClassic);
        }
        self.require_active(seat)?;
        let upturned = self
            .upturned_card
            .expect("Classic deals an upturned card")
            .suit;
        if self.round == 1 && trump != upturned {
            return Err(GameError::MustTakeUpturnedSuit);
        }
        if self.round == 2 && trump == upturned {
            return Err(GameError::MustChooseAnotherSuit);
        }
        self.record_call(seat, CallType::Trump, 0, Some(trump.into()));
        let mut hands = self.hands.clone();
        for (index, hand) in hands.iter_mut().enumerate() {
            let extra = if index == self.active { 2 } else { 3 };
            hand.extend(self.remaining_deck.drain(..extra));
        }
        hands[seat].push(self.upturned_card.expect("Classic deals an upturned card"));
        self.completed_board = Some(GameBoard::from_bidding(
            hands,
            trump,
            self.active,
            self.dealer,
            self.announcements,
        ));
        Ok(())
    }

    /// A Contrée bid. It must be higher than the highest bid; a contract of all trumps or no trumps
    /// at the same value does not overcall a suit (FFB §4.1 b).
    pub fn bid(&mut self, seat: Seat, value: i32, trump: Trump) -> Result<(), GameError> {
        self.require_contree()?;
        self.require_active(seat)?;
        if !self.all_trumps && !trump.suit_contract() {
            return Err(GameError::AllTrumpsNotPlayed);
        }
        if !(80..=160).contains(&value) || value % 10 != 0 {
            return Err(GameError::InvalidBid);
        }
        if value <= self.highest_bid {
            return Err(GameError::BidTooLow);
        }
        self.highest_bid = value;
        self.highest_bid_suit = Some(trump);
        self.highest_bidder = Some(self.active);
        self.record_call(seat, CallType::Bid, value, Some(trump));
        self.consecutive_passes = 0;
        self.active = (self.active + 1) % 4;
        Ok(())
    }

    pub fn coinche(&mut self, seat: Seat) -> Result<(), GameError> {
        self.require_contree()?;
        self.require_active(seat)?;
        match self.highest_bidder {
            Some(bidder) if bidder % 2 != self.active % 2 => {}
            _ => return Err(GameError::CoincheNotAllowed),
        }
        self.coinched = true;
        self.record_call(seat, CallType::Coinche, 0, None);
        self.complete_contree_auction();
        Ok(())
    }

    /// Applies a bot's decision to the matching call.
    pub fn apply(&mut self, seat: Seat, decision: AuctionDecision) -> Result<(), GameError> {
        match decision {
            AuctionDecision::Pass => self.pass(seat),
            AuctionDecision::Bid { value, trump } => self.bid(seat, value, trump),
            AuctionDecision::Coinche => self.coinche(seat),
            AuctionDecision::Trump { suit } => self.choose_trump(seat, suit),
        }
    }

    /// What the given seat can see when making a call: its own hand, the upturned card and the auction.
    pub fn auction_view(&self, seat: Seat) -> AuctionView {
        AuctionView {
            seat,
            hand: self.hands[seat].clone(),
            upturned_card: self.upturned_card,
            round: self.round,
            variant: self.variant,
            dealer_index: self.dealer,
            auction: self.auction.clone(),
            highest_bid: self.highest_bid,
            highest_bid_suit: self.highest_bid_suit,
            highest_bidder_seat: self.highest_bidder,
            coinche_allowed: self.can_coinche(seat),
            announcements: self.announcements,
            all_trumps: self.all_trumps,
        }
    }

    pub fn active_seat(&self) -> Seat {
        self.active
    }

    pub fn dealer(&self) -> Seat {
        self.dealer
    }

    /// How many times the cards have been dealt, counting redeals after everyone passed.
    pub fn deals(&self) -> u32 {
        self.deals
    }

    pub fn round(&self) -> i32 {
        self.round
    }

    pub fn hand(&self, seat: Seat) -> &[Card] {
        &self.hands[seat]
    }

    pub fn upturned_card(&self) -> Option<Card> {
        self.upturned_card
    }

    pub fn highest_bid(&self) -> i32 {
        self.highest_bid
    }

    pub fn highest_bid_suit(&self) -> Option<Trump> {
        self.highest_bid_suit
    }

    pub fn highest_bidder(&self) -> Option<Seat> {
        self.highest_bidder
    }

    pub fn auction(&self) -> &[SeatCall] {
        &self.auction
    }

    pub fn can_coinche(&self, seat: Seat) -> bool {
        self.variant == Variant::Contree
            && self.completed_board.is_none()
            && self.active == seat
            && self
                .highest_bidder
                .is_some_and(|bidder| seat % 2 != bidder % 2)
    }

    pub fn completed_board(&self) -> Option<&GameBoard> {
        self.completed_board.as_ref()
    }

    pub fn into_board(self) -> Option<GameBoard> {
        self.completed_board
    }

    fn record_call(&mut self, seat: Seat, call: CallType, value: i32, trump: Option<Trump>) {
        self.auction.push(SeatCall {
            seat,
            r#type: call,
            value,
            trump,
        });
    }

    fn first_bidder(&self) -> Seat {
        (self.dealer + 1) % 4
    }

    fn deal_again(&mut self) {
        let mut deck = Card::deck();
        java_shuffle(&mut deck, &mut *self.rng);
        self.deals += 1;
        let per_seat = if self.variant == Variant::Contree {
            8
        } else {
            5
        };
        let mut rest = deck.split_off(per_seat * 4);
        for (seat, hand) in self.hands.iter_mut().enumerate() {
            *hand = deck[seat * per_seat..(seat + 1) * per_seat].to_vec();
        }
        self.upturned_card = if self.variant == Variant::Classic {
            Some(rest.remove(0))
        } else {
            None
        };
        self.remaining_deck = rest;
        self.active = self.first_bidder();
        self.round = 1;
        self.consecutive_passes = 0;
        self.highest_bid = 0;
        self.highest_bidder = None;
        self.highest_bid_suit = None;
        self.coinched = false;
        self.completed_board = None;
        self.auction.clear();
    }

    fn pass_contree(&mut self) {
        self.consecutive_passes += 1;
        if self.highest_bidder.is_some() && self.consecutive_passes == 3 {
            self.complete_contree_auction();
            return;
        }
        self.active = (self.active + 1) % 4;
        if self.highest_bidder.is_none() && self.consecutive_passes == 4 {
            self.dealer = (self.dealer + 1) % 4;
            self.deal_again();
        }
    }

    fn complete_contree_auction(&mut self) {
        let (Some(trump), Some(declarer)) = (self.highest_bid_suit, self.highest_bidder) else {
            return;
        };
        self.completed_board = Some(GameBoard::from_contract(
            self.hands.clone(),
            trump,
            declarer,
            self.highest_bid,
            self.coinched,
            self.dealer,
            self.announcements,
        ));
    }

    fn require_contree(&self) -> Result<(), GameError> {
        if self.variant == Variant::Contree {
            Ok(())
        } else {
            Err(GameError::ContractOnlyInContree)
        }
    }

    fn require_active(&self, seat: Seat) -> Result<(), GameError> {
        if self.completed_board.is_some() {
            return Err(GameError::AuctionEnded);
        }
        if self.active != seat {
            return Err(GameError::NotYourTurn);
        }
        Ok(())
    }
}
