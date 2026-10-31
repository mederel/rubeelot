//! One table: its seats, the auction or the card play, the match score, and the bots that play their turns. Solo,
//! pass-and-play and online tables are all this one model; they differ by their seats and their kind.
//!
//! Ports the game flow of Java's `BotGameService` and `PrivateTableService`.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

use beelot_game::bidding::BiddingState;
use beelot_game::board::GameBoard;
use beelot_game::bot::{BotStrategy, strategies, turns};
use beelot_game::deck::Deck;
use beelot_game::match_score::MatchScore;
use beelot_game::{Card, Difficulty, GameError, RandomSource, Rank, SplitMix64};
use time::OffsetDateTime;
use uuid::Uuid;

use super::OsRandom;
use super::actions::Action;
use super::finished::{DELETED_PLAYER, FinishedMatch, FinishedSeat, Mode};
use super::messages as msg;
use super::reactions::{Reaction, Reactions};
use super::seats::{Connection, Lobby, Seat, SeatKind, Status, TableKind};
use crate::error::AppError;

/// Makes the random source of each new deal.
pub type RngFactory = Arc<dyn Fn() -> Box<dyn RandomSource + Send> + Send + Sync>;

/// How a table deals and which strategy its bots play. Production deals from the operating system's generator.
#[derive(Clone)]
pub struct TableSetup {
    rng: RngFactory,
    strategy: Option<&'static dyn BotStrategy>,
    reactions: Arc<AtomicU64>,
}

impl Default for TableSetup {
    fn default() -> Self {
        Self {
            rng: Arc::new(|| Box::new(OsRandom)),
            strategy: None,
            reactions: Arc::default(),
        }
    }
}

impl TableSetup {
    /// Deals from generators seeded from `seed`, one per deal, so tests replay the same deals.
    pub fn seeded(seed: i64) -> Self {
        let deals = Arc::new(AtomicI64::new(0));
        Self {
            rng: Arc::new(move || {
                let deal = deals.fetch_add(1, Ordering::Relaxed);
                Box::new(SplitMix64::new(
                    seed.wrapping_mul(1_000_003).wrapping_add(deal),
                ))
            }),
            strategy: None,
            reactions: Arc::default(),
        }
    }

    /// Reactions take their sequence numbers from this counter, shared by every table.
    pub fn with_reaction_sequence(mut self, sequence: Arc<AtomicU64>) -> Self {
        self.reactions = sequence;
        self
    }

    /// The bots play this strategy instead of the table's own.
    pub fn with_strategy(mut self, strategy: &'static dyn BotStrategy) -> Self {
        self.strategy = Some(strategy);
        self
    }
}

/// Who acts: an online player in their seat, or the one device of a local table, which acts for the human whose
/// turn it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Actor {
    Seat(usize),
    Device,
}

/// What an accepted change leaves for the caller: a match to hand to the history, or a table to remove.
#[derive(Debug, Default)]
pub struct Outcome {
    pub finished: Option<FinishedMatch>,
    pub removed: bool,
}

/// The first dealer of a match: the seat before North, so North speaks first.
const FIRST_DEALER: usize = 3;

pub struct Table {
    pub id: Uuid,
    pub kind: TableKind,
    pub difficulty: Option<Difficulty>,
    pub invitation_code: Option<String>,
    lobby: Lobby,
    bidding: Option<BiddingState>,
    board: Option<GameBoard>,
    match_score: Option<MatchScore>,
    match_id: Uuid,
    round_recorded: bool,
    /// The human in charge of a local device.
    viewer: usize,
    /// Changes with every deal, redeals included.
    auction_id: Uuid,
    auction_deals: u32,
    /// The completed-trick count and since when that trick has been shown, at an online table.
    trick_shown: Option<(usize, OffsetDateTime)>,
    version: u64,
    last_activity: OffsetDateTime,
    rng: RngFactory,
    strategy: &'static dyn BotStrategy,
    reactions: Reactions,
    pending: Outcome,
}

impl std::fmt::Debug for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Table")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("lobby", &self.lobby)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

fn conflict(code: &'static str, message: &str) -> AppError {
    AppError::conflict(code, message)
}

fn bot_failed(error: GameError) -> AppError {
    AppError::Internal(anyhow::anyhow!("a bot made an illegal move: {error}"))
}

