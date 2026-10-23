//! The bot arena: plays two arena bots against each other on seeded duplicate deals and reports
//! how they did. Port of the Java `BotArena` and `ArenaReport`.

pub mod arena;
pub mod java_format;
pub mod report;

pub use arena::BotArena;
pub use report::{ArenaReport, Rate, Timing};
