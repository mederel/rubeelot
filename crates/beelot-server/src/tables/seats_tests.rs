//! Ports `PrivateTableTest` and `PublicTableTest` (game package), and the name rules of `PassAndPlayServiceTest`.

use std::time::Duration;

use beelot_game::card::Variant;
use time::OffsetDateTime;
use time::macros::datetime;
use uuid::Uuid;

use super::*;
use crate::error::AppError;

const T0: OffsetDateTime = datetime!(2026-10-17 12:00 UTC);

fn refusal<T: std::fmt::Debug>(result: Result<T, AppError>) -> String {
    match result.expect_err("a refusal") {
        AppError::Conflict { message, .. } => message,
        other => panic!("not a conflict: {other:?}"),
    }
}

fn private_table() -> Lobby {
    Lobby::private("Ana".into(), None, Variant::Classic)
}

fn public_table() -> Lobby {
    Lobby::public(
        "Ana".into(),
        None,
        Variant::Classic,
        T0 + Duration::from_secs(60),
    )
}

fn seat_three_more(table: &mut Lobby) -> [usize; 3] {
    [
        table.join("Ben".into(), None).unwrap(),
        table.join("Chloe".into(), None).unwrap(),
        table.join("David".into(), None).unwrap(),
    ]
}

fn names(table: &Lobby) -> Vec<&str> {
    table
        .occupied()
        .map(|(_, seat)| seat.name.as_str())
        .collect()
}

// PrivateTableTest

#[test]
fn a_fifth_player_cannot_join_a_full_table() {
    let mut table = private_table();
    seat_three_more(&mut table);
    assert_eq!(
        refusal(table.join("Eve".into(), None)),
        "This table is full."
    );
    assert_eq!(table.occupied().count(), 4);
}

#[test]
fn nobody_joins_or_leaves_once_the_game_has_started() {
    let mut table = private_table();
    table.set_ready(0, true).unwrap();
    table.start_with_bots(0).unwrap();
    assert_eq!(
        refusal(table.join("Eve".into(), None)),
        "This table has already started."
    );
    assert_eq!(
        refusal(table.choose_seat(0, 1)),
        "This table has already started."
    );
    assert_eq!(
        refusal(table.set_ready(0, false)),
        "This table has already started."
    );
    assert_eq!(refusal(table.leave(0)), "This table has already started.");
}

#[test]
fn the_owner_starts_only_with_four_ready_players() {
    let mut table = private_table();
    let others = seat_three_more(&mut table);
    table.set_ready(0, true).unwrap();
    table.set_ready(others[0], true).unwrap();
    table.set_ready(others[1], true).unwrap();
    assert_eq!(
        refusal(table.start(others[0])),
        "Only the table owner can start the game."
    );
    assert_eq!(
        refusal(table.start(0)),
        "Four ready players are required to start the game."
    );
    table.set_ready(others[2], true).unwrap();
    table.start(0).unwrap();
    assert_eq!(table.status(), Status::InProgress);
    assert_eq!(refusal(table.start(0)), "This table has already started.");
}

#[test]
fn a_player_moves_only_to_a_seat_that_exists() {
    let mut table = private_table();
    assert_eq!(
        refusal(table.choose_seat(0, usize::MAX)),
        "That seat does not exist."
    );
    assert_eq!(
        refusal(table.choose_seat(0, 4)),
        "That seat does not exist."
    );
    assert_eq!(table.choose_seat(0, 0).unwrap(), 0);
    assert_eq!(table.choose_seat(0, 2).unwrap(), 2);
    assert!(table.seat(0).is_none());
    assert_eq!(table.seat(2).unwrap().name, "Ana");
    assert_eq!(table.owner(), 2, "the owner moves with their seat");
}

#[test]
fn a_seat_that_is_taken_cannot_be_chosen() {
    let mut table = private_table();
    let ben = table.join("Ben".into(), None).unwrap();
    assert_eq!(refusal(table.choose_seat(0, ben)), "That seat is taken.");
}

#[test]
fn an_empty_seat_is_not_seated() {
    let mut table = private_table();
    assert_eq!(
        refusal(table.set_ready(3, true)),
        "You are not seated at this table."
    );
}

#[test]
fn seats_are_assigned_at_public_tables() {
    let mut table = public_table();
    assert_eq!(
        refusal(table.choose_seat(0, 2)),
        "Seats are assigned at public tables."
    );
}

#[test]
fn only_the_owner_sets_the_turn_timer_to_none_30_or_60_seconds_before_the_start() {
    let mut table = private_table();
    let guest = table.join("Ben".into(), None).unwrap();
    assert_eq!(
        refusal(table.set_turn_timer(guest, 30)),
        "Only the table owner can change the turn timer."
    );
    assert_eq!(
        refusal(table.set_turn_timer(0, 45)),
        "Choose no timer, 30 seconds, or 60 seconds."
    );
    table.set_turn_timer(0, 60).unwrap();
    assert_eq!(table.turn_timer_seconds(), 60);
    table.set_turn_timer(0, 0).unwrap();
    assert_eq!(table.turn_timer_seconds(), 0);

    table.set_ready(0, true).unwrap();
    table.set_ready(guest, true).unwrap();
    table.start_with_bots(0).unwrap();
    assert_eq!(
        refusal(table.set_turn_timer(0, 30)),
        "The turn timer cannot change after the game starts."
    );
}

