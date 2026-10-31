//! What a hint answers (US-065): the card or the call the Challenging bot would choose, and why.

use beelot_game::bot::hints::{AuctionReason, CardReason};
use beelot_game::{Suit, Trump};
use serde::Serialize;

use crate::api::snapshot::CardView;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Hint {
    Card {
        card: CardView,
        reason: CardReason,
    },
    Bid {
        action: BidHint,
        reason: AuctionReason,
    },
}

/// The call as the action the client would send to make it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BidHint {
    Pass,
    Trump { suit: Suit },
    Contract { value: i32, suit: Trump },
    Coinche,
}
