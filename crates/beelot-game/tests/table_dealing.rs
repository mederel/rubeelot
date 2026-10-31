//! Dealing as at a real table (US-085): the cut, the packets, the trick piles and the gathered
//! deck. Seat 0 is North.

use std::collections::HashSet;
use std::panic;

use beelot_game::bidding::{BiddingState, DealPacket};
use beelot_game::board::GameBoard;
use beelot_game::bot::rule_based::RuleBasedStrategy;
use beelot_game::bot::turns::{play_turn, take_auction_turn};
use beelot_game::deck::{self, Deck, Gathering};
use beelot_game::{Card, SplitMix64, Trump, Variant, rules};

fn ordered() -> Vec<Card> {
    Card::deck()
}

fn ordered_deck() -> Deck {
    Deck::gathered_hands(ordered())
}

fn concat(top: &[Card], bottom: &[Card]) -> Vec<Card> {
    let mut cards = top.to_vec();
    cards.extend_from_slice(bottom);
    cards
}

fn reversed(pile: &[Card]) -> Vec<Card> {
    pile.iter().rev().copied().collect()
}

fn picks(deck: &[Card], positions: &[usize]) -> Vec<Card> {
    positions.iter().map(|&position| deck[position]).collect()
}

fn packet(seat: usize, cards: usize) -> DealPacket {
    DealPacket { seat, cards }
}

fn table(variant: Variant, dealer: usize, seed: i64, deck: Option<Deck>) -> BiddingState {
    BiddingState::dealt_from(variant, dealer, Box::new(SplitMix64::new(seed)), deck)
}

/// The cut the deal was dealt from.
fn cut_of(deck: &Deck, bidding: &BiddingState) -> Vec<Card> {
    deck.cut_at(bidding.deal_summary().cut_lifted).cards
}

fn play_out(mut board: GameBoard) -> GameBoard {
    let rules = RuleBasedStrategy::new();
    while board.round_result().is_none() {
        if board.reviewing_completed_trick() {
            board.continue_after_trick().unwrap();
        } else {
            play_turn(&mut board, &rules).unwrap();
        }
    }
    board
}

/// A deal from a shuffled deck that someone bids on, played out by the bots.
fn played_deal(variant: Variant, seed: i64) -> GameBoard {
    let rules = RuleBasedStrategy::new();
    for attempt in seed.. {
        let mut bidding = BiddingState::new(variant, 3, Box::new(SplitMix64::new(attempt)));
        while bidding.completed_board().is_none() && bidding.deals() == 1 {
            take_auction_turn(&mut bidding, &rules).unwrap();
        }
        if let Some(board) = bidding.into_board() {
            return play_out(board);
        }
    }
    unreachable!()
}

fn assert_every_card_once(board: &GameBoard) {
    let view = board.play_view(0);
    let mut all: Vec<Card> = (0..4).flat_map(|seat| board.play_view(seat).hand).collect();
    all.extend(view.tricks.iter().flatten().map(|played| played.card));
    all.extend(view.current_trick.iter().map(|played| played.card));
    assert_eq!(all.len(), 32);
    assert_eq!(
        all.into_iter().collect::<HashSet<_>>(),
        ordered().into_iter().collect::<HashSet<_>>()
    );
}

// Deck

#[test]
fn a_trick_won_goes_face_down_on_top_of_the_pile_with_its_first_card_on_top() {
    let cards = ordered();
    let (first, second) = (&cards[0..4], &cards[4..8]);

    let pile = deck::pile_trick(&deck::pile_trick(&[], first), second);

    assert_eq!(pile, concat(second, first));
}

#[test]
fn counting_a_pile_turns_its_cards_over_one_by_one_so_it_ends_up_reversed() {
    let cards = ordered();

    assert_eq!(deck::count(&cards[..12]), reversed(&cards[..12]));
    assert_eq!(deck::count(&[]), Vec::<Card>::new());
}

#[test]
fn gathering_counts_both_piles_then_puts_one_on_the_other() {
    let cards = ordered();
    let (north_south, east_west) = (&cards[..20], &cards[20..]);

    let north_south_on_top = Deck::gathered_piles(north_south, east_west, true);
    let east_west_on_top = Deck::gathered_piles(north_south, east_west, false);

    assert_eq!(
        north_south_on_top.cards(),
        concat(&reversed(north_south), &reversed(east_west))
    );
    assert_eq!(
        east_west_on_top.cards(),
        concat(&reversed(east_west), &reversed(north_south))
    );
    assert_eq!(north_south_on_top.gathering(), Gathering::Tricks);
    assert_eq!(north_south_on_top.north_south_pile(), 20);
    assert_eq!(north_south_on_top.east_west_pile(), 12);
    assert!(north_south_on_top.north_south_on_top());
}