fn auction_error(error: GameError) -> AppError {
    let message = match error {
        GameError::NotYourTurn => msg::NOT_YOUR_TURN_TO_BID,
        GameError::ContractOnlyInContree => msg::CONTRACT_ONLY_IN_CONTREE,
        GameError::TrumpChoiceOnlyInClassic => msg::TRUMP_CHOICE_ONLY_IN_CLASSIC,
        GameError::AuctionEnded => msg::AUCTION_ENDED,
        GameError::MustTakeUpturnedSuit => msg::MUST_TAKE_UPTURNED_SUIT,
        GameError::MustChooseAnotherSuit => msg::MUST_CHOOSE_ANOTHER_SUIT,
        GameError::InvalidBid => msg::INVALID_BID,
        GameError::BidTooLow => msg::BID_TOO_LOW,
        GameError::CoincheNotAllowed => msg::COINCHE_NOT_ALLOWED,
        GameError::AllTrumpsNotPlayed => msg::ALL_TRUMPS_NOT_PLAYED,
        other => return board_error(other),
    };
    conflict(error.code(), message)
}

fn board_error(error: GameError) -> AppError {
    let message = match error {
        GameError::NotYourTurn => msg::NOT_YOUR_TURN_TO_PLAY,
        GameError::IllegalCard => msg::ILLEGAL_CARD,
        GameError::RoundEnded => msg::ROUND_ENDED,
        GameError::NoCompletedTrick => msg::NO_COMPLETED_TRICK,
        other => return auction_error(other),
    };
    conflict(error.code(), message)
}

impl Table {
    fn new(
        id: Uuid,
        kind: TableKind,
        difficulty: Option<Difficulty>,
        invitation_code: Option<String>,
        lobby: Lobby,
        setup: TableSetup,
        now: OffsetDateTime,
    ) -> Self {
        let strategy = setup.strategy.unwrap_or_else(|| match difficulty {
            Some(difficulty) => strategies::for_difficulty(difficulty),
            None => strategies::for_tables(),
        });
        let viewer = lobby
            .occupied()
            .find(|(_, seat)| seat.kind == SeatKind::Human)
            .map_or(0, |(position, _)| position);
        Self {
            id,
            kind,
            difficulty,
            invitation_code,
            lobby,
            bidding: None,
            board: None,
            match_score: None,
            match_id: Uuid::new_v4(),
            round_recorded: false,
            viewer,
            auction_id: Uuid::new_v4(),
            auction_deals: 0,
            trick_shown: None,
            version: 0,
            last_activity: now,
            rng: setup.rng,
            strategy,
            reactions: Reactions::new(setup.reactions),
            pending: Outcome::default(),
        }
    }

    /// A solo or pass-and-play table, dealt at once; its bots play the difficulty's strategy.
    pub fn local(
        id: Uuid,
        difficulty: Difficulty,
        lobby: Lobby,
        setup: TableSetup,
        now: OffsetDateTime,
    ) -> Self {
        let mut table = Self::new(
            id,
            TableKind::Local,
            Some(difficulty),
            None,
            lobby,
            setup,
            now,
        );
        table
            .begin_match(now)
            .expect("the first deal of a local table");
        table
    }

    /// A private or public table, waiting for players; its bots play the table strategy.
    pub fn online(
        id: Uuid,
        kind: TableKind,
        invitation_code: Option<String>,
        lobby: Lobby,
        setup: TableSetup,
        now: OffsetDateTime,
    ) -> Self {
        Self::new(id, kind, None, invitation_code, lobby, setup, now)
    }

    pub fn lobby(&self) -> &Lobby {
        &self.lobby
    }

    pub fn bidding(&self) -> Option<&BiddingState> {
        self.bidding.as_ref()
    }

    pub fn board(&self) -> Option<&GameBoard> {
        self.board.as_ref()
    }

    pub fn match_score(&self) -> Option<&MatchScore> {
        self.match_score.as_ref()
    }

    pub fn auction_id(&self) -> Uuid {
        self.auction_id
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn last_activity(&self) -> OffsetDateTime {
        self.last_activity
    }

    /// The reactions sent at the table in the last 30 seconds, oldest first.
    pub fn reactions(&self, now: OffsetDateTime) -> Vec<Reaction> {
        self.reactions.recent(now)
    }

    pub fn strategy(&self) -> &'static dyn BotStrategy {
        self.strategy
    }

    /// Whether several humans share the device of a local table.
    pub fn pass_and_play(&self) -> bool {
        self.kind == TableKind::Local
            && self
                .lobby
                .occupied()
                .filter(|(_, seat)| seat.kind == SeatKind::Human)
                .count()
                > 1
    }

