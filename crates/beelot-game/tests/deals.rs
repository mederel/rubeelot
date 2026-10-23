//! Replays every deal recorded from the Java engine and checks that the Rust engine shows the same
//! views and reaches the same result.

mod common;

use std::panic::{self, AssertUnwindSafe};

use common::{Step, load_deals, replay};

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
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string()))
                    .unwrap_or_else(|| "a panic without a message".to_string());
                failures.push(format!("{label}: {message}"));
            }
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
