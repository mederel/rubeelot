//! Bot players. A strategy only receives what its seat can see, so it cannot look at the other
//! hands, and strategies can be swapped without touching the rules engine.

pub mod arena;
pub mod hand_features;
pub mod hints;
pub(crate) mod memory;
pub mod rule_based;
pub mod sampling;
pub mod solver;
pub mod strategies;
pub mod turns;

use crate::bidding::{AuctionDecision, AuctionView};
use crate::board::PlayView;
use crate::card::Card;

/// How a bot bids and plays.
pub trait BotStrategy: Send + Sync {
    fn decide_auction(&self, view: &AuctionView) -> AuctionDecision;

    /// Returns one of the view's legal cards.
    fn choose_card(&self, view: &PlayView) -> Card;
}