    /// Whether a bot that searches sampled deals, which takes a while, may move during the next change.
    pub fn needs_blocking_bot(&self) -> bool {
        std::ptr::addr_eq(
            self.strategy,
            strategies::for_difficulty(Difficulty::Challenging),
        ) && self.lobby.occupied().any(|(_, seat)| seat.played_by_bot())
    }

    /// Marks the table as used now, so it isn't evicted as idle.
    pub fn touch(&mut self, now: OffsetDateTime) {
        self.last_activity = now;
    }

    fn changed(&mut self) -> Outcome {
        self.version += 1;
        std::mem::take(&mut self.pending)
    }

    /// Applies a player's action. Every accepted change is a new version; a refusal or a no-op changes nothing.
    pub fn apply(
        &mut self,
        actor: Actor,
        action: Action,
        now: OffsetDateTime,
    ) -> Result<Outcome, AppError> {
        let changed = self.dispatch(actor, action, now)?;
        self.last_activity = now;
        if !changed {
            return Ok(Outcome::default());
        }
        self.refresh_viewer();
        Ok(self.changed())
    }

    fn seat_of(&self, actor: Actor) -> Result<usize, AppError> {
        match (actor, self.kind) {
            (Actor::Seat(position), TableKind::Private | TableKind::Public) => Ok(position),
            (Actor::Device, TableKind::Local) => Ok(self.viewer),
            _ => Err(AppError::table_unauthorized()),
        }
    }

    /// The seat that makes a move: an online player's own, or the human whose turn it is on a local device.
    fn mover(&self, actor: Actor, active: usize) -> Result<usize, AppError> {
        match actor {
            Actor::Device => {
                self.seat_of(actor)?;
                match self.lobby.seat(active) {
                    Some(seat) if !seat.played_by_bot() => Ok(active),
                    _ => Err(conflict("not-your-turn", msg::NOT_YOUR_TURN)),
                }
            }
            Actor::Seat(_) => self.seat_of(actor),
        }
    }

    fn dispatch(
        &mut self,
        actor: Actor,
        action: Action,
        now: OffsetDateTime,
    ) -> Result<bool, AppError> {
        match action {
            Action::Ready { ready } => {
                let seat = self.seat_of(actor)?;
                self.lobby.set_ready(seat, ready)?;
            }
            Action::Seat { position } => {
                let seat = self.seat_of(actor)?;
                self.lobby.choose_seat(seat, position)?;
            }
            Action::Start { fill_with_bots } => {
                let seat = self.seat_of(actor)?;
                if fill_with_bots {
                    self.lobby.start_with_bots(seat)?;
                } else {
                    self.lobby.start(seat)?;
                }
                self.begin_match(now)?;
            }
            Action::Settings {
                announcements,
                all_trumps,
                turn_timer,
                target,
            } => {
                let seat = self.seat_of(actor)?;
                if announcements.is_none()
                    && all_trumps.is_none()
                    && turn_timer.is_none()
                    && target.is_none()
                {
                    return Ok(false);
                }
                // Every setting is checked before any changes.
                let mut lobby = self.lobby.clone();
                if let Some(on) = announcements {
                    lobby.set_announcements(seat, on)?;
                }
                if let Some(on) = all_trumps {
                    lobby.set_all_trumps(seat, on)?;
                }
                if let Some(seconds) = turn_timer {
                    lobby.set_turn_timer(seat, seconds)?;
                }
                if let Some(target) = target {
                    lobby.set_target(seat, target)?;
                }
                self.lobby = lobby;
            }
            Action::Pass => self.auction(actor, now, |bidding, seat| bidding.pass(seat))?,
            Action::Trump { suit } => {
                self.auction(actor, now, |bidding, seat| bidding.choose_trump(seat, suit))?
            }
            Action::Contract { value, suit } => {
                let suit = suit.ok_or_else(|| conflict("suit-required", msg::SUIT_REQUIRED))?;
                self.auction(actor, now, |bidding, seat| bidding.bid(seat, value, suit))?
            }
            Action::Coinche => self.auction(actor, now, |bidding, seat| bidding.coinche(seat))?,
            Action::Card {
                rank,
                suit,
                declare,
            } => {
                let board = self.started_board()?;
                let seat = self.mover(actor, board.active_seat())?;
                let rank = Rank::ALL
                    .into_iter()
                    .find(|candidate| candidate.label() == rank)
                    .ok_or_else(|| board_error(GameError::IllegalCard))?;
                let board = self.board.as_mut().expect("checked above");
                board
                    .play(seat, Card::new(rank, suit), declare.unwrap_or(false))
                    .map_err(board_error)?;
                self.drive_bots(now)?;
            }
            Action::ContinueTrick => {
                self.seat_of(actor)?;
                self.started_board()?;
                let board = self.board.as_mut().expect("checked above");
                board.continue_after_trick().map_err(board_error)?;
                self.drive_bots(now)?;
            }
            Action::NextRound => {
                self.seat_of(actor)?;
                let score = self
                    .match_score
                    .as_ref()
                    .ok_or_else(|| conflict("not-started", msg::NOT_STARTED))?;
                if score.complete() {
                    return Err(conflict("match-ended", msg::MATCH_ENDED));
                }
                return self.deal_next_round(now, true);
            }
            Action::Rematch => {
                self.seat_of(actor)?;
                let score = self
                    .match_score
                    .as_ref()
                    .ok_or_else(|| conflict("not-started", msg::NOT_STARTED))?;
                if !score.complete() {
                    return Ok(false);
                }
                self.match_score = Some(self.new_match_score());
                self.match_id = Uuid::new_v4();
                self.deal_next_round(now, false)?;
            }
            Action::React { reaction } => {
                let seat = self.seat_of(actor)?;
                if self.kind == TableKind::Local || self.lobby.status() != Status::InProgress {
                    return Err(conflict("reactions-closed", msg::REACTIONS_CLOSED));
                }
                self.reactions.send(seat, reaction, now)?;
            }
            Action::Leave => {
                let seat = self.seat_of(actor)?;
                if self.kind == TableKind::Local {
                    self.pending.removed = true;
                } else {
                    self.lobby.leave(seat)?;
                    self.pending.removed = self.lobby.is_empty();
                }
            }
        }
        Ok(true)
    }