#[test]
fn only_the_owner_of_a_private_table_turns_announcements_off_before_the_start() {
    let mut table = private_table();
    let guest = table.join("Ben".into(), None).unwrap();
    assert_eq!(
        refusal(table.set_announcements(guest, false)),
        "Only the table owner can change announcements."
    );
    table.set_announcements(0, false).unwrap();
    assert!(!table.announcements());
    table.set_ready(0, true).unwrap();
    table.set_ready(guest, true).unwrap();
    table.start_with_bots(0).unwrap();
    assert_eq!(
        refusal(table.set_announcements(0, true)),
        "Announcements cannot change after the game starts."
    );
    assert_eq!(
        refusal(public_table().set_announcements(0, false)),
        "Public tables always play with announcements."
    );
}

#[test]
fn a_seat_a_bot_has_taken_over_stays_with_the_bot() {
    let mut table = private_table();
    let guest = table.join("Ben".into(), None).unwrap();
    table.hand_to_bot(guest, "Luc").unwrap();
    table.disconnect(guest, T0).unwrap();
    assert_eq!(
        table.seat(guest).unwrap().connection,
        Connection::BotTakeover
    );
    assert_eq!(table.seat(guest).unwrap().name, "Luc");
    assert_eq!(
        refusal(table.reconnect(guest)),
        "A bot has taken over this seat for the rest of the match."
    );
}

#[test]
fn a_disconnected_seat_goes_to_a_bot_once_the_timeout_passes() {
    let mut table = private_table();
    let guest = table.join("Ben".into(), None).unwrap();
    table.disconnect(guest, T0).unwrap();
    assert_eq!(
        table.seat(guest).unwrap().connection,
        Connection::Disconnected { since: T0 }
    );

    let timeout = Duration::from_secs(120);
    table.replace_expired_disconnections(T0 + Duration::from_secs(119), timeout);
    assert!(matches!(
        table.seat(guest).unwrap().connection,
        Connection::Disconnected { .. }
    ));
    table.reconnect(guest).unwrap();
    assert_eq!(table.seat(guest).unwrap().connection, Connection::Connected);

    table.disconnect(guest, T0).unwrap();
    table.replace_expired_disconnections(T0 + timeout, timeout);
    assert_eq!(
        table.seat(guest).unwrap().connection,
        Connection::BotTakeover
    );
    assert_eq!(table.seat(0).unwrap().connection, Connection::Connected);
}

// PublicTableTest

#[test]
fn public_table_has_a_bot_fill_deadline() {
    let table = public_table();
    assert!(table.is_public());
    assert_eq!(table.bot_fill_at(), Some(T0 + Duration::from_secs(60)));
    assert_eq!(table.status(), Status::WaitingForPlayers);
}

#[test]
fn fourth_join_starts_a_public_table() {
    let mut table = public_table();
    table.join("Benoit".into(), None).unwrap();
    table.join("Chloe".into(), None).unwrap();
    assert_eq!(table.status(), Status::WaitingForPlayers);
    table.join("David".into(), None).unwrap();
    assert_eq!(table.status(), Status::InProgress);
    assert_eq!(table.occupied().count(), 4);
    assert!(table.occupied().all(|(_, seat)| seat.ready));
}

#[test]
fn fourth_join_does_not_start_a_private_table() {
    let mut table = private_table();
    seat_three_more(&mut table);
    assert_eq!(table.status(), Status::WaitingForPlayers);
    assert!(!table.is_public());
    assert_eq!(table.bot_fill_at(), None);
}

#[test]
fn leave_frees_the_seat_and_transfers_ownership() {
    let mut table = public_table();
    let joiner = table.join("Benoit".into(), None).unwrap();
    table.leave(0).unwrap();
    assert_eq!(table.occupied().count(), 1);
    assert_eq!(table.owner(), joiner);
    assert!(!table.is_empty());
    table.leave(joiner).unwrap();
    assert!(table.is_empty());
}

#[test]
fn leave_after_start_fails() {
    let mut table = public_table();
    seat_three_more(&mut table);
    assert!(table.leave(0).is_err());
}

#[test]
fn leave_unknown_player_fails() {
    assert_eq!(
        refusal(public_table().leave(2)),
        "You are not seated at this table."
    );
}

#[test]
fn fill_with_bots_waits_for_the_deadline() {
    let mut table = public_table();
    table.join("Benoit".into(), None).unwrap();
    assert!(!table.fill_with_bots_if_due(T0 + Duration::from_secs(59)));
    assert_eq!(table.status(), Status::WaitingForPlayers);

    assert!(table.fill_with_bots_if_due(T0 + Duration::from_secs(60)));
    assert_eq!(table.status(), Status::InProgress);
    assert_eq!(names(&table), ["Ana", "Benoit", "Camille", "Luc"]);
    for position in [2, 3] {
        let seat = table.seat(position).unwrap();
        assert_eq!(seat.connection, Connection::BotTakeover);
        assert_eq!(seat.kind, SeatKind::Bot);
    }
    assert!(table.occupied().all(|(_, seat)| seat.ready));
    assert!(!table.fill_with_bots_if_due(T0 + Duration::from_secs(61)));
}

