//! The strategy each difficulty level plays: Relaxed bots follow the hand-written rules, and
//! Challenging bots search sampled deals before playing a card (US-056).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::LazyLock;

use super::BotStrategy;
use super::rule_based::RuleBasedStrategy;
use super::sampling::SamplingStrategy;
use crate::card::Difficulty;

static RELAXED: LazyLock<RuleBasedStrategy> = LazyLock::new(RuleBasedStrategy::new);
static CHALLENGING: LazyLock<SamplingStrategy> =
    LazyLock::new(|| SamplingStrategy::new(random_seed()));

/// A seed drawn from the operating system: the standard library keys each `RandomState` with
/// random bits, so hashing nothing with it gives a random number.
fn random_seed() -> i64 {
    RandomState::new().build_hasher().finish() as i64
}

pub fn for_difficulty(difficulty: Difficulty) -> &'static dyn BotStrategy {
    match difficulty {
        Difficulty::Relaxed => &*RELAXED,
        Difficulty::Challenging => &*CHALLENGING,
    }
}

/// Bots at private and public tables play at the Challenging level.
pub fn for_tables() -> &'static dyn BotStrategy {
    for_difficulty(Difficulty::Challenging)
}

#[cfg(test)]
mod tests {
    //! Tests of the difficulty-to-strategy mapping.

    use std::ptr;

    use super::*;

    #[test]
    fn each_difficulty_has_its_own_strategy() {
        assert!(!ptr::addr_eq(
            for_difficulty(Difficulty::Relaxed),
            for_difficulty(Difficulty::Challenging)
        ));
    }

    #[test]
    fn relaxed_bots_follow_the_rules_and_challenging_bots_search() {
        let relaxed: &dyn BotStrategy = &*RELAXED;
        let challenging: &dyn BotStrategy = &*CHALLENGING;

        assert!(ptr::addr_eq(for_difficulty(Difficulty::Relaxed), relaxed));
        assert!(ptr::addr_eq(
            for_difficulty(Difficulty::Challenging),
            challenging
        ));
    }

    #[test]
    fn tables_use_the_challenging_strategy() {
        assert!(ptr::addr_eq(
            for_tables(),
            for_difficulty(Difficulty::Challenging)
        ));
    }
}
