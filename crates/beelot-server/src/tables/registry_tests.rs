//! Ports Java's `MatchmakingServiceTest`, `PublicTableServiceTest` and `MemoryBoundsTest`, and the registry cases of
//! `PrivateTableServiceTest`.

use std::sync::Arc;
use std::time::Duration;

use beelot_game::bot::strategies;
use beelot_game::{Difficulty, Variant};
use time::macros::datetime;
use uuid::Uuid;

use super::*;
use crate::clock::FakeClock;
use crate::config::Limits;
use crate::error::AppError;
use crate::tables::reactions::TableReaction;
use crate::tables::{Action, Actor, Connection, Status, TableKind, TableSetup};

fn registry_with(limits: Limits) -> (TableRegistry, Arc<FakeClock>) {
    let clock = Arc::new(FakeClock::new(datetime!(2026-10-24 12:00 UTC)));
    let registry = TableRegistry::new(&limits, clock.clone()).with_setup(|| {
        TableSetup::seeded(7).with_strategy(strategies::for_difficulty(Difficulty::Relaxed))
    });
    (registry, clock)
}

fn registry() -> (TableRegistry, Arc<FakeClock>) {
    registry_with(Limits::default())
}

fn conflict(error: AppError) -> String {
    match error {
        AppError::Conflict { message, .. } => message,
        other => panic!("not a conflict: {other:?}"),
    }
}

fn status(registry: &TableRegistry, id: Uuid) -> Status {
    registry
        .handle(id)
        .unwrap()
        .state
        .lock()
        .unwrap()
        .lobby()
        .status()
}

fn seat_count(registry: &TableRegistry, id: Uuid) -> usize {
    registry
        .handle(id)
        .unwrap()
        .state
        .lock()
        .unwrap()
        .lobby()
        .occupied()
        .count()
}

fn act(registry: &TableRegistry, access: &Access, action: Action) -> Result<(), AppError> {
    let (handle, actor) = registry.authorize(access.table_id, &access.player_token)?;
    registry.act(&handle, actor, action).map(|_| ())
}

// Tokens

#[test]
fn a_token_authorizes_its_seat_at_its_table_only() {
    let (registry, _) = registry();
    let ana = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let code = ana.code.clone().unwrap();
    let ben = registry.join(&code, "Ben", None).unwrap();
    let other = registry
        .create_private("Chloe", Variant::Classic, None)
        .unwrap();

    assert_eq!(
        registry
            .authorize(ana.table_id, &ana.player_token)
            .unwrap()
            .1,
        Actor::Seat(0)
    );
    assert_eq!(
        registry
            .authorize(ben.table_id, &ben.player_token)
            .unwrap()
            .1,
        Actor::Seat(1)
    );
    assert!(matches!(
        registry.authorize(other.table_id, &ana.player_token),
        Err(AppError::Unauthorized { .. })
    ));
    assert!(matches!(
        registry.authorize(ana.table_id, "not-a-token"),
        Err(AppError::Unauthorized { .. })
    ));
    assert!(matches!(
        registry.authorize(Uuid::new_v4(), &ana.player_token),
        Err(AppError::NotFound)
    ));
}

#[test]
fn tokens_are_long_random_and_url_safe() {
    let (registry, _) = registry();
    let first = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let second = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    assert_ne!(first.player_token, second.player_token);
    assert_eq!(first.player_token.len(), 43, "32 bytes in unpadded base64");
    assert!(
        first
            .player_token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    );
}

#[test]
fn token_hash_only() {
    let (registry, _) = registry();
    let access = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let local = registry
        .create_local(
            Variant::Classic,
            Difficulty::Relaxed,
            true,
            1_000,
            None,
            None,
        )
        .unwrap();
    let debug = format!(
        "{registry:?} {:?} {:?}",
        registry.handle(access.table_id).unwrap(),
        access
    );
    assert!(!debug.contains(&access.player_token));
    assert!(!debug.contains(&local.player_token));
}

