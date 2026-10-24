//! The background jobs run the tables' timed rules on their own.

mod common;

use std::time::Duration;

use beelot_game::Variant;
use beelot_server::jobs;
use beelot_server::tables::Status;
use common::test_state_with_clock;
use tokio_util::sync::CancellationToken;

fn status(state: &beelot_server::app::AppState, id: uuid::Uuid) -> Status {
    state
        .registry
        .handle(id)
        .unwrap()
        .state
        .lock()
        .unwrap()
        .lobby()
        .status()
}

#[tokio::test(start_paused = true)]
async fn the_sweep_fills_a_public_table_with_bots_and_eviction_frees_idle_tables() {
    let (state, clock) = test_state_with_clock();
    let cancel = CancellationToken::new();
    let handles = jobs::spawn_all(state.clone(), cancel.clone());
    let public = state
        .registry
        .quick_match("Ana", Variant::Classic, None)
        .unwrap();

    clock.advance(time::Duration::seconds(61));
    tokio::time::sleep(Duration::from_secs(6)).await;
    assert_eq!(
        status(&state, public.table_id),
        Status::InProgress,
        "bots took the empty seats"
    );

    clock.advance(time::Duration::hours(3));
    tokio::time::sleep(Duration::from_secs(61)).await;
    assert!(
        state.registry.handle(public.table_id).is_none(),
        "the idle table was evicted"
    );

    cancel.cancel();
    for handle in handles {
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .unwrap()
            .unwrap();
    }
}
