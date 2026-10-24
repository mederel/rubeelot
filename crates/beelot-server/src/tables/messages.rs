//! Every English string the tables send, copied verbatim from the Java source: the frontend translates them with
//! the English text as the key.

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