    /// A call in the auction, from the seat that makes it; the bots then take their turns.
    fn auction(
        &mut self,
        actor: Actor,
        now: OffsetDateTime,
        call: impl FnOnce(&mut BiddingState, usize) -> Result<(), GameError>,
    ) -> Result<(), AppError> {
        let bidding = self
            .bidding
            .as_ref()
            .ok_or_else(|| conflict("not-started", msg::NOT_STARTED))?;
        if self.board.is_some() {
            return Err(auction_error(GameError::AuctionEnded));
        }
        let seat = self.mover(actor, bidding.active_seat())?;
        call(self.bidding.as_mut().expect("checked above"), seat).map_err(auction_error)?;
        self.drive_bots(now)
    }

    fn started_board(&self) -> Result<&GameBoard, AppError> {
        if self.bidding.is_none() {
            return Err(conflict("not-started", msg::NOT_STARTED));
        }
        self.board
            .as_ref()
            .ok_or_else(|| conflict("auction-not-finished", msg::AUCTION_NOT_FINISHED))
    }

    /// A match to the table's target: the next round and a rematch keep it.
    fn new_match_score(&self) -> MatchScore {
        MatchScore::to(self.lobby.target()).expect("the lobby holds a valid target")
    }

    fn begin_match(&mut self, now: OffsetDateTime) -> Result<(), AppError> {
        self.match_score = Some(self.new_match_score());
        self.match_id = Uuid::new_v4();
        self.deal(FIRST_DEALER, None);
        self.drive_bots(now)
    }

    /// Deals the next round once the current one is finished. Within a match, its cards are gathered without a
    /// shuffle (US-085); a rematch starts from a shuffled deck. Several players may click at once, so a call made
    /// after the new deal has already happened is a no-op.
    fn deal_next_round(&mut self, now: OffsetDateTime, same_match: bool) -> Result<bool, AppError> {
        let Some(board) = &self.board else {
            return Ok(false);
        };
        if board.round_result().is_none() {
            return Err(conflict("round-not-finished", msg::ROUND_NOT_FINISHED));
        }
        let dealer = (self
            .bidding
            .as_ref()
            .map_or(FIRST_DEALER, BiddingState::dealer)
            + 1)
            % 4;
        let deck = if same_match {
            board.gathered_deck(dealer)
        } else {
            None
        };
        self.deal(dealer, deck);
        self.drive_bots(now)?;
        Ok(true)
    }