#[test]
fn a_capot_leaves_one_pile_empty() {
    let cards = ordered();

    let deck = Deck::gathered_piles(&[], &cards, true);

    assert_eq!(deck.cards(), reversed(&cards));
    assert_eq!(deck.north_south_pile(), 0);
}

#[test]
fn the_cut_puts_the_bottom_part_on_top() {
    let cards = ordered();

    let cut = ordered_deck().cut_at(10);

    assert_eq!(cut.lifted, 10);
    assert_eq!(cut.cards, concat(&cards[10..], &cards[..10]));
}

#[test]
fn each_part_of_a_cut_keeps_at_least_three_cards() {
    let deck = ordered_deck();

    for lifted in [2, 30] {
        let result = panic::catch_unwind(|| deck.cut_at(lifted));
        assert!(result.is_err(), "a cut lifting {lifted} cards");
    }
    assert_eq!(deck.cut_at(3).lifted, 3);
    assert_eq!(deck.cut_at(29).lifted, 29);
}

#[test]
fn a_random_cut_stays_within_the_limits_and_reaches_every_point() {
    let deck = ordered_deck();
    let mut random = SplitMix64::new(3);
    let mut points = HashSet::new();

    for _ in 0..2000 {
        let cut = deck.cut(&mut random);
        assert!((3..=29).contains(&cut.lifted), "cut at {}", cut.lifted);
        assert_eq!(deck.cut_at(cut.lifted).cards, cut.cards);
        points.insert(cut.lifted);
    }

    assert_eq!(points.len(), 27);
}

#[test]
fn the_same_seed_cuts_at_the_same_point() {
    let deck = ordered_deck();

    assert_eq!(
        deck.cut(&mut SplitMix64::new(11)),
        deck.cut(&mut SplitMix64::new(11))
    );
}

#[test]
fn a_shuffled_deck_holds_every_card_once_and_depends_on_the_seed() {
    let first = Deck::shuffled(&mut SplitMix64::new(5));

    assert_eq!(
        first.cards().iter().copied().collect::<HashSet<_>>(),
        ordered().into_iter().collect::<HashSet<_>>()
    );
    assert_eq!(first.gathering(), Gathering::Shuffled);
    assert_eq!(first, Deck::shuffled(&mut SplitMix64::new(5)));
    assert_ne!(first, Deck::shuffled(&mut SplitMix64::new(6)));
}

#[test]
fn a_deck_holds_the_thirty_two_cards_once_each() {
    let cards = ordered();
    let mut duplicated = cards.clone();
    duplicated[31] = duplicated[0];

    assert!(panic::catch_unwind(|| Deck::gathered_hands(duplicated)).is_err());
    assert!(panic::catch_unwind(|| Deck::gathered_hands(cards[..31].to_vec())).is_err());
    assert!(panic::catch_unwind(|| Deck::gathered_piles(&cards[..4], &cards[..28], true)).is_err());
}

#[test]
fn a_deck_read_from_json_holds_the_thirty_two_cards_once_each() {
    let deck = ordered_deck();
    let json = serde_json::to_value(&deck).unwrap();
    assert_eq!(json["gathering"], "HANDS");
    assert_eq!(serde_json::from_value::<Deck>(json.clone()).unwrap(), deck);

    let mut short = json;
    short["cards"].as_array_mut().unwrap().pop();
    assert!(serde_json::from_value::<Deck>(short).is_err());
}

// Dealing

#[test]
fn contree_deals_three_two_three_from_the_player_after_the_dealer() {
    let bidding = table(Variant::Contree, 3, 1, Some(ordered_deck()));
    let cut = cut_of(&ordered_deck(), &bidding);

    assert_eq!(bidding.hand(0), picks(&cut, &[0, 1, 2, 12, 13, 20, 21, 22]));
    assert_eq!(bidding.hand(1), picks(&cut, &[3, 4, 5, 14, 15, 23, 24, 25]));
    assert_eq!(bidding.hand(2), picks(&cut, &[6, 7, 8, 16, 17, 26, 27, 28]));
    assert_eq!(
        bidding.hand(3),
        picks(&cut, &[9, 10, 11, 18, 19, 29, 30, 31])
    );
    let expected: Vec<DealPacket> = [3, 2, 3]
        .into_iter()
        .flat_map(|size| (0..4).map(move |seat| packet(seat, size)))
        .collect();
    assert_eq!(bidding.deal_summary().packets, expected);
    assert_eq!(bidding.auction_view(0).upturned_card, None);
}

