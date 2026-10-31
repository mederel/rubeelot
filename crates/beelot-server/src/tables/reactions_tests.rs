//! Tests of the table reactions.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use time::macros::datetime;
use time::{Duration, OffsetDateTime};

use super::*;
use crate::error::AppError;

const START: OffsetDateTime = datetime!(2026-10-06 20:00 UTC);

fn message(result: Result<u64, AppError>) -> String {
    match result.expect_err("a refusal") {
        AppError::Conflict { message, .. } => message,
        other => panic!("not a conflict: {other:?}"),
    }
}

#[test]
fn lists_the_reactions_of_the_table_in_the_order_they_were_sent() {
    let sequence = Arc::new(AtomicU64::new(0));
    let mut reactions = Reactions::new(sequence.clone());
    let mut other_table = Reactions::new(sequence);
    reactions.send(0, TableReaction::Wow, START).unwrap();
    reactions
        .send(3, TableReaction::Oops, START + Duration::seconds(1))
        .unwrap();
    other_table
        .send(1, TableReaction::GoodGame, START + Duration::seconds(1))
        .unwrap();

    let recent = reactions.recent(START + Duration::seconds(2));
    let kinds: Vec<TableReaction> = recent.iter().map(|reaction| reaction.reaction).collect();
    assert_eq!(kinds, [TableReaction::Wow, TableReaction::Oops]);
    let positions: Vec<usize> = recent.iter().map(|reaction| reaction.position).collect();
    assert_eq!(positions, [0, 3]);
    assert!(recent[0].sequence < recent[1].sequence);
    assert!(
        other_table
            .recent(START)
            .iter()
            .all(|reaction| reaction.sequence > recent[1].sequence)
    );
}

#[test]
fn forgets_reactions_after_thirty_seconds() {
    let mut reactions = Reactions::new(Arc::default());
    reactions.send(0, TableReaction::Wow, START).unwrap();
    assert_eq!(reactions.recent(START + Duration::seconds(30)).len(), 1);
    assert_eq!(reactions.recent(START + Duration::seconds(31)).len(), 0);
}

#[test]
fn a_player_sends_at_most_three_reactions_in_ten_seconds() {
    let mut reactions = Reactions::new(Arc::default());
    for second in 0..3 {
        reactions
            .send(0, TableReaction::Wow, START + Duration::seconds(second))
            .unwrap();
    }
    assert_eq!(
        message(reactions.send(0, TableReaction::Wow, START + Duration::seconds(9))),
        "Slow down: wait a moment before reacting again."
    );
    reactions
        .send(1, TableReaction::Oops, START + Duration::seconds(9))
        .unwrap();
    reactions
        .send(0, TableReaction::Wow, START + Duration::seconds(10))
        .unwrap();
}

#[test]
fn at_most_forty_reactions_are_kept() {
    let mut reactions = Reactions::new(Arc::default());
    for index in 0..60 {
        let at = START + Duration::seconds(index / 10 * 10 + 10);
        let _ = reactions.send((index % 4) as usize, TableReaction::Wow, at);
    }
    assert!(reactions.recent(START + Duration::seconds(65)).len() <= 40);
}
