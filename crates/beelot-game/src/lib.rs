pub mod announcements;
pub mod bidding;
pub mod board;
pub mod bot;
pub mod card;
pub mod error;
pub mod match_score;
pub mod rng;
pub mod rules;
pub mod scoring;

pub use card::{Card, Difficulty, Rank, Seat, Suit, Team, Trump, Variant};
pub use error::GameError;
pub use rng::{JavaRandom, RandomSource, SplitMix64, java_shuffle};

#[cfg(test)]
mod thread_safety {
    use super::{bidding::BiddingState, board::GameBoard};

    const _: fn() = || {
        fn assert_send<T: Send>() {}
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send::<BiddingState>();
        assert_send_sync::<GameBoard>();
    };
}