#[test]
fn an_emptied_table_is_closed() {
    let mut table = public_table();
    table.leave(0).unwrap();
    assert_eq!(
        refusal(table.join("Benoit".into(), None)),
        "This table does not exist."
    );
    assert!(!table.fill_with_bots_if_due(T0 + Duration::from_secs(3600)));
    assert!(table.is_empty());
}

#[test]
fn fill_with_bots_ignores_private_and_started_tables() {
    let mut private = private_table();
    assert!(!private.fill_with_bots_if_due(T0 + Duration::from_secs(3600)));

    let mut table = public_table();
    seat_three_more(&mut table);
    assert!(!table.fill_with_bots_if_due(T0 + Duration::from_secs(3600)));
    assert_eq!(table.occupied().count(), 4);
}

#[test]
fn start_with_bots_seats_bots_in_the_empty_seats() {
    let mut table = private_table();
    table.choose_seat(0, 1).unwrap();
    let ben = table.join("Ben".into(), None).unwrap();
    assert_eq!(ben, 0);
    assert_eq!(
        refusal(table.start_with_bots(1)),
        "Every seated player must be ready to start with bots."
    );
    table.set_ready(0, true).unwrap();
    table.set_ready(1, true).unwrap();
    table.start_with_bots(1).unwrap();
    assert_eq!(names(&table), ["Ben", "Ana", "Camille", "Luc"]);
}

#[test]
fn seats_keep_their_account() {
    let account = Uuid::new_v4();
    let mut table = Lobby::private("Ana".into(), Some(account), Variant::Contree);
    let ben = table.join("Ben".into(), None).unwrap();
    assert_eq!(table.seat(0).unwrap().account, Some(account));
    assert_eq!(table.seat(ben).unwrap().account, None);
    assert_eq!(table.variant(), Variant::Contree);
}

// Names (PrivateTableService.requiredName, PassAndPlayServiceTest)

#[test]
fn a_player_name_is_trimmed_and_required() {
    assert_eq!(required_name("  Ana ").unwrap(), "Ana");
    assert_eq!(refusal(required_name("   ")), "Enter a player name.");
    assert_eq!(refusal(required_name("")), "Enter a player name.");
    assert_eq!(required_name(&"é".repeat(30)).unwrap(), "é".repeat(30));
    assert_eq!(
        refusal(required_name(&"B".repeat(31))),
        "Player names are limited to 30 characters."
    );
}

fn local_names(seats: &[Option<&str>]) -> Result<[Seat; 4], AppError> {
    let seats: Vec<Option<String>> = seats.iter().map(|name| name.map(str::to_owned)).collect();
    local_seats(Some(&seats))
}

#[test]
fn a_solo_table_seats_you_and_three_bots() {
    let seats = local_seats(None).unwrap();
    let names: Vec<&str> = seats.iter().map(|seat| seat.name.as_str()).collect();
    assert_eq!(names, ["You", "Camille", "Luc", "Manon"]);
    let kinds: Vec<SeatKind> = seats.iter().map(|seat| seat.kind).collect();
    assert_eq!(
        kinds,
        [SeatKind::Human, SeatKind::Bot, SeatKind::Bot, SeatKind::Bot]
    );
}

#[test]
fn humans_take_the_seats_they_chose_and_bots_the_others() {
    let seats = local_names(&[Some("Ana"), None, Some(" Ben "), Some("")]).unwrap();
    let names: Vec<&str> = seats.iter().map(|seat| seat.name.as_str()).collect();
    assert_eq!(names, ["Ana", "Camille", "Ben", "Luc"]);
    let kinds: Vec<SeatKind> = seats.iter().map(|seat| seat.kind).collect();
    assert_eq!(
        kinds,
        [
            SeatKind::Human,
            SeatKind::Bot,
            SeatKind::Human,
            SeatKind::Bot
        ]
    );
}

#[test]
fn bots_are_not_named_like_a_player() {
    let seats = local_names(&[Some("Camille"), Some("Luc"), None, None]).unwrap();
    let names: Vec<&str> = seats.iter().map(|seat| seat.name.as_str()).collect();
    assert_eq!(names, ["Camille", "Luc", "Manon", "Hugo"]);
}

#[test]
fn needs_two_to_four_players_with_different_short_enough_names() {
    assert_eq!(
        refusal(local_names(&[Some("Ana"), None, None, Some(" ")])),
        "Pass and play needs at least two players."
    );
    assert_eq!(
        refusal(local_names(&[Some("Ana"), Some("ana"), None, None])),
        "Each player needs a different name."
    );
    let long = "B".repeat(31);
    assert_eq!(
        refusal(local_names(&[Some("Ana"), Some(&long), None, None])),
        "Player names are limited to 30 characters."
    );
    assert_eq!(
        refusal(local_names(&[Some("Ana"), Some("Ben")])),
        "Fill in the four seats."
    );
}
