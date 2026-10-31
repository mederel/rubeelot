//! Tables: who sits where, and the game played at each table.

pub mod actions;
pub mod codes;
pub mod finished;
pub mod hints;
pub mod matchmaking;
pub mod messages;
pub mod reactions;
pub mod registry;
pub mod seats;
pub mod table;

use beelot_game::RandomSource;

pub use actions::Action;
pub use hints::{BidHint, Hint};
pub use registry::{Access, TableHandle, TableRegistry};
pub use seats::{
    Connection, Lobby, Seat, SeatKind, Status, TableKind, local_seats, required_name, valid_target,
};
pub use table::{Actor, Outcome, RngFactory, Table, TableSetup};

/// Deals from the operating system's generator: production deals need no reproducibility.
#[derive(Debug, Clone, Copy, Default)]
pub struct OsRandom;

impl RandomSource for OsRandom {
    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "the bound must be positive");
        let bound = bound as u32;
        // Rejects the top values that would make the remainder uneven.
        let zone = u32::MAX - u32::MAX % bound;
        loop {
            let mut bytes = [0u8; 4];
            getrandom::fill(&mut bytes).expect("the operating system's random generator");
            let value = u32::from_ne_bytes(bytes);
            if value < zone {
                return (value % bound) as i32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_random_stays_within_its_bound_and_covers_it() {
        let mut rng = OsRandom;
        let mut seen = [false; 7];
        for _ in 0..1000 {
            let value = rng.next_int(7);
            assert!((0..7).contains(&value));
            seen[value as usize] = true;
        }
        assert!(seen.iter().all(|&hit| hit));
        assert_eq!(rng.next_int(1), 0);
    }
}