    /// Every deal of the table keeps its variant and its options. A match's first deal has no deck and is shuffled.
    fn deal(&mut self, dealer: usize, deck: Option<Deck>) {
        let bidding = BiddingState::dealt_from(self.lobby.variant(), dealer, (self.rng)(), deck)
            .with_announcements(self.lobby.announcements())
            .with_all_trumps(self.lobby.all_trumps());
        self.auction_deals = bidding.deals();
        self.bidding = Some(bidding);
        self.auction_id = Uuid::new_v4();
        self.board = None;
        self.round_recorded = false;
        self.trick_shown = None;
    }

    /// Bots take their turns until a human's turn, a completed trick or the end of the round; a finished round is
    /// then added to the match score once.
    fn drive_bots(&mut self, now: OffsetDateTime) -> Result<(), AppError> {
        if self.board.is_none()
            && let Some(bidding) = self.bidding.as_mut()
        {
            while bidding.completed_board().is_none()
                && self
                    .lobby
                    .seat(bidding.active_seat())
                    .is_some_and(Seat::played_by_bot)
            {
                turns::take_auction_turn(bidding, self.strategy).map_err(bot_failed)?;
            }
            if bidding.deals() != self.auction_deals {
                self.auction_deals = bidding.deals();
                self.auction_id = Uuid::new_v4();
            }
            self.board = bidding.completed_board().cloned();
        }
        if let Some(board) = self.board.as_mut() {
            while board.round_result().is_none()
                && !board.reviewing_completed_trick()
                && self
                    .lobby
                    .seat(board.active_seat())
                    .is_some_and(Seat::played_by_bot)
            {
                turns::play_turn(board, self.strategy).map_err(bot_failed)?;
            }
        }
        self.stamp_trick_review(now);
        self.record_round_if_complete(now);
        Ok(())
    }

    fn stamp_trick_review(&mut self, now: OffsetDateTime) {
        let shown = self.board.as_ref().filter(|board| {
            self.kind != TableKind::Local
                && board.reviewing_completed_trick()
                && board.round_result().is_none()
        });
        self.trick_shown = match (shown, self.trick_shown) {
            (Some(board), Some((count, since))) if count == board.completed_tricks() => {
                Some((count, since))
            }
            (Some(board), _) => Some((board.completed_tricks(), now)),
            (None, _) => None,
        };
    }

    /// Adds a finished round to the match score once, and hands the match over when it ends. Pass-and-play matches
    /// are never handed over: their players cannot be told apart.
    fn record_round_if_complete(&mut self, now: OffsetDateTime) {
        if self.round_recorded {
            return;
        }
        let (Some(board), Some(score)) = (&self.board, self.match_score.as_mut()) else {
            return;
        };
        let Some(round) = board.round_summary() else {
            return;
        };
        self.round_recorded = true;
        if score.record_summary(round) && !self.pass_and_play() {
            self.pending.finished = Some(self.finished_match(now));
        }
    }

    fn finished_match(&self, now: OffsetDateTime) -> FinishedMatch {
        let score = self.match_score.as_ref().expect("a match was played");
        FinishedMatch {
            id: self.match_id,
            ended_at: now,
            mode: match self.kind {
                TableKind::Local => Mode::Solo,
                TableKind::Private => Mode::Private,
                TableKind::Public => Mode::Public,
            },
            variant: self.lobby.variant(),
            difficulty: self.difficulty,
            north_south_score: score.north_south(),
            east_west_score: score.east_west(),
            winning_team: score.winner().expect("a complete match has a winner"),
            seats: self
                .lobby
                .occupied()
                .map(|(_, seat)| FinishedSeat {
                    account: seat.account,
                    name: seat.name.clone(),
                    bot: seat.played_by_bot(),
                })
                .collect(),
            rounds: score.rounds().to_vec(),
            announcements: self.lobby.announcements(),
            all_trumps: self.lobby.all_trumps(),
            target: score.target(),
        }
    }

    /// The human in charge of a local device: the active player when that is a human and no completed trick is on
    /// the table, otherwise the human who was in charge before.
    pub fn viewer(&self) -> usize {
        let active = match (&self.board, &self.bidding) {
            (Some(board), _) => board.active_seat(),
            (None, Some(bidding)) => bidding.active_seat(),
            (None, None) => return self.viewer,
        };
        let trick_shown = self.board.as_ref().is_some_and(|board| {
            board.round_result().is_some() || board.reviewing_completed_trick()
        });
        let human = self
            .lobby
            .seat(active)
            .is_some_and(|seat| seat.kind == SeatKind::Human);
        if human && !trick_shown {
            active
        } else {
            self.viewer
        }
    }

