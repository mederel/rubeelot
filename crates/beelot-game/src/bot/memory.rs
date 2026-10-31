//! What a bot remembers of the cards played so far, from its seat's view: the cards it has not
//! seen, which cards are masters, how many trumps remain, which suits each seat has shown to be
//! void in, and which cards the other seats revealed with their announcements and still hold
//! (US-061).
//!
//! The cards are kept in bit sets indexed by suit then rank. Every
//! query only tests membership or counts, so the order of a set never matters, and the lists
//! (`unseen_cards`, `known_cards`) come out by suit, then rank.

use crate::board::{PlayView, SeatCard};
use crate::card::{Card, Seat, Suit, Trump};
use crate::rules;

/// A set of cards, one bit per card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CardSet(u32);

impl CardSet {
    fn bit(card: Card) -> u32 {
        1 << (card.suit.ordinal() * 8 + card.rank as usize)
    }

    fn insert(&mut self, card: Card) {
        self.0 |= Self::bit(card);
    }

    fn remove(&mut self, card: Card) {
        self.0 &= !Self::bit(card);
    }

    fn contains(self, card: Card) -> bool {
        self.0 & Self::bit(card) != 0
    }

    /// The cards of the set, by suit then rank.
    fn cards(self) -> impl Iterator<Item = Card> {
        Card::deck()
            .into_iter()
            .filter(move |&card| self.contains(card))
    }
}

pub(crate) struct CardMemory {
    trump: Trump,
    unseen: CardSet,
    /// Per seat, the suits it has shown to be void in, by suit ordinal.
    voids: [[bool; 4]; 4],
    known: [CardSet; 4],
}

impl CardMemory {
    pub(crate) fn new(view: &PlayView) -> Self {
        let mut unseen = CardSet::default();
        for card in Card::deck() {
            unseen.insert(card);
        }
        for &card in &view.hand {
            unseen.remove(card);
        }
        let mut known = [CardSet::default(); 4];
        for declaration in &view.revealed_announcements {
            if declaration.seat == view.seat {
                continue;
            }
            for announcement in &declaration.announcements {
                for &card in &announcement.cards {
                    known[declaration.seat].insert(card);
                }
            }
        }
        let mut memory = CardMemory {
            trump: view.trump,
            unseen,
            voids: [[false; 4]; 4],
            known,
        };
        for trick in &view.tricks {
            memory.remember(trick);
        }
        if !view.current_trick.is_empty() {
            memory.remember(&view.current_trick);
        }
        memory
    }

    /// A seat that does not follow the suit led is void in it. In a suit contract, a seat that
    /// neither follows nor trumps, while the opponents are winning the trick, is also void in
    /// trumps, since the rules would have forced a trump. Under all trumps and no trumps nobody is
    /// forced to cut, so a discard shows nothing more.
    fn remember(&mut self, trick: &[SeatCard]) {
        let lead = trick[0].card.suit;
        let mut played: Vec<Card> = Vec::with_capacity(trick.len());
        for seat_card in trick {
            let card = seat_card.card;
            self.unseen.remove(card);
            self.known[seat_card.seat].remove(card);
            if card.suit != lead {
                self.voids[seat_card.seat][lead.ordinal()] = true;
                // The partner played two cards back: the second card of a trick has none yet.
                let partner_winning = played.len() >= 2
                    && rules::winning_index(&played, self.trump) == played.len() - 2;
                if let Some(trump_suit) = self.trump.suit()
                    && card.suit != trump_suit
                    && !partner_winning
                {
                    self.voids[seat_card.seat][trump_suit.ordinal()] = true;
                }
            }
            played.push(card);
        }
    }

    /// Whether the card is neither in the bot's hand nor played yet, so another seat may hold it.
    pub(crate) fn unseen(&self, card: Card) -> bool {
        self.unseen.contains(card)
    }

    /// The cards neither in the bot's hand nor played yet, by suit, then by rank, in a fixed
    /// order: the sampling bot shuffles this list.
    pub(crate) fn unseen_cards(&self) -> Vec<Card> {
        self.unseen.cards().collect()
    }

    /// Unseen cards of the suit.
    pub(crate) fn unseen_count(&self, suit: Suit) -> i32 {
        self.unseen.cards().filter(|card| card.suit == suit).count() as i32
    }

    /// Trumps still held by the other seats: every unseen card under all trumps, none under no
    /// trumps.
    #[cfg(test)]
    pub(crate) fn remaining_trumps(&self) -> i32 {
        self.unseen
            .cards()
            .filter(|card| self.trump.is_trump(card.suit))
            .count() as i32
    }

    /// Whether the seat may still hold a trump: one in a suit it has not shown to be void in.
    pub(crate) fn may_hold_trump(&self, seat: Seat) -> bool {
        self.unseen
            .cards()
            .any(|card| self.trump.is_trump(card.suit) && !self.shown_void(seat, card.suit))
    }

    /// Whether no unseen card of the card's suit beats it.
    pub(crate) fn master(&self, card: Card) -> bool {
        let strength = rules::strength(card, self.trump);
        !self
            .unseen
            .cards()
            .any(|other| other.suit == card.suit && rules::strength(other, self.trump) > strength)
    }