#[test]
fn a_token_follows_its_player_to_another_seat() {
    let (registry, _) = registry();
    let ana = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let ben = registry
        .join(ana.code.as_deref().unwrap(), "Ben", None)
        .unwrap();
    act(&registry, &ben, Action::Seat { position: 2 }).unwrap();
    assert_eq!(
        registry
            .authorize(ben.table_id, &ben.player_token)
            .unwrap()
            .1,
        Actor::Seat(2)
    );
    act(&registry, &ben, Action::Ready { ready: true }).unwrap();
    let handle = registry.handle(ben.table_id).unwrap();
    assert!(handle.state.lock().unwrap().lobby().seat(2).unwrap().ready);
}

#[test]
fn a_player_who_leaves_loses_their_token() {
    let (registry, _) = registry();
    let ana = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let ben = registry
        .join(ana.code.as_deref().unwrap(), "Ben", None)
        .unwrap();
    act(&registry, &ben, Action::Leave).unwrap();
    assert!(registry.authorize(ben.table_id, &ben.player_token).is_err());
    let chloe = registry
        .join(ana.code.as_deref().unwrap(), "Chloe", None)
        .unwrap();
    assert_eq!(
        registry
            .authorize(chloe.table_id, &chloe.player_token)
            .unwrap()
            .1,
        Actor::Seat(1)
    );
}

#[test]
fn every_change_is_published() {
    let (registry, _) = registry();
    let ana = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let handle = registry.handle(ana.table_id).unwrap();
    let mut changes = handle.changes.subscribe();
    let before = *changes.borrow_and_update();
    registry
        .join(ana.code.as_deref().unwrap(), "Ben", None)
        .unwrap();
    assert!(changes.has_changed().unwrap());
    let after = *changes.borrow_and_update();
    assert!(after > before);
    act(&registry, &ana, Action::Ready { ready: true }).unwrap();
    assert!(*changes.borrow_and_update() > after);
}

// Invitation codes

#[test]
fn invitation_codes_use_six_unambiguous_characters() {
    for _ in 0..200 {
        let code = codes::new_code(|_| false);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| codes::ALPHABET.contains(c)), "{code}");
    }
    let mut calls = 0;
    let code = codes::new_code(|_| {
        calls += 1;
        calls < 3
    });
    assert_eq!(calls, 3, "a taken code is drawn again");
    assert_eq!(code.len(), 6);
}

#[test]
fn codes_are_matched_trimmed_and_upper_cased() {
    let (registry, _) = registry();
    let ana = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let code = ana.code.clone().unwrap();
    let ben = registry
        .join(&format!("  {}  ", code.to_lowercase()), "Ben", None)
        .unwrap();
    assert_eq!(ben.table_id, ana.table_id);
    assert_eq!(
        conflict(registry.join("NOPE42", "Chloe", None).unwrap_err()),
        "The invitation code is invalid."
    );
    let long = format!("{code}{}", " ".repeat(40));
    assert_eq!(
        conflict(registry.join(&long, "Chloe", None).unwrap_err()),
        "The invitation code is invalid."
    );
}

#[test]
fn creating_and_joining_need_a_name() {
    let (registry, _) = registry();
    assert_eq!(
        conflict(
            registry
                .create_private(" ", Variant::Classic, None)
                .unwrap_err()
        ),
        "Enter a player name."
    );
    let ana = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    assert_eq!(
        conflict(
            registry
                .join(ana.code.as_deref().unwrap(), &"x".repeat(200), None)
                .unwrap_err()
        ),
        "Player names are limited to 30 characters."
    );
}

// MatchmakingServiceTest

