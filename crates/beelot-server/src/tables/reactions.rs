//! The preset reactions players send at an online table.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TableReaction {
    WellPlayed,
    NiceHand,
    Oops,
    Wow,
    Thinking,
    GoodGame,
}
