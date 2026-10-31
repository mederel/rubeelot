//! Every English string the tables send: the frontend translates them with the English text as the key.

use beelot_game::Trump;
use beelot_game::Variant;
use beelot_game::bidding::{BiddingState, CallType, SeatCall};
use beelot_game::board::{BeloteDeclaration, GameBoard};

pub const TABLE_CLOSED: &str = "This table does not exist.";
pub const ALREADY_STARTED: &str = "This table has already started.";
pub const TABLE_FULL: &str = "This table is full.";
pub const NOT_SEATED: &str = "You are not seated at this table.";
pub const SEATS_ASSIGNED: &str = "Seats are assigned at public tables.";
pub const NO_SUCH_SEAT: &str = "That seat does not exist.";
pub const SEAT_TAKEN: &str = "That seat is taken.";
pub const OWNER_STARTS: &str = "Only the table owner can start the game.";
pub const FOUR_READY_PLAYERS: &str = "Four ready players are required to start the game.";
pub const READY_FOR_BOTS: &str = "Every seated player must be ready to start with bots.";
pub const OWNER_SETS_TIMER: &str = "Only the table owner can change the turn timer.";
pub const TIMER_LOCKED: &str = "The turn timer cannot change after the game starts.";
pub const TIMER_CHOICES: &str = "Choose no timer, 30 seconds, or 60 seconds.";
pub const PUBLIC_ANNOUNCEMENTS: &str = "Public tables always play with announcements.";
pub const OWNER_SETS_ANNOUNCEMENTS: &str = "Only the table owner can change announcements.";
pub const ANNOUNCEMENTS_LOCKED: &str = "Announcements cannot change after the game starts.";
pub const PUBLIC_ALL_TRUMPS: &str = "Public tables always play with tout-atout and sans-atout.";
pub const OWNER_SETS_ALL_TRUMPS: &str =
    "Only the table owner can change tout-atout and sans-atout.";
pub const ALL_TRUMPS_LOCKED: &str =
    "Tout-atout and sans-atout cannot change after the game starts.";
pub const ALL_TRUMPS_NOT_PLAYED: &str = "Tout-atout and sans-atout are not played at this table.";
pub const PUBLIC_TARGET: &str = "Public tables always play to 1,000 points.";
pub const OWNER_SETS_TARGET: &str = "Only the table owner can change the match target.";
pub const TARGET_LOCKED: &str = "The match target cannot change after the game starts.";
pub const TARGET_CHOICES: &str = "The match target must be 500, 1,000, 1,500 or 2,000 points.";
pub const BOT_TOOK_OVER: &str = "A bot has taken over this seat for the rest of the match.";

pub const NAME_REQUIRED: &str = "Enter a player name.";
pub const NAME_TOO_LONG: &str = "Player names are limited to 30 characters.";
pub const FOUR_SEATS: &str = "Fill in the four seats.";
pub const TWO_PLAYERS: &str = "Pass and play needs at least two players.";
pub const DIFFERENT_NAMES: &str = "Each player needs a different name.";

pub const NOT_YOUR_TURN_TO_BID: &str = "It is not your turn to bid.";
pub const NOT_YOUR_TURN_TO_PLAY: &str = "It is not your turn to play.";
/// At a local table, when a bot is to act.
pub const NOT_YOUR_TURN: &str = "It is not your turn.";
pub const NO_HINT: &str = "There is no hint to give now.";
pub const ILLEGAL_CARD: &str = "That card is not a legal play.";
pub const ROUND_ENDED: &str = "This round has ended.";
pub const NO_COMPLETED_TRICK: &str = "There is no completed trick to continue from.";
pub const CONTRACT_ONLY_IN_CONTREE: &str = "Contract bids are only available in Contrée.";
pub const TRUMP_CHOICE_ONLY_IN_CLASSIC: &str = "Choose a contract value and suit in Contrée.";
pub const AUCTION_ENDED: &str = "The auction has ended.";
pub const MUST_TAKE_UPTURNED_SUIT: &str =
    "In the first round, you may only accept the upturned suit.";
pub const MUST_CHOOSE_ANOTHER_SUIT: &str = "Choose a suit other than the upturned suit.";
pub const INVALID_BID: &str = "A Contrée bid must be from 80 to 160 in steps of 10.";
pub const BID_TOO_LOW: &str = "Your bid must be higher than the current contract.";
pub const COINCHE_NOT_ALLOWED: &str = "Only an opponent of the declaring team may coinche.";
pub const SUIT_REQUIRED: &str = "Choose a trump suit for the contract.";