    fn refresh_viewer(&mut self) {
        self.viewer = self.viewer();
    }

    /// Seats a player at an online table; the fourth player at a public table starts the match.
    pub fn join(
        &mut self,
        name: String,
        account: Option<Uuid>,
        now: OffsetDateTime,
    ) -> Result<usize, AppError> {
        let position = self.lobby.join(name, account)?;
        self.last_activity = now;
        if self.lobby.status() == Status::InProgress {
            self.begin_match(now)?;
        }
        self.changed();
        Ok(position)
    }

    /// Starts a waiting public table with bots once its deadline has passed; true when it did.
    pub fn fill_with_bots_if_due(&mut self, now: OffsetDateTime) -> bool {
        if !self.lobby.fill_with_bots_if_due(now) {
            return false;
        }
        if let Err(error) = self.begin_match(now) {
            tracing::warn!(table = %self.id, %error, "the bots could not begin the match");
        }
        self.changed();
        true
    }

    pub fn disconnect(&mut self, position: usize, now: OffsetDateTime) -> Result<(), AppError> {
        self.lobby.disconnect(position, now)?;
        self.changed();
        Ok(())
    }

    /// Marks the seat connected again; a seat already connected changes nothing.
    pub fn reconnect(&mut self, position: usize) -> Result<(), AppError> {
        if self
            .lobby
            .seat(position)
            .is_some_and(|seat| seat.connection == Connection::Connected)
        {
            return Ok(());
        }
        self.lobby.reconnect(position)?;
        self.changed();
        Ok(())
    }

    /// Hands to bots the seats disconnected for at least `timeout`; the bots then play any turn they now hold.
    pub fn replace_expired_disconnections(
        &mut self,
        now: OffsetDateTime,
        timeout: Duration,
    ) -> Option<Outcome> {
        if !self.lobby.replace_expired_disconnections(now, timeout) {
            return None;
        }
        if let Err(error) = self.drive_bots(now) {
            tracing::warn!(table = %self.id, %error, "the bots could not take over");
        }
        Some(self.changed())
    }

    /// While an online table shows a completed trick for at least `review`, the count of completed tricks.
    pub fn trick_review_due(&self, now: OffsetDateTime, review: Duration) -> Option<usize> {
        let (count, since) = self.trick_shown?;
        let board = self.board.as_ref()?;
        let shown = board.reviewing_completed_trick()
            && board.round_result().is_none()
            && board.completed_tricks() == count;
        (shown && since + review <= now).then_some(count)
    }

    /// Collects the completed trick, unless another one is shown by now; the bots then play up to a human's turn.
    pub fn collect_trick(
        &mut self,
        expected_tricks: usize,
        now: OffsetDateTime,
    ) -> Option<Outcome> {
        let board = self.board.as_mut()?;
        if !board.reviewing_completed_trick() || board.completed_tricks() != expected_tricks {
            return None;
        }
        board.continue_after_trick().ok()?;
        if let Err(error) = self.drive_bots(now) {
            tracing::warn!(table = %self.id, %error, "the bots could not play after the trick");
        }
        Some(self.changed())
    }

    /// Takes a deleted account away from the table: it leaves a table still waiting for players, and a bot plays its
    /// seat, as "Deleted player", in a match. The match finished afterwards is recorded without it.
    pub fn release_account(&mut self, account: Uuid, now: OffsetDateTime) -> Outcome {
        let seats: Vec<usize> = self
            .lobby
            .occupied()
            .filter(|(_, seat)| seat.account == Some(account))
            .map(|(position, _)| position)
            .collect();
        if seats.is_empty() {
            return Outcome::default();
        }
        for position in seats {
            if self.lobby.status() == Status::WaitingForPlayers {
                self.lobby
                    .leave(position)
                    .expect("a waiting seat can leave");
            } else {
                let seat = self.lobby.seat_mut(position).expect("an occupied seat");
                seat.account = None;
                if self.kind != TableKind::Local {
                    self.lobby
                        .hand_to_bot(position, DELETED_PLAYER)
                        .expect("an occupied seat");
                }
            }
        }
        self.pending.removed = self.lobby.is_empty();
        if self.lobby.status() == Status::InProgress
            && let Err(error) = self.drive_bots(now)
        {
            tracing::warn!(table = %self.id, %error, "the bots could not take over");
        }
        self.changed()
    }
}