#[test]
fn first_player_opens_a_table() {
    let (registry, _) = registry();
    let access = registry.quick_match("Ana", Variant::Classic, None).unwrap();
    let handle = registry.handle(access.table_id).unwrap();
    let table = handle.state.lock().unwrap();
    assert_eq!(table.kind, TableKind::Public);
    assert!(table.lobby().is_public());
    assert_eq!(table.lobby().occupied().count(), 1);
    assert_eq!(access.code, None);
}

#[test]
fn players_of_the_same_variant_share_a_table() {
    let (registry, _) = registry();
    let first = registry.quick_match("Ana", Variant::Classic, None).unwrap();
    let second = registry
        .quick_match("Benoit", Variant::Classic, None)
        .unwrap();
    assert_eq!(first.table_id, second.table_id);
    assert_eq!(seat_count(&registry, second.table_id), 2);
}

#[test]
fn variants_are_not_mixed() {
    let (registry, _) = registry();
    let classic = registry.quick_match("Ana", Variant::Classic, None).unwrap();
    let contree = registry
        .quick_match("Benoit", Variant::Contree, None)
        .unwrap();
    assert_ne!(classic.table_id, contree.table_id);
}

#[test]
fn quick_match_prefers_fullest_table() {
    let (registry, clock) = registry();
    let emptier = registry
        .open_public("David", Variant::Classic, None)
        .unwrap();
    clock.advance(time::Duration::seconds(1));
    let fuller = registry.open_public("Ana", Variant::Classic, None).unwrap();
    registry
        .join_public(fuller.table_id, "Benoit", None)
        .unwrap();
    registry
        .join_public(fuller.table_id, "Chloe", None)
        .unwrap();

    let emma = registry
        .quick_match("Emma", Variant::Classic, None)
        .unwrap();
    assert_eq!(emma.table_id, fuller.table_id);
    assert_eq!(status(&registry, fuller.table_id), Status::InProgress);

    // Equally full tables: the one whose bots come first.
    let later = registry
        .open_public("Hugo", Variant::Classic, None)
        .unwrap();
    let next = registry
        .quick_match("Ines", Variant::Classic, None)
        .unwrap();
    assert_eq!(next.table_id, emptier.table_id);
    assert_ne!(next.table_id, later.table_id);
}

#[test]
fn four_quick_matches_start_one_game_and_the_fifth_opens_a_new_table() {
    let (registry, _) = registry();
    let mut table = None;
    for name in ["Ana", "Benoit", "Chloe", "David"] {
        let seated = registry
            .quick_match(name, Variant::Classic, None)
            .unwrap()
            .table_id;
        if let Some(table) = table {
            assert_eq!(table, seated);
        }
        table = Some(seated);
    }
    let table = table.unwrap();
    assert_eq!(status(&registry, table), Status::InProgress);
    let fifth = registry
        .quick_match("Emma", Variant::Classic, None)
        .unwrap();
    assert_ne!(fifth.table_id, table);
    assert_eq!(seat_count(&registry, fifth.table_id), 1);
}

#[test]
fn after_the_last_player_leaves_a_new_table_is_opened() {
    let (registry, _) = registry();
    let first = registry.quick_match("Ana", Variant::Classic, None).unwrap();
    act(&registry, &first, Action::Leave).unwrap();
    assert!(
        registry.handle(first.table_id).is_none(),
        "the empty table is removed"
    );
    let next = registry
        .quick_match("Benoit", Variant::Classic, None)
        .unwrap();
    assert_ne!(first.table_id, next.table_id);
}

