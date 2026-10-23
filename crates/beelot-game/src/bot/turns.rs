//! Lets a strategy take the active seat's turn. Port of the Java `BotTurns`; the Java locks are
//! the `&mut` borrows here.

use super::BotStrategy;
use crate::bidding::BiddingState;
use crate::board::GameBoard;
use crate::error::GameError;

/// The active seat makes the strategy's call.
pub fn take_auction_turn(
    bidding: &mut BiddingState,
    strategy: &dyn BotStrategy,
) -> Result<(), GameError> {
    if bidding.completed_board().is_some() {
        return Err(GameError::AuctionEnded);
    }
    let seat = bidding.active_seat();
    let decision = strategy.decide_auction(&bidding.auction_view(seat));
    bidding.apply(seat, decision)
}

/// The active seat plays the strategy's card. Bots always declare their announcements (US-060).
/// Unlike Java, which fails inside the strategy, the strategy is not asked when the seat has no
/// legal card: the round has ended, or a completed trick is under review.
pub fn play_turn(board: &mut GameBoard, strategy: &dyn BotStrategy) -> Result<(), GameError> {
    if board.round_result().is_some() {
        return Err(GameError::RoundEnded);
    }
    let seat = board.active_seat();
    let view = board.play_view(seat);
    if view.legal_cards.is_empty() {
        return Err(GameError::IllegalCard);
    }
    board.play(seat, strategy.choose_card(&view), true)
}