#[test]
fn the_player_before_the_dealer_cuts() {
    let bidding = table(Variant::Contree, 1, 1, Some(ordered_deck()));
    let deal = bidding.deal_summary();

    assert_eq!(deal.dealer_seat, 1);
    assert_eq!(deal.cutter_seat, 0);
    assert_eq!(
        deal.packets[..4]
            .iter()
            .map(|packet| packet.seat)
            .collect::<Vec<_>>(),
        [2, 3, 0, 1]
    );
}

#[test]
fn classic_deals_three_two_then_turns_up_the_next_card() {
    let bidding = table(Variant::Classic, 3, 1, Some(ordered_deck()));
    let cut = cut_of(&ordered_deck(), &bidding);

    assert_eq!(bidding.hand(0), picks(&cut, &[0, 1, 2, 12, 13]));
    assert_eq!(bidding.hand(1), picks(&cut, &[3, 4, 5, 14, 15]));
    assert_eq!(bidding.hand(2), picks(&cut, &[6, 7, 8, 16, 17]));
    assert_eq!(bidding.hand(3), picks(&cut, &[9, 10, 11, 18, 19]));
    assert_eq!(bidding.auction_view(0).upturned_card, Some(cut[20]));
    assert_eq!(bidding.deal_summary().packets.len(), 8);
}

#[test]
fn after_the_auction_the_taker_gets_the_turned_card_and_two_more_and_the_others_three() {
    let mut bidding = table(Variant::Classic, 3, 1, Some(ordered_deck()));
    let cut = cut_of(&ordered_deck(), &bidding);
    bidding.pass(0).unwrap();

    bidding.choose_trump(1, cut[20].suit).unwrap();
    let board = bidding.into_board().unwrap();

    assert_eq!(
        board.play_view(0).hand,
        picks(&cut, &[0, 1, 2, 12, 13, 21, 22, 23])
    );
    assert_eq!(
        board.play_view(1).hand,
        picks(&cut, &[3, 4, 5, 14, 15, 20, 24, 25])
    );
    assert_eq!(
        board.play_view(2).hand,
        picks(&cut, &[6, 7, 8, 16, 17, 26, 27, 28])
    );
    assert_eq!(
        board.play_view(3).hand,
        picks(&cut, &[9, 10, 11, 18, 19, 29, 30, 31])
    );
    assert_every_card_once(&board);
}

#[test]
fn the_first_deal_of_a_match_is_shuffled_then_cut() {
    let mut random = SplitMix64::new(9);
    let shuffled = Deck::shuffled(&mut random);
    let cut = shuffled.cut(&mut random);

    let bidding = BiddingState::new(Variant::Contree, 3, Box::new(SplitMix64::new(9)));
    let deal = bidding.deal_summary();

    assert_eq!(deal.gathering, Gathering::Shuffled);
    assert_eq!(deal.cut_lifted, cut.lifted);
    assert_eq!(
        bidding.hand(0),
        picks(&cut.cards, &[0, 1, 2, 12, 13, 20, 21, 22])
    );
}

#[test]
fn the_tricks_pile_up_per_team_with_the_latest_trick_on_top() {
    let board = played_deal(Variant::Contree, 7);
    let view = board.play_view(0);
    let mut north_south = Vec::new();
    let mut east_west = Vec::new();
    for trick in &view.tricks {
        let cards: Vec<Card> = trick.iter().map(|played| played.card).collect();
        let winner = trick[rules::winning_index(&cards, view.trump)].seat;
        let pile: &mut Vec<Card> = if winner % 2 == 0 {
            &mut north_south
        } else {
            &mut east_west
        };
        pile.splice(0..0, cards);
    }

    assert_eq!(board.trick_pile(true), north_south);
    assert_eq!(board.trick_pile(false), east_west);
}

#[test]
fn the_next_dealers_team_puts_its_counted_pile_on_top() {
    let board = played_deal(Variant::Contree, 7);
    let (north_south, east_west) = (board.trick_pile(true), board.trick_pile(false));

    assert_eq!(
        board.gathered_deck(0),
        Some(Deck::gathered_piles(north_south, east_west, true))
    );
    assert_eq!(
        board.gathered_deck(1),
        Some(Deck::gathered_piles(north_south, east_west, false))
    );
}

