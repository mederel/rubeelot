//! Who sits where before and during a match, ported from Java's `PrivateTable` and the seat rules of
//! `BotGameService`. Seats are positions 0–3 in play order; North–South hold the even seats.

use std::time::Duration;

use beelot_game::card::Variant;
use beelot_game::match_score::{DEFAULT_TARGET, TARGETS};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

use super::messages as msg;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TableKind {
    Local,
    Private,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatKind {
    Human,
    Bot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Connected,
    Disconnected {
        since: OffsetDateTime,
    },
    /// A bot plays the seat for the rest of the match.
    BotTakeover,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seat {
    pub name: String,
    pub kind: SeatKind,
    pub ready: bool,
    pub connection: Connection,
    pub account: Option<Uuid>,
}

impl Seat {
    pub fn human(name: String, account: Option<Uuid>) -> Self {
        Self {
            name,
            kind: SeatKind::Human,
            ready: false,
            connection: Connection::Connected,
            account,
        }
    }

    pub fn bot(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            kind: SeatKind::Bot,
            ready: true,
            connection: Connection::BotTakeover,
            account: None,
        }
    }

    /// Whether a bot decides for this seat: a bot's seat, or a human's a bot has taken over.
    pub fn played_by_bot(&self) -> bool {
        self.kind == SeatKind::Bot || self.connection == Connection::BotTakeover
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    WaitingForPlayers,
    InProgress,
}

/// The bots of online tables.
const ONLINE_BOT_NAMES: [&str; 3] = ["Camille", "Luc", "Manon"];
/// The bots of local tables.
const LOCAL_BOT_NAMES: [&str; 4] = ["Camille", "Luc", "Manon", "Hugo"];
const MAX_NAME_LENGTH: usize = 30;

fn conflict(code: &'static str, message: &str) -> AppError {
    AppError::conflict(code, message)
}

#[derive(Debug, Clone)]
pub struct Lobby {
    seats: [Option<Seat>; 4],
    owner: usize,
    status: Status,
    variant: Variant,
    announcements: bool,
    target: i32,
    turn_timer_seconds: u32,
    /// Set for public tables only: bots take the empty seats once it passes.
    bot_fill_at: Option<OffsetDateTime>,
}

impl Lobby {
    fn open(owner: Seat, variant: Variant, bot_fill_at: Option<OffsetDateTime>) -> Self {
        Self {
            seats: [Some(owner), None, None, None],
            owner: 0,
            status: Status::WaitingForPlayers,
            variant,
            announcements: true,
            target: DEFAULT_TARGET,
            turn_timer_seconds: 0,
            bot_fill_at,
        }
    }

    /// A table friends join with its invitation code; its owner sits North.
    pub fn private(owner_name: String, account: Option<Uuid>, variant: Variant) -> Self {
        Self::open(Seat::human(owner_name, account), variant, None)
    }

    /// A matchmaking table: bots take the empty seats once `bot_fill_at` passes.
    pub fn public(
        owner_name: String,
        account: Option<Uuid>,
        variant: Variant,
        bot_fill_at: OffsetDateTime,
    ) -> Self {
        Self::open(Seat::human(owner_name, account), variant, Some(bot_fill_at))
    }

    /// A solo or pass-and-play table, which starts at once.
    pub fn local(seats: [Seat; 4], variant: Variant, announcements: bool, target: i32) -> Self {
        let owner = seats
            .iter()
            .position(|seat| seat.kind == SeatKind::Human)
            .unwrap_or(0);
        Self {
            seats: seats.map(Some),
            owner,
            status: Status::InProgress,
            variant,
            announcements,
            target,
            turn_timer_seconds: 0,
            bot_fill_at: None,
        }
    }

    pub fn seat(&self, position: usize) -> Option<&Seat> {
        self.seats.get(position).and_then(Option::as_ref)
    }

    pub(crate) fn seat_mut(&mut self, position: usize) -> Option<&mut Seat> {
        self.seats.get_mut(position).and_then(Option::as_mut)
    }

    /// The occupied seats with their positions, in table order.
    pub fn occupied(&self) -> impl Iterator<Item = (usize, &Seat)> {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(position, seat)| seat.as_ref().map(|seat| (position, seat)))
    }

    pub fn owner(&self) -> usize {
        self.owner
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn variant(&self) -> Variant {
        self.variant
    }

    pub fn announcements(&self) -> bool {
        self.announcements
    }

    /// The points the match is played to (US-062).
    pub fn target(&self) -> i32 {
        self.target
    }

    pub fn turn_timer_seconds(&self) -> u32 {
        self.turn_timer_seconds
    }

    pub fn bot_fill_at(&self) -> Option<OffsetDateTime> {
        self.bot_fill_at
    }

    pub fn is_public(&self) -> bool {
        self.bot_fill_at.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.seats.iter().all(Option::is_none)
    }

    fn seated_count(&self) -> usize {
        self.seats.iter().filter(|seat| seat.is_some()).count()
    }

    fn seated(&self, position: usize) -> Result<&Seat, AppError> {
        self.seat(position)
            .ok_or_else(|| conflict("not-seated", msg::NOT_SEATED))
    }

    fn seated_mut(&mut self, position: usize) -> Result<&mut Seat, AppError> {
        self.seat_mut(position)
            .ok_or_else(|| conflict("not-seated", msg::NOT_SEATED))
    }

    fn waiting(&self) -> Result<(), AppError> {
        match self.status {
            Status::WaitingForPlayers => Ok(()),
            Status::InProgress => Err(conflict("already-started", msg::ALREADY_STARTED)),
        }
    }

    fn owned_by(&self, position: usize, message: &str) -> Result<(), AppError> {
        if self.owner == position && self.seat(position).is_some() {
            Ok(())
        } else {
            Err(conflict("owner-only", message))
        }
    }

    /// Seats a player in the first empty seat and returns it. The fourth player at a public table starts it.
    pub fn join(&mut self, name: String, account: Option<Uuid>) -> Result<usize, AppError> {
        // Everyone left: the table is being removed, so nobody may sit down at it any more.
        if self.is_empty() {
            return Err(conflict("table-closed", msg::TABLE_CLOSED));
        }
        self.waiting()?;
        let position = self
            .seats
            .iter()
            .position(Option::is_none)
            .ok_or_else(|| conflict("table-full", msg::TABLE_FULL))?;
        self.seats[position] = Some(Seat::human(name, account));
        if self.is_public() && self.seated_count() == 4 {
            self.mark_everyone_ready();
            self.status = Status::InProgress;
        }
        Ok(position)
    }

    /// Frees a seat before the start; the owner's leaving hands the table to the first player still seated.
    pub fn leave(&mut self, position: usize) -> Result<(), AppError> {
        self.seated(position)?;
        self.waiting()?;
        self.seats[position] = None;
        if self.owner == position
            && let Some(first) = self.seats.iter().position(Option::is_some)
        {
            self.owner = first;
        }
        Ok(())
    }

    /// Moves a waiting player to an empty seat, so players pick their partners before a private game starts.
    /// Returns the player's new position.
    pub fn choose_seat(&mut self, position: usize, to: usize) -> Result<usize, AppError> {
        self.waiting()?;
        if self.is_public() {
            return Err(conflict("seats-assigned", msg::SEATS_ASSIGNED));
        }
        self.seated(position)?;
        if to >= self.seats.len() {
            return Err(conflict("no-such-seat", msg::NO_SUCH_SEAT));
        }
        if to == position {
            return Ok(to);
        }
        if self.seats[to].is_some() {
            return Err(conflict("seat-taken", msg::SEAT_TAKEN));
        }
        self.seats[to] = self.seats[position].take();
        if self.owner == position {
            self.owner = to;
        }
        Ok(to)
    }

    pub fn set_ready(&mut self, position: usize, ready: bool) -> Result<(), AppError> {
        self.waiting()?;
        self.seated_mut(position)?.ready = ready;
        Ok(())
    }

    pub fn start(&mut self, position: usize) -> Result<(), AppError> {
        self.owned_by(position, msg::OWNER_STARTS)?;
        self.waiting()?;
        if self.seated_count() != 4 || self.occupied().any(|(_, seat)| !seat.ready) {
            return Err(conflict("not-ready", msg::FOUR_READY_PLAYERS));
        }
        self.status = Status::InProgress;
        Ok(())
    }

    pub fn start_with_bots(&mut self, position: usize) -> Result<(), AppError> {
        self.owned_by(position, msg::OWNER_STARTS)?;
        self.waiting()?;
        if self.occupied().any(|(_, seat)| !seat.ready) {
            return Err(conflict("not-ready", msg::READY_FOR_BOTS));
        }
        self.seat_bots_and_start();
        Ok(())
    }

    /// Starts a waiting public table with bots in the empty seats once its deadline has passed; true when it did.
    pub fn fill_with_bots_if_due(&mut self, now: OffsetDateTime) -> bool {
        let due = matches!(self.bot_fill_at, Some(at) if at <= now);
        if !due || self.is_empty() || self.status != Status::WaitingForPlayers {
            return false;
        }
        self.mark_everyone_ready();
        self.seat_bots_and_start();
        true
    }

    fn seat_bots_and_start(&mut self) {
        let mut names = ONLINE_BOT_NAMES.iter();
        for seat in self.seats.iter_mut().filter(|seat| seat.is_none()) {
            *seat = Some(Seat::bot(names.next().expect("three bots at most")));
        }
        self.status = Status::InProgress;
    }

    fn mark_everyone_ready(&mut self) {
        for seat in self.seats.iter_mut().flatten() {
            seat.ready = true;
        }
    }

    pub fn disconnect(&mut self, position: usize, now: OffsetDateTime) -> Result<(), AppError> {
        let seat = self.seated_mut(position)?;
        if seat.connection != Connection::BotTakeover {
            seat.connection = Connection::Disconnected { since: now };
        }
        Ok(())
    }

    pub fn reconnect(&mut self, position: usize) -> Result<(), AppError> {
        let seat = self.seated_mut(position)?;
        if seat.connection == Connection::BotTakeover {
            return Err(conflict("bot-took-over", msg::BOT_TOOK_OVER));
        }
        seat.connection = Connection::Connected;
        Ok(())
    }

    /// Hands to a bot every seat disconnected for at least `timeout`; returns whether any seat changed.
    pub fn replace_expired_disconnections(
        &mut self,
        now: OffsetDateTime,
        timeout: Duration,
    ) -> bool {
        let mut changed = false;
        for seat in self.seats.iter_mut().flatten() {
            if let Connection::Disconnected { since } = seat.connection
                && since + timeout <= now
            {
                seat.connection = Connection::BotTakeover;
                changed = true;
            }
        }
        changed
    }

    /// A bot plays the seat, under the given name, for the rest of the match: the player is gone for good.
    pub fn hand_to_bot(&mut self, position: usize, name: &str) -> Result<(), AppError> {
        let seat = self.seated_mut(position)?;
        seat.name = name.to_owned();
        seat.connection = Connection::BotTakeover;
        Ok(())
    }

    pub fn set_turn_timer(&mut self, position: usize, seconds: u32) -> Result<(), AppError> {
        self.owned_by(position, msg::OWNER_SETS_TIMER)?;
        if self.status != Status::WaitingForPlayers {
            return Err(conflict("settings-locked", msg::TIMER_LOCKED));
        }
        if !matches!(seconds, 0 | 30 | 60) {
            return Err(conflict("invalid-turn-timer", msg::TIMER_CHOICES));
        }
        self.turn_timer_seconds = seconds;
        Ok(())
    }

    /// Before the game starts, the owner of a private table may turn announcements off.
    pub fn set_announcements(&mut self, position: usize, on: bool) -> Result<(), AppError> {
        if self.is_public() {
            return Err(conflict("public-announcements", msg::PUBLIC_ANNOUNCEMENTS));
        }
        self.owned_by(position, msg::OWNER_SETS_ANNOUNCEMENTS)?;
        if self.status != Status::WaitingForPlayers {
            return Err(conflict("settings-locked", msg::ANNOUNCEMENTS_LOCKED));
        }
        self.announcements = on;
        Ok(())
    }

    /// Before the game starts, the owner of a private table may choose the match target.
    pub fn set_target(&mut self, position: usize, target: i32) -> Result<(), AppError> {
        if self.is_public() {
            return Err(conflict("public-target", msg::PUBLIC_TARGET));
        }
        self.owned_by(position, msg::OWNER_SETS_TARGET)?;
        if self.status != Status::WaitingForPlayers {
            return Err(conflict("settings-locked", msg::TARGET_LOCKED));
        }
        self.target = valid_target(target)?;
        Ok(())
    }
}

/// A match target players may choose: 500, 1,000, 1,500 or 2,000 points.
pub fn valid_target(target: i32) -> Result<i32, AppError> {
    if TARGETS.contains(&target) {
        Ok(target)
    } else {
        Err(conflict("invalid-target", msg::TARGET_CHOICES))
    }
}

/// Trims a player's name: it is required, and 30 characters at most.
pub fn required_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(conflict("name-required", msg::NAME_REQUIRED));
    }
    if trimmed.chars().count() > MAX_NAME_LENGTH {
        return Err(conflict("name-too-long", msg::NAME_TOO_LONG));
    }
    Ok(trimmed.to_owned())
}

