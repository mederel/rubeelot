//! The bot arena: plays two arena bots against each other on seeded duplicate deals and reports
//! how they did, and fits the bots' all trumps and no trumps estimates. Port of the Java
//! `BotArena`, `ArenaReport` and `EstimateFit`.

pub mod arena;
pub mod fit;
pub mod report;
pub mod report_format;

pub use arena::{BotArena, Dealing};
pub use report::{ArenaReport, Rate, Timing};