pub const NOT_STARTED: &str = "This game has not started.";
pub const AUCTION_NOT_FINISHED: &str = "The auction has not finished.";
pub const ROUND_NOT_FINISHED: &str = "The current round is not finished.";
pub const MATCH_ENDED: &str = "This match has ended. Start a rematch.";
pub const REACTIONS_CLOSED: &str = "Reactions open once the game starts.";

/// A call as players read it: `Pass`, `90 Hearts`, `Hearts` for a chosen trump, or `Coinche!`.
pub fn call_label(call: &SeatCall) -> String {
    match call.r#type {
        CallType::Pass => "Pass".into(),
        CallType::Bid => format!(
            "{} {}",
            call.value,
            call.trump.map_or("", Trump::display_name)
        ),
        CallType::Trump => call.trump.map_or("", Trump::display_name).into(),
        CallType::Coinche => "Coinche!".into(),
    }
}

/// What the auction says now, after each call. Choosing a trump leaves the message
/// as it was.
pub fn auction_message(
    bidding: &BiddingState,
    variant: Variant,
    name: impl Fn(usize) -> String,
) -> String {
    let auction = bidding.auction();
    let Some(index) = auction
        .iter()
        .rposition(|call| call.r#type != CallType::Trump)
    else {
        return match (bidding.deals() > 1, variant) {
            (false, Variant::Contree) => {
                "Eight cards have been dealt. Bid from 80 to 160 or pass.".into()
            }
            (false, Variant::Classic) => {
                "Five cards have been dealt. Accept the upturned suit or pass.".into()
            }
            (true, Variant::Contree) => "Everyone passed. The cards have been redealt.".into(),
            (true, Variant::Classic) => {
                "Everyone passed twice. The cards have been redealt.".into()
            }
        };
    };
    let call = &auction[index];
    let deciding = || format!("{} is deciding.", name((call.seat + 1) % 4));
    match call.r#type {
        CallType::Bid => format!(
            "{} bids {} {}.",
            name(call.seat),
            call.value,
            call.trump.map_or("", Trump::display_name)
        ),
        CallType::Coinche => format!("{} coinches the contract.", name(call.seat)),
        CallType::Pass if variant == Variant::Classic && index == 3 => {
            let upturned = bidding
                .upturned_card()
                .expect("Classic Belote turns a card up");
            format!(
                "Everyone passed. Choose any trump suit except {}.",
                suit_name(upturned.suit)
            )
        }
        CallType::Pass if variant == Variant::Contree && bidding.completed_board().is_some() => {
            format!(
                "The {} {} contract is accepted.",
                bidding.highest_bid(),
                bidding
                    .highest_bid_suit()
                    .map_or("", |suit| suit.display_name())
            )
        }
        CallType::Pass | CallType::Trump => deciding(),
    }
}

/// The suit's upper-case name: `HEARTS`.
pub fn suit_name(suit: beelot_game::Suit) -> &'static str {
    match suit {
        beelot_game::Suit::Clubs => "CLUBS",
        beelot_game::Suit::Diamonds => "DIAMONDS",
        beelot_game::Suit::Hearts => "HEARTS",
        beelot_game::Suit::Spades => "SPADES",
    }
}

/// The trump's upper-case name: `HEARTS`, `ALL_TRUMPS` or `NO_TRUMPS`.
pub fn trump_name(trump: Trump) -> &'static str {
    match trump {
        Trump::AllTrumps => "ALL_TRUMPS",
        Trump::NoTrumps => "NO_TRUMPS",
        suit => suit_name(suit.suit().expect("a suit contract")),
    }
}

/// The latest Belote announcement of the round, once its first card is played.
pub fn declaration_message(board: &GameBoard, name: impl Fn(usize) -> String) -> String {
    match board.last_belote() {
        Some(BeloteDeclaration {
            seat,
            rebelote: false,
        }) => format!("{} declares Belote.", name(seat)),
        Some(BeloteDeclaration {
            seat,
            rebelote: true,
        }) => format!("{} declares Rebelote: 20 bonus points.", name(seat)),
        None => String::new(),
    }
}
pub const SLOW_DOWN: &str = "Slow down: wait a moment before reacting again.";
pub const INVALID_CODE: &str = "The invitation code is invalid.";