/// The seats of a local table. With no names, a solo table: `You` against three bots. With four entries, a
/// pass-and-play table: the named seats are the humans sharing the device, and bots take the blank ones.
pub fn local_seats(seats: Option<&[Option<String>]>) -> Result<[Seat; 4], AppError> {
    let Some(names) = seats else {
        return Ok([
            Seat::human("You".into(), None),
            Seat::bot("Camille"),
            Seat::bot("Luc"),
            Seat::bot("Manon"),
        ]);
    };
    if names.len() != 4 {
        return Err(conflict("four-seats", msg::FOUR_SEATS));
    }
    let human = |name: &Option<String>| {
        name.as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    };
    let humans: Vec<String> = names.iter().filter_map(human).collect();
    if humans.len() < 2 {
        return Err(conflict("two-players", msg::TWO_PLAYERS));
    }
    if humans
        .iter()
        .any(|name| name.chars().count() > MAX_NAME_LENGTH)
    {
        return Err(conflict("name-too-long", msg::NAME_TOO_LONG));
    }
    let mut lower: Vec<String> = humans.iter().map(|name| name.to_lowercase()).collect();
    lower.sort();
    lower.dedup();
    if lower.len() < humans.len() {
        return Err(conflict("different-names", msg::DIFFERENT_NAMES));
    }
    let mut bots = LOCAL_BOT_NAMES
        .iter()
        .filter(|bot| !humans.iter().any(|name| name.eq_ignore_ascii_case(bot)));
    let seats: Vec<Seat> = names
        .iter()
        .map(|name| match human(name) {
            Some(name) => Seat::human(name, None),
            None => Seat::bot(bots.next().expect("four bot names for at most two bots")),
        })
        .collect();
    Ok(seats.try_into().expect("four seats"))
}

#[cfg(test)]
#[path = "seats_tests.rs"]
mod tests;