    /// The cards the seat revealed with an announcement and has not played yet, by suit then
    /// rank; none for the bot. The sampling bot only adds these
    /// cards to hands and removes them from the unseen cards, so their order never matters.
    pub(crate) fn known_cards(&self, seat: Seat) -> Vec<Card> {
        self.known[seat].cards().collect()
    }

    /// Whether the card is known to be in the hand of a seat other than the given one.
    pub(crate) fn known_elsewhere(&self, card: Card, seat: Seat) -> bool {
        (0..4).any(|other| other != seat && self.known[other].contains(card))
    }

    pub(crate) fn shown_void(&self, seat: Seat, suit: Suit) -> bool {
        self.voids[seat][suit.ordinal()]
    }

    /// Whether the seat may still hold an unseen card that beats the given card on a trick led in
    /// the given suit. A seat is only expected to trump once shown void in the suit led, or once
    /// no card of it is unseen: fearing a trump earlier made bots lose points in the arena. A card
    /// known to be in another hand doesn't count.
    pub(crate) fn may_beat(&self, seat: Seat, winner: Card, lead: Suit) -> bool {
        let may_trump = self.trump.suit().is_some_and(|trump_suit| {
            winner.suit != trump_suit
                && !self.shown_void(seat, trump_suit)
                && self
                    .unseen
                    .cards()
                    .any(|other| other.suit == trump_suit && !self.known_elsewhere(other, seat))
        }) && (self.shown_void(seat, lead) || self.unseen_count(lead) == 0);
        if may_trump {
            return true;
        }
        let strength = rules::strength(winner, self.trump);
        !self.shown_void(seat, winner.suit)
            && self.unseen.cards().any(|other| {
                other.suit == winner.suit
                    && !self.known_elsewhere(other, seat)
                    && rules::strength(other, self.trump) > strength
            })
    }
}

#[cfg(test)]
mod tests {
    //! Hearts are trump; the bot sits at seat 0 and seats 1
    //! and 3 are its opponents.

    use super::*;
    use crate::announcements::{Announcement, AnnouncementKind};
    use crate::board::{PlayView, SeatCard};
    use crate::card::{Card, Seat, Suit, Variant};
    use crate::scoring::Declaration;

    fn card(code: &str) -> Card {
        code.parse().unwrap()
    }

    fn cards(codes: &str) -> Vec<Card> {
        codes.split_whitespace().map(card).collect()
    }

    /// A completed trick led from the given seat.
    fn trick(leader: Seat, codes: &str) -> Vec<SeatCard> {
        cards(codes)
            .into_iter()
            .enumerate()
            .map(|(index, card)| SeatCard {
                seat: (leader + index) % 4,
                card,
            })
            .collect()
    }

    fn memory(hand: &str, tricks: Vec<Vec<SeatCard>>) -> CardMemory {
        memory_revealed(hand, tricks, Vec::new())
    }

    fn memory_revealed(
        hand: &str,
        tricks: Vec<Vec<SeatCard>>,
        revealed: Vec<Declaration>,
    ) -> CardMemory {
        memory_in(Trump::Hearts, hand, tricks, revealed)
    }

    fn memory_in(
        trump: Trump,
        hand: &str,
        tricks: Vec<Vec<SeatCard>>,
        revealed: Vec<Declaration>,
    ) -> CardMemory {
        let hand = cards(hand);
        CardMemory::new(&PlayView {
            seat: 0,
            hand: hand.clone(),
            legal_cards: hand,
            trump,
            variant: Variant::Classic,
            contract_value: 0,
            coinched: false,
            declaring_seat: 0,
            tricks,
            current_trick: Vec::new(),
            announcements: true,
            revealed_announcements: revealed,
        })
    }

    /// The given seat declared a tierce to the king of spades.
    fn spade_tierce(seat: Seat) -> Declaration {
        Declaration {
            seat,
            announcements: vec![Announcement {
                kind: AnnouncementKind::Tierce,
                cards: cards("JS QS KS"),
                value: 20,
                suit: Some(Suit::Spades),
            }],
            scored: true,
        }
    }

    #[test]
    fn revealed_cards_are_known_until_played() {
        let memory = memory_revealed("7C", Vec::new(), vec![spade_tierce(1)]);

        assert_eq!(memory.known_cards(1), cards("JS QS KS"));
        assert!(memory.known_elsewhere(card("KS"), 3));
        assert!(!memory.known_elsewhere(card("KS"), 1));
    }

    #[test]
    fn played_revealed_card_is_forgotten() {
        let memory = memory_revealed("7C", vec![trick(1, "JS 7S 8S 9S")], vec![spade_tierce(1)]);

        assert_eq!(memory.known_cards(1), cards("QS KS"));
    }

    #[test]
    fn own_revealed_cards_are_not_known_elsewhere() {
        let memory = memory_revealed("JS QS KS", Vec::new(), vec![spade_tierce(0)]);

        assert!(memory.known_cards(0).is_empty());
        assert!(!memory.known_elsewhere(card("KS"), 1));
    }

