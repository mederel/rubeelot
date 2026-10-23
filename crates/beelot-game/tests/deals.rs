//! Replays every deal recorded from the Java engine and checks that the Rust engine shows the same
//! views and reaches the same result, and that the Rust bot decides as the Java bot did.

mod common;

use std::any::Any;
use std::collections::BTreeMap;
use std::panic::{self, AssertUnwindSafe};

use beelot_game::bot::arena::{ARENA_BOTS, ArenaBot, arena_bot_unbounded};
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

/// Every recorded decision of every bot equals Java's. Each bot is created once and serves every
/// deal, as in the Java recorder; the replay's shared `SplitMix64` is the generator they draw from,
/// so a bot that draws when Java's does not (or the reverse) desynchronises the later decisions.
/// Panics listing every deal that differs, and when a bot never plays a step.
#[test]
fn every_bot_matches_java() {
    let deals = load_deals();
    let bots: BTreeMap<&str, Box<dyn ArenaBot>> = ARENA_BOTS
        .iter()
        .map(|&name| (name, arena_bot_unbounded(name).expect("an arena bot")))
        .collect();
    let mut checked: BTreeMap<&str, usize> = ARENA_BOTS.iter().map(|&name| (name, 0)).collect();
    let mut failures = Vec::new();
    for (label, deal) in &deals {
        let mut deal_checked: Vec<&str> = Vec::new();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut index = 0;
            replay(deal, &mut |step: &Step, rng| {
                let step_index = index;
                index += 1;
                let seat = match step {
                    Step::Auction { seat, .. } | Step::Play { seat, .. } => *seat,
                    Step::Continue {} => return,
                };
                let name = if seat % 2 == 0 {
                    deal.north_south.as_str()
                } else {
                    deal.east_west.as_str()
                };
                let bot = bots
                    .get(name)
                    .unwrap_or_else(|| panic!("step {step_index}: unknown bot {name}"));
                match step {
                    Step::Auction { view, decision, .. } => assert_eq!(
                        bot.decide_auction(view, rng),
                        *decision,
                        "step {step_index}: bot {name} at seat {seat}: auction decision \
                         (left: Rust, right: Java)\nview: {view:?}"
                    ),
                    Step::Play { view, card, .. } => assert_eq!(
                        bot.choose_card(view, rng),
                        *card,
                        "step {step_index}: bot {name} at seat {seat}: card \
                         (left: Rust, right: Java)\nview: {view:?}"
                    ),
                    Step::Continue {} => unreachable!(),
                }
                deal_checked.push(bot.name());
            });
        }));
        match outcome {
            Ok(()) => {
                for name in deal_checked {
                    *checked.get_mut(name).expect("a known bot") += 1;
                }
            }
            Err(payload) => failures.push(format!("{label}: {}", panic_message(payload))),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} deals have a bot decision that differs from Java:\n{}",
        failures.len(),
        deals.len(),
        failures.join("\n\n")
    );
    for (name, count) in &checked {
        assert!(*count > 0, "no step is played by the {name} bot");
        println!("{count} {name} bot decisions match Java");
    }
}
