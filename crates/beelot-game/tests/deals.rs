//! Replays every deal recorded from the Java engine and checks that the Rust engine shows the same
//! views and reaches the same result, and that the Rust bot decides as the Java bot did.

mod common;

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};

use beelot_game::bot::BotStrategy;
use beelot_game::bot::rule_based::RuleBasedStrategy;
use common::{Step, load_deals, replay};

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string()))
        .unwrap_or_else(|| "a panic without a message".to_string())
}

#[test]
fn engine_matches_every_recorded_deal() {
    let deals = load_deals();
    let mut steps = 0;
    let mut failures = Vec::new();
    for (label, deal) in &deals {
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            replay(deal, &mut |_: &Step, _| {});
        }));
        match outcome {
            Ok(()) => steps += deal.steps.len(),
            Err(payload) => failures.push(format!("{label}: {}", panic_message(payload))),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} deals do not match Java:\n{}",
        failures.len(),
        deals.len(),
        failures.join("\n\n")
    );
    println!("replayed {} deals, {steps} steps", deals.len());
}

/// For every step whose seat the `"current"` bot plays (`north_south` for even seats, `east_west`
/// for odd ones), `RuleBasedStrategy::new()` gives the recorded call or card. Java's current bot
/// ignores the random generator, so this check draws nothing from it.
#[test]
fn current_bot_matches_java() {
    const BOT: &str = "current";
    let deals = load_deals();
    let strategy = RuleBasedStrategy::new();
    let mut checked = 0;
    let mut failures = Vec::new();
    for (label, deal) in &deals {
        let mut deal_checked = 0;
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut index = 0;
            replay(deal, &mut |step: &Step, _| {
                let step_index = index;
                index += 1;
                let seat = match step {
                    Step::Auction { seat, .. } | Step::Play { seat, .. } => *seat,
                    Step::Continue {} => return,
                };
                let bot = if seat % 2 == 0 {
                    &deal.north_south
                } else {
                    &deal.east_west
                };
                if bot != BOT {
                    return;
                }
                match step {
                    Step::Auction { view, decision, .. } => assert_eq!(
                        strategy.decide_auction(view),
                        *decision,
                        "step {step_index}: bot {bot} at seat {seat}: auction decision \
                         (left: Rust, right: Java)\nview: {view:?}"
                    ),
                    Step::Play { view, card, .. } => assert_eq!(
                        strategy.choose_card(view),
                        *card,
                        "step {step_index}: bot {bot} at seat {seat}: card \
                         (left: Rust, right: Java)\nview: {view:?}"
                    ),
                    Step::Continue {} => unreachable!(),
                }
                deal_checked += 1;
            });
        }));
        match outcome {
            Ok(()) => checked += deal_checked,
            Err(payload) => failures.push(format!("{label}: {}", panic_message(payload))),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} deals have a {BOT} bot decision that differs from Java:\n{}",
        failures.len(),
        deals.len(),
        failures.join("\n\n")
    );
    assert!(checked > 0, "no step is played by the {BOT} bot");
    println!("{checked} {BOT} bot decisions match Java");
}