#[test]
fn parallel_quick_matches_fill_tables_completely() {
    let (registry, _) = registry();
    let registry = Arc::new(registry);
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let seated: Vec<Uuid> = (0..8)
        .map(|index| {
            let registry = registry.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                registry
                    .quick_match(&format!("Player {index}"), Variant::Classic, None)
                    .unwrap()
                    .table_id
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    let mut distinct = seated.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 2);
    for table in distinct {
        assert_eq!(status(&registry, table), Status::InProgress);
        assert_eq!(seat_count(&registry, table), 4);
    }
}

#[test]
fn rejects_a_blank_name() {
    let (registry, _) = registry();
    assert!(registry.quick_match(" ", Variant::Classic, None).is_err());
}

// PublicTableServiceTest

#[test]
fn open_public_creates_a_waiting_public_table() {
    let (registry, clock) = registry();
    let access = registry.open_public("Ana", Variant::Classic, None).unwrap();
    let handle = registry.handle(access.table_id).unwrap();
    let table = handle.state.lock().unwrap();
    assert_eq!(table.invitation_code, None);
    assert_eq!(
        table.lobby().bot_fill_at(),
        Some(clock.now() + Duration::from_secs(60))
    );
    drop(table);
    assert_eq!(
        registry.open_public_tables(Variant::Classic),
        [access.table_id]
    );
    assert!(registry.open_public_tables(Variant::Contree).is_empty());
}

#[test]
fn public_tables_always_play_with_announcements() {
    let (registry, _) = registry();
    let access = registry.open_public("Ana", Variant::Classic, None).unwrap();
    let refused = act(
        &registry,
        &access,
        Action::Settings {
            announcements: Some(false),
            turn_timer: None,
            target: None,
        },
    );
    assert_eq!(
        conflict(refused.unwrap_err()),
        "Public tables always play with announcements."
    );
}

#[test]
fn fourth_public_join_begins_the_match() {
    let (registry, _) = registry();
    let id = registry
        .open_public("Ana", Variant::Classic, None)
        .unwrap()
        .table_id;
    registry.join_public(id, "Benoit", None).unwrap();
    registry.join_public(id, "Chloe", None).unwrap();
    registry.join_public(id, "David", None).unwrap();
    let handle = registry.handle(id).unwrap();
    let table = handle.state.lock().unwrap();
    assert_eq!(table.lobby().status(), Status::InProgress);
    assert_eq!(table.bidding().unwrap().hand(3).len(), 5);
    drop(table);
    assert!(registry.open_public_tables(Variant::Classic).is_empty());
}

#[test]
fn join_public_rejects_private_tables() {
    let (registry, _) = registry();
    let private = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    assert_eq!(
        conflict(
            registry
                .join_public(private.table_id, "Benoit", None)
                .unwrap_err()
        ),
        "This table does not exist."
    );
}

#[test]
fn the_sweep_fills_public_tables_with_bots_once_their_wait_is_over() {
    let (registry, clock) = registry();
    let owner = registry.open_public("Ana", Variant::Classic, None).unwrap();
    registry
        .join_public(owner.table_id, "Benoit", None)
        .unwrap();

    clock.advance(time::Duration::seconds(59));
    registry.sweep(registry.now());
    assert_eq!(status(&registry, owner.table_id), Status::WaitingForPlayers);

    clock.advance(time::Duration::seconds(2));
    registry.sweep(registry.now());
    let handle = registry.handle(owner.table_id).unwrap();
    let table = handle.state.lock().unwrap();
    assert_eq!(table.lobby().status(), Status::InProgress);
    assert_eq!(table.lobby().occupied().count(), 4);
    assert_eq!(table.bidding().unwrap().hand(0).len(), 5);
}

#[test]
fn fill_after_auto_start_does_nothing() {
    let (registry, clock) = registry();
    let id = registry
        .open_public("Ana", Variant::Classic, None)
        .unwrap()
        .table_id;
    for name in ["Benoit", "Chloe", "David"] {
        registry.join_public(id, name, None).unwrap();
    }
    let hand = registry
        .handle(id)
        .unwrap()
        .state
        .lock()
        .unwrap()
        .bidding()
        .unwrap()
        .hand(0)
        .to_vec();
    clock.advance(time::Duration::seconds(61));
    registry.sweep(registry.now());
    let handle = registry.handle(id).unwrap();
    assert_eq!(
        handle.state.lock().unwrap().bidding().unwrap().hand(0),
        hand.as_slice()
    );
}

#[test]
fn owner_leaving_then_bot_fill_still_plays() {
    let (registry, clock) = registry();
    let owner = registry.open_public("Ana", Variant::Classic, None).unwrap();
    let joiner = registry
        .join_public(owner.table_id, "Benoit", None)
        .unwrap();
    act(&registry, &owner, Action::Leave).unwrap();
    assert_eq!(seat_count(&registry, owner.table_id), 1);
    clock.advance(time::Duration::seconds(61));
    registry.sweep(registry.now());
    assert_eq!(status(&registry, owner.table_id), Status::InProgress);
    assert!(
        registry
            .authorize(owner.table_id, &owner.player_token)
            .is_err()
    );
    assert!(
        registry
            .authorize(joiner.table_id, &joiner.player_token)
            .is_ok()
    );
}

#[test]
fn last_leave_removes_the_table() {
    let (registry, _) = registry();
    let owner = registry.open_public("Ana", Variant::Classic, None).unwrap();
    act(&registry, &owner, Action::Leave).unwrap();
    assert_eq!(registry.counts(), (0, 0));
    assert!(registry.handle(owner.table_id).is_none());
}

#[test]
fn leave_after_start_fails() {
    let (registry, _) = registry();
    let owner = registry.open_public("Ana", Variant::Classic, None).unwrap();
    for name in ["Benoit", "Chloe", "David"] {
        registry.join_public(owner.table_id, name, None).unwrap();
    }
    assert!(act(&registry, &owner, Action::Leave).is_err());
}

// Presence

#[test]
fn the_sweep_hands_expired_disconnections_to_bots() {
    let (registry, clock) = registry();
    let owner = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let handle = registry.handle(owner.table_id).unwrap();
    handle
        .state
        .lock()
        .unwrap()
        .disconnect(0, clock.now())
        .unwrap();
    clock.advance(time::Duration::seconds(119));
    registry.sweep(registry.now());
    assert!(matches!(
        handle
            .state
            .lock()
            .unwrap()
            .lobby()
            .seat(0)
            .unwrap()
            .connection,
        Connection::Disconnected { .. }
    ));
    clock.advance(time::Duration::seconds(1));
    registry.sweep(registry.now());
    assert_eq!(
        handle
            .state
            .lock()
            .unwrap()
            .lobby()
            .seat(0)
            .unwrap()
            .connection,
        Connection::BotTakeover
    );
}

#[test]
fn the_sweep_collects_a_trick_four_seconds_after_it_is_shown() {
    let (registry, clock) = registry();
    let owner = registry
        .create_private("Ana", Variant::Contree, None)
        .unwrap();
    act(&registry, &owner, Action::Ready { ready: true }).unwrap();
    act(
        &registry,
        &owner,
        Action::Start {
            fill_with_bots: true,
        },
    )
    .unwrap();
    let handle = registry.handle(owner.table_id).unwrap();
    loop {
        let action = {
            let table = handle.state.lock().unwrap();
            match table.board() {
                None if table.bidding().unwrap().highest_bid() < 80 => Action::Contract {
                    value: 80,
                    suit: Some(beelot_game::Suit::Hearts),
                },
                None => Action::Pass,
                Some(board) if board.reviewing_completed_trick() => break,
                Some(board) => {
                    let card = board.legal_cards(0)[0];
                    Action::Card {
                        rank: card.rank.label().into(),
                        suit: card.suit,
                        declare: None,
                    }
                }
            }
        };
        act(&registry, &owner, action).unwrap();
    }
    clock.advance(time::Duration::milliseconds(3900));
    registry.sweep(registry.now());
    assert!(
        handle
            .state
            .lock()
            .unwrap()
            .board()
            .unwrap()
            .reviewing_completed_trick()
    );
    clock.advance(time::Duration::milliseconds(100));
    registry.sweep(registry.now());
    assert!(
        !handle
            .state
            .lock()
            .unwrap()
            .board()
            .unwrap()
            .reviewing_completed_trick()
    );
}

// MemoryBoundsTest

#[test]
fn local_tables_are_capped_and_idle_tables_free_capacity() {
    let (registry, clock) = registry_with(Limits {
        max_local_tables: 2,
        ..Limits::default()
    });
    let first = registry
        .create_local(
            Variant::Classic,
            Difficulty::Relaxed,
            true,
            1_000,
            None,
            None,
        )
        .unwrap();
    registry
        .create_local(
            Variant::Classic,
            Difficulty::Relaxed,
            true,
            1_000,
            None,
            None,
        )
        .unwrap();
    assert!(matches!(
        registry.create_local(
            Variant::Classic,
            Difficulty::Relaxed,
            true,
            1_000,
            None,
            None
        ),
        Err(AppError::CapacityExceeded)
    ));
    registry.evict_idle(registry.now());
    assert_eq!(registry.counts(), (2, 0));

    clock.advance(time::Duration::hours(2) + time::Duration::seconds(1));
    let third = registry.create_local(
        Variant::Classic,
        Difficulty::Relaxed,
        true,
        1_000,
        None,
        None,
    );
    assert!(third.is_ok(), "idle tables are evicted to make room");
    assert!(registry.handle(first.table_id).is_none());
    assert_eq!(registry.counts().0, 1);
}

#[test]
fn online_tables_are_capped_and_idle_tables_evicted_with_their_codes() {
    let (registry, clock) = registry_with(Limits {
        max_online_tables: 1,
        ..Limits::default()
    });
    let first = registry
        .create_private("Ann", Variant::Classic, None)
        .unwrap();
    assert!(matches!(
        registry.create_private("Bob", Variant::Classic, None),
        Err(AppError::CapacityExceeded)
    ));
    assert!(matches!(
        registry.quick_match("Bob", Variant::Classic, None),
        Err(AppError::CapacityExceeded)
    ));
    clock.advance(time::Duration::hours(3));
    registry
        .create_private("Bob", Variant::Classic, None)
        .unwrap();
    assert_eq!(
        conflict(
            registry
                .join(first.code.as_deref().unwrap(), "Cy", None)
                .unwrap_err()
        ),
        "The invitation code is invalid."
    );
    assert_eq!(registry.counts(), (0, 1));
}

#[test]
fn local_and_online_tables_have_separate_caps() {
    let (registry, _) = registry_with(Limits {
        max_local_tables: 1,
        max_online_tables: 1,
        ..Limits::default()
    });
    registry
        .create_local(
            Variant::Classic,
            Difficulty::Relaxed,
            true,
            1_000,
            None,
            None,
        )
        .unwrap();
    registry
        .create_private("Ann", Variant::Classic, None)
        .unwrap();
    assert_eq!(registry.counts(), (1, 1));
}

#[test]
fn a_used_table_is_not_idle() {
    let (registry, clock) = registry();
    let access = registry
        .create_private("Ann", Variant::Classic, None)
        .unwrap();
    clock.advance(time::Duration::minutes(90));
    act(&registry, &access, Action::Ready { ready: true }).unwrap();
    clock.advance(time::Duration::minutes(90));
    registry.evict_idle(registry.now());
    assert!(registry.handle(access.table_id).is_some());
}

// Local tables

#[test]
fn a_solo_table_belongs_to_its_account_and_a_device_token() {
    let (registry, _) = registry();
    let account = Uuid::new_v4();
    let access = registry
        .create_local(
            Variant::Contree,
            Difficulty::Challenging,
            false,
            1_000,
            None,
            Some(account),
        )
        .unwrap();
    let (handle, actor) = registry
        .authorize(access.table_id, &access.player_token)
        .unwrap();
    assert_eq!(actor, Actor::Device);
    let table = handle.state.lock().unwrap();
    assert_eq!(table.kind, TableKind::Local);
    assert_eq!(table.difficulty, Some(Difficulty::Challenging));
    assert_eq!(table.lobby().seat(0).unwrap().account, Some(account));
    assert!(!table.lobby().announcements());
    assert!(table.bidding().is_some(), "a local table is dealt at once");
}

#[test]
fn a_pass_and_play_table_belongs_to_no_account() {
    let (registry, _) = registry();
    let seats = vec![Some("Ana".into()), None, Some("Ben".into()), None];
    let access = registry
        .create_local(
            Variant::Classic,
            Difficulty::Relaxed,
            true,
            1_000,
            Some(seats),
            Some(Uuid::new_v4()),
        )
        .unwrap();
    let handle = registry.handle(access.table_id).unwrap();
    let table = handle.state.lock().unwrap();
    assert!(table.pass_and_play());
    assert!(
        table
            .lobby()
            .occupied()
            .all(|(_, seat)| seat.account.is_none())
    );
}

// Released accounts

#[test]
fn a_released_account_loses_its_seats_and_tokens_everywhere() {
    let (registry, _) = registry();
    let (deleted, kept) = (Uuid::new_v4(), Uuid::new_v4());
    let ana = registry
        .create_private("Ana", Variant::Classic, Some(kept))
        .unwrap();
    let ben = registry
        .join(ana.code.as_deref().unwrap(), "Ben", Some(deleted))
        .unwrap();
    act(&registry, &ana, Action::Ready { ready: true }).unwrap();
    act(&registry, &ben, Action::Ready { ready: true }).unwrap();
    act(
        &registry,
        &ana,
        Action::Start {
            fill_with_bots: true,
        },
    )
    .unwrap();
    let waiting = registry
        .create_private("Ben", Variant::Classic, Some(deleted))
        .unwrap();
    let solo = registry
        .create_local(
            Variant::Classic,
            Difficulty::Relaxed,
            true,
            1_000,
            None,
            Some(deleted),
        )
        .unwrap();

    registry.release_account(deleted);

    let handle = registry.handle(ana.table_id).unwrap();
    let seat = handle
        .state
        .lock()
        .unwrap()
        .lobby()
        .seat(1)
        .unwrap()
        .clone();
    assert_eq!(seat.name, "Deleted player");
    assert_eq!(seat.connection, Connection::BotTakeover);
    assert!(registry.authorize(ben.table_id, &ben.player_token).is_err());
    assert!(registry.authorize(ana.table_id, &ana.player_token).is_ok());
    assert!(
        registry.handle(waiting.table_id).is_none(),
        "the table left empty is removed"
    );
    assert!(
        registry
            .authorize(solo.table_id, &solo.player_token)
            .is_err()
    );
}

// Reactions

#[test]
fn seated_players_react_once_the_game_has_started() {
    let (registry, _) = registry();
    let ana = registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    let ben = registry
        .join(ana.code.as_deref().unwrap(), "Ben", None)
        .unwrap();
    let wow = Action::React {
        reaction: TableReaction::Wow,
    };
    assert_eq!(
        conflict(act(&registry, &ana, wow.clone()).unwrap_err()),
        "Reactions open once the game starts."
    );
    act(&registry, &ana, Action::Ready { ready: true }).unwrap();
    act(&registry, &ben, Action::Ready { ready: true }).unwrap();
    act(
        &registry,
        &ana,
        Action::Start {
            fill_with_bots: true,
        },
    )
    .unwrap();

    act(
        &registry,
        &ben,
        Action::React {
            reaction: TableReaction::WellPlayed,
        },
    )
    .unwrap();
    let handle = registry.handle(ana.table_id).unwrap();
    let table = handle.state.lock().unwrap();
    let recent = table.reactions(registry.now());
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].reaction, TableReaction::WellPlayed);
    assert_eq!(recent[0].position, 1);
}
