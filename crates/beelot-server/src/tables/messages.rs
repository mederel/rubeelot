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