    #[test]
    fn a_card_held_by_the_partner_cannot_beat_for_an_opponent() {
        let tricks = || vec![trick(1, "AS 10S 7S 8S")];
        let kings = Announcement {
            kind: AnnouncementKind::Carre,
            cards: cards("KS KH KC KD"),
            value: 100,
            suit: None,
        };
        let partner = || Declaration {
            seat: 2,
            announcements: vec![kings.clone()],
            scored: true,
        };

        assert!(
            !memory_revealed("QS 7C", tricks(), vec![partner()]).may_beat(
                1,
                card("QS"),
                Suit::Spades
            )
        );
        assert!(
            !memory_revealed("QS 7C", tricks(), vec![partner()]).may_beat(
                3,
                card("QS"),
                Suit::Spades
            )
        );
        assert!(memory("QS 7C", tricks()).may_beat(1, card("QS"), Suit::Spades));
    }

    #[test]
    fn a_card_becomes_master_once_the_higher_cards_are_played() {
        assert!(!memory("10S 7C", Vec::new()).master(card("10S")));
        assert!(memory("10S 7C", vec![trick(1, "AS 7S 8S 9S")]).master(card("10S")));
    }

    #[test]
    fn the_jack_is_the_master_trump() {
        let memory = memory("JH AH", Vec::new());

        assert!(memory.master(card("JH")));
        assert!(!memory.master(card("AH")));
    }

    #[test]
    fn counts_the_trumps_the_other_players_may_hold() {
        let memory = memory("JH 7C", vec![trick(1, "9H 7H 8H QH")]);

        assert_eq!(memory.remaining_trumps(), 3);
    }

    #[test]
    fn a_player_who_does_not_follow_is_void_in_the_suit_led() {
        let memory = memory("", vec![trick(1, "7S 8H 9S 10S")]);

        assert!(memory.shown_void(2, Suit::Spades));
        assert!(!memory.shown_void(2, Suit::Hearts));
        assert!(!memory.shown_void(3, Suit::Spades));
    }

    #[test]
    fn a_player_who_discards_while_the_opponents_win_is_void_in_trumps() {
        let memory = memory("", vec![trick(1, "AS 7C 9S 10S")]);

        assert!(memory.shown_void(2, Suit::Spades));
        assert!(memory.shown_void(2, Suit::Hearts));
    }

    #[test]
    fn a_player_who_discards_on_its_partners_trick_may_still_hold_trumps() {
        let memory = memory("", vec![trick(1, "AS 7S 7C 10S")]);

        assert!(memory.shown_void(3, Suit::Spades));
        assert!(!memory.shown_void(3, Suit::Hearts));
    }

    #[test]
    fn an_opponent_void_in_the_suit_led_may_trump_it() {
        let void_in_spades = memory("AS", vec![trick(0, "7S 7H 8S 9S")]);
        let void_in_spades_and_trumps = memory("AS", vec![trick(0, "7S 7C 8S 9S")]);

        assert!(void_in_spades.may_beat(1, card("AS"), Suit::Spades));
        assert!(!void_in_spades.may_beat(3, card("AS"), Suit::Spades));
        assert!(!void_in_spades_and_trumps.may_beat(1, card("AS"), Suit::Spades));
    }

    #[test]
    fn unseen_cards_are_listed_by_suit_then_rank() {
        let memory = memory("7C", vec![trick(1, "AS 7S 8S 9S")]);
        let unseen = memory.unseen_cards();

        assert_eq!(unseen.len(), 32 - 5);
        assert_eq!(unseen[0], card("8C"));
        assert_eq!(*unseen.last().unwrap(), card("KS"));
        assert!(memory.unseen(card("AH")));
        assert!(!memory.unseen(card("7C")));
    }

    #[test]
    fn under_no_trumps_nobody_cuts_and_nobody_holds_a_trump() {
        let tricks = vec![trick(0, "7S 7H 8S 9S")];
        let memory = memory_in(Trump::NoTrumps, "AS", tricks, Vec::new());

        assert_eq!(memory.remaining_trumps(), 0);
        assert!(!memory.may_hold_trump(1));
        assert!(memory.shown_void(1, Suit::Spades));
        assert!(!memory.shown_void(1, Suit::Hearts));
        assert!(!memory.may_beat(1, card("AS"), Suit::Spades));
    }

    #[test]
    fn under_all_trumps_every_unseen_card_is_a_trump_and_only_void_suits_are_excluded() {
        let tricks = vec![trick(0, "7S 7H 8S 9S")];
        let memory = memory_in(Trump::AllTrumps, "AS", tricks, Vec::new());

        assert_eq!(memory.remaining_trumps(), 32 - 1 - 4);
        assert!(memory.may_hold_trump(1));
        assert!(!memory.shown_void(1, Suit::Hearts));
        assert!(memory.shown_void(1, Suit::Spades));
        assert!(!memory.may_beat(1, card("AS"), Suit::Spades));
    }
}