#[test]
fn the_cards_are_only_gathered_once_the_deal_is_played() {
    let mut bidding = table(Variant::Contree, 3, 1, Some(ordered_deck()));
    bidding.bid(0, 80, Trump::Hearts).unwrap();
    for seat in 1..4 {
        bidding.pass(seat).unwrap();
    }

    assert_eq!(bidding.completed_board().unwrap().gathered_deck(0), None);
}

#[test]
fn the_next_deal_comes_from_the_previous_deals_cards_without_a_shuffle() {
    let board = played_deal(Variant::Classic, 7);
    let gathered = board.gathered_deck(0).unwrap();

    let next = table(Variant::Classic, 0, 4, Some(gathered.clone()));
    let deal = next.deal_summary();
    let cut = cut_of(&gathered, &next);

    assert_eq!(deal.gathering, Gathering::Tricks);
    assert_eq!(deal.cutter_seat, 3);
    assert_eq!(next.hand(1), picks(&cut, &[0, 1, 2, 12, 13]));
    assert_eq!(next.hand(0), picks(&cut, &[9, 10, 11, 18, 19]));
    assert_eq!(deal.north_south_pile, gathered.north_south_pile());
    assert_eq!(deal.east_west_pile, gathered.east_west_pile());
    assert_eq!(gathered.north_south_pile() + gathered.east_west_pile(), 32);
}

#[test]
fn when_everyone_passes_the_hands_are_gathered_in_play_order_cut_and_the_next_dealer_deals() {
    let mut bidding = table(Variant::Classic, 3, 2, Some(ordered_deck()));
    let previous_cut = cut_of(&ordered_deck(), &bidding);
    let mut gathered: Vec<Card> = (0..4)
        .flat_map(|seat| bidding.hand(seat).to_vec())
        .collect();
    gathered.push(bidding.auction_view(0).upturned_card.unwrap());
    gathered.extend_from_slice(&previous_cut[21..]);

    for _ in 0..8 {
        bidding.pass(bidding.active_seat()).unwrap();
    }

    let deal = bidding.deal_summary();
    let cut = Deck::gathered_hands(gathered).cut_at(deal.cut_lifted).cards;
    assert_eq!(deal.gathering, Gathering::Hands);
    assert_eq!(deal.dealer_seat, 0);
    assert_eq!(deal.cutter_seat, 3);
    assert_eq!(bidding.hand(1), picks(&cut, &[0, 1, 2, 12, 13]));
    assert_eq!(bidding.auction_view(0).upturned_card, Some(cut[20]));
}

#[test]
fn when_everyone_passes_in_contree_the_hands_are_gathered_from_the_player_after_the_dealer() {
    let mut bidding = table(Variant::Contree, 1, 2, Some(ordered_deck()));
    let gathered: Vec<Card> = (1..=4)
        .flat_map(|offset| bidding.hand((1 + offset) % 4).to_vec())
        .collect();

    for _ in 0..4 {
        bidding.pass(bidding.active_seat()).unwrap();
    }

    let deal = bidding.deal_summary();
    let cut = Deck::gathered_hands(gathered).cut_at(deal.cut_lifted).cards;
    assert_eq!(deal.dealer_seat, 2);
    assert_eq!(deal.cutter_seat, 1);
    assert_eq!(bidding.hand(3), picks(&cut, &[0, 1, 2, 12, 13, 20, 21, 22]));
}

#[test]
fn the_same_seed_deals_the_same_cards_from_the_same_deck() {
    let gathered = played_deal(Variant::Contree, 7).gathered_deck(0).unwrap();

    let first = table(Variant::Contree, 0, 8, Some(gathered.clone()));
    let second = table(Variant::Contree, 0, 8, Some(gathered));

    for seat in 0..4 {
        assert_eq!(first.hand(seat), second.hand(seat));
    }
}

#[test]
fn several_deals_in_a_row_keep_every_card_once() {
    let rules = RuleBasedStrategy::new();
    let mut random = SplitMix64::new(3);
    let mut deck = Deck::shuffled(&mut random);
    for round in 0..12 {
        let variant = if round % 2 == 0 {
            Variant::Classic
        } else {
            Variant::Contree
        };
        // A fresh generator per deal: the Rust state owns its generator.
        let mut bidding = table(variant, round % 4, 3 + round as i64, Some(deck));
        while bidding.completed_board().is_none() {
            take_auction_turn(&mut bidding, &rules).unwrap();
        }
        let next_dealer = (bidding.dealer() + 1) % 4;
        let board = play_out(bidding.into_board().unwrap());
        assert_every_card_once(&board);
        deck = board.gathered_deck(next_dealer).unwrap();
    }
}
