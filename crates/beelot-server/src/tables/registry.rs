//! Every table in memory, with the player tokens that act at it and the channel that announces its changes. Ports
//! the registry parts of Java's `PrivateTableService` and `BotGameService`.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use beelot_game::{Difficulty, Variant};
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::sync::watch;
use uuid::Uuid;

use super::messages as msg;
use super::{
    Action, Actor, Lobby, Outcome, Table, TableKind, TableSetup, codes, local_seats, required_name,
};
use crate::api::{Snapshot, Viewer, snapshot};
use crate::clock::Clock;
use crate::config::Limits;
use crate::error::AppError;

/// The SHA-256 of a player token: the registry never keeps a token itself.
pub type TokenHash = [u8; 32];

fn hash(token: &str) -> TokenHash {
    Sha256::digest(token.as_bytes()).into()
}

/// 32 random bytes in URL-safe base64.
fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system's random generator");
    URL_SAFE_NO_PAD.encode(bytes)
}

/// What a player gets on creating or joining a table: the token that acts at it, and a private table's code.
#[derive(Clone, PartialEq, Eq)]
pub struct Access {
    pub table_id: Uuid,
    pub player_token: String,
    pub code: Option<String>,
}

impl std::fmt::Debug for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Access")
            .field("table_id", &self.table_id)
            .field("player_token", &"<redacted>")
            .field("code", &self.code)
            .finish()
    }
}

pub(crate) fn viewer_of(actor: Actor) -> Viewer {
    match actor {
        Actor::Seat(position) => Viewer::Seat(position),
        Actor::Device => Viewer::Device,
    }
}

/// One table, the hashes of the tokens that act at it, and its open event streams.
#[derive(Debug)]
pub struct TableHandle {
    pub kind: TableKind,
    pub state: Mutex<Table>,
    /// The table's version after each change.
    pub changes: watch::Sender<u64>,
    tokens: Mutex<HashMap<TokenHash, Actor>>,
    /// The open event streams of each seat.
    pub(crate) streams: Mutex<[u32; 4]>,
}

impl TableHandle {
    fn new(table: Table) -> Self {
        let (changes, _) = watch::channel(table.version());
        Self {
            kind: table.kind,
            state: Mutex::new(table),
            changes,
            tokens: Mutex::new(HashMap::new()),
            streams: Mutex::new([0; 4]),
        }
    }

    /// Tells the table's streams about its new version.
    pub fn publish(&self, version: u64) {
        self.changes.send_if_modified(|current| {
            let newer = version > *current;
            if newer {
                *current = version;
            }
            newer
        });
    }

    fn issue_token(&self, actor: Actor) -> String {
        let token = new_token();
        self.tokens.lock().unwrap().insert(hash(&token), actor);
        token
    }

    fn actor(&self, token: &str) -> Option<Actor> {
        self.tokens.lock().unwrap().get(&hash(token)).copied()
    }

    fn revoke(&self, actor: Actor) {
        self.tokens.lock().unwrap().retain(|_, held| *held != actor);
    }

    fn move_tokens(&self, from: usize, to: usize) {
        for held in self.tokens.lock().unwrap().values_mut() {
            if *held == Actor::Seat(from) {
                *held = Actor::Seat(to);
            }
        }
    }

    /// Applies a player's action and returns what it leaves, with the table as that player now sees it. A player who
    /// moves keeps their token; one who leaves loses it.
    pub fn act(
        &self,
        actor: Actor,
        action: Action,
        now: OffsetDateTime,
    ) -> Result<(Outcome, Snapshot), AppError> {
        let mut table = self.state.lock().unwrap();
        let moved_to = match action {
            Action::Seat { position } => Some(position),
            _ => None,
        };
        let leaving = action == Action::Leave;
        let outcome = table.apply(actor, action, now)?;
        let mut viewer = actor;
        if let (Actor::Seat(from), Some(to)) = (actor, moved_to) {
            self.move_tokens(from, to);
            viewer = Actor::Seat(to);
        }
        if leaving {
            self.revoke(actor);
        }
        let view = snapshot(&table, viewer_of(viewer), now);
        let version = table.version();
        drop(table);
        self.publish(version);
        Ok((outcome, view))
    }
}

pub struct TableRegistry {
    tables: DashMap<Uuid, Arc<TableHandle>>,
    codes: DashMap<String, Uuid>,
    /// Quick matches run one at a time, so parallel players fill the same tables.
    pub(super) matchmaking: Mutex<()>,
    clock: Arc<dyn Clock>,
    max_local: usize,
    max_online: usize,
    idle_expiry: Duration,
    pub(super) bot_fill_wait: Duration,
    reconnect_timeout: Duration,
    trick_review: Duration,
    reaction_sequence: Arc<AtomicU64>,
    setup: Box<dyn Fn() -> TableSetup + Send + Sync>,
}

impl std::fmt::Debug for TableRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableRegistry")
            .field("tables", &self.tables)
            .field("codes", &self.codes)
            .finish_non_exhaustive()
    }
}

impl TableRegistry {
    pub fn new(limits: &Limits, clock: Arc<dyn Clock>) -> Self {
        Self {
            tables: DashMap::new(),
            codes: DashMap::new(),
            matchmaking: Mutex::new(()),
            clock,
            max_local: limits.max_local_tables,
            max_online: limits.max_online_tables,
            idle_expiry: limits.idle_expiry,
            bot_fill_wait: limits.bot_fill_wait,
            reconnect_timeout: limits.reconnect_timeout,
            trick_review: limits.trick_review,
            reaction_sequence: Arc::default(),
            setup: Box::new(TableSetup::default),
        }
    }

    /// Tables are set up by `setup`: tests deal from seeded generators.
    pub fn with_setup(mut self, setup: impl Fn() -> TableSetup + Send + Sync + 'static) -> Self {
        self.setup = Box::new(setup);
        self
    }

    pub fn now(&self) -> OffsetDateTime {
        self.clock.now()
    }

    fn table_setup(&self) -> TableSetup {
        (self.setup)().with_reaction_sequence(self.reaction_sequence.clone())
    }

    pub fn handle(&self, id: Uuid) -> Option<Arc<TableHandle>> {
        self.tables.get(&id).map(|entry| entry.value().clone())
    }

    pub(super) fn public_ids(&self) -> Vec<Uuid> {
        self.tables
            .iter()
            .filter(|entry| entry.value().kind == TableKind::Public)
            .map(|entry| *entry.key())
            .collect()
    }

    fn handles(&self) -> Vec<Arc<TableHandle>> {
        self.tables
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// The table and the actor a token acts as there. An unknown table is not found; a token that doesn't act at
    /// it is unauthorized.
    pub fn authorize(&self, id: Uuid, token: &str) -> Result<(Arc<TableHandle>, Actor), AppError> {
        let handle = self.handle(id).ok_or(AppError::NotFound)?;
        let actor = handle
            .actor(token)
            .ok_or_else(AppError::table_unauthorized)?;
        handle.state.lock().unwrap().touch(self.now());
        Ok((handle, actor))
    }

    /// The local and the online tables in memory.
    pub fn counts(&self) -> (usize, usize) {
        let local = self
            .tables
            .iter()
            .filter(|entry| entry.value().kind == TableKind::Local)
            .count();
        (local, self.tables.len() - local)
    }

    /// Makes room for one more table of that kind, evicting idle tables first.
    fn ensure_capacity(&self, local: bool) -> Result<(), AppError> {
        let full = |registry: &Self| {
            let (locals, online) = registry.counts();
            if local {
                locals >= registry.max_local
            } else {
                online >= registry.max_online
            }
        };
        if full(self) {
            self.evict_idle(self.now());
        }
        if full(self) {
            return Err(AppError::CapacityExceeded);
        }
        Ok(())
    }

    fn insert(&self, table: Table) -> Arc<TableHandle> {
        if let Some(code) = &table.invitation_code {
            self.codes.insert(code.clone(), table.id);
        }
        let handle = Arc::new(TableHandle::new(table));
        let id = handle.state.lock().unwrap().id;
        self.tables.insert(id, handle.clone());
        handle
    }

    /// Applies a player's action at the table, and removes the table when the action leaves it empty.
    pub fn act(
        &self,
        handle: &TableHandle,
        actor: Actor,
        action: Action,
    ) -> Result<(Outcome, Snapshot), AppError> {
        let id = handle.state.lock().unwrap().id;
        let (outcome, view) = handle.act(actor, action, self.now())?;
        if outcome.removed {
            self.remove(id);
        }
        Ok((outcome, view))
    }

    pub fn remove(&self, id: Uuid) {
        if let Some((_, handle)) = self.tables.remove(&id) {
            let code = handle.state.lock().unwrap().invitation_code.clone();
            if let Some(code) = code {
                self.codes.remove(&code);
            }
        }
    }

    /// A solo table, or a pass-and-play one when `seats` names the humans. A solo seat belongs to `account`;
    /// pass-and-play seats belong to nobody, since the players cannot be told apart.
    pub fn create_local(
        &self,
        variant: Variant,
        difficulty: Difficulty,
        announcements: bool,
        seats: Option<Vec<Option<String>>>,
        account: Option<Uuid>,
    ) -> Result<Access, AppError> {
        let mut seated = local_seats(seats.as_deref())?;
        if seats.is_none() {
            seated[0].account = account;
        }
        self.ensure_capacity(true)?;
        let table = Table::local(
            Uuid::new_v4(),
            variant,
            difficulty,
            announcements,
            seated,
            self.table_setup(),
            self.now(),
        );
        let handle = self.insert(table);
        let table_id = handle.state.lock().unwrap().id;
        Ok(Access {
            table_id,
            player_token: handle.issue_token(Actor::Device),
            code: None,
        })
    }

    /// Opens a private table, joined with its invitation code; the owner sits North.
    pub fn create_private(
        &self,
        name: &str,
        variant: Variant,
        account: Option<Uuid>,
    ) -> Result<Access, AppError> {
        let name = required_name(name)?;
        self.ensure_capacity(false)?;
        let code = codes::new_code(|code| self.codes.contains_key(code));
        let lobby = Lobby::private(name, account, variant);
        let id = Uuid::new_v4();
        let table = Table::online(
            id,
            TableKind::Private,
            Some(code.clone()),
            lobby,
            self.table_setup(),
            self.now(),
        );
        let handle = self.insert(table);
        Ok(Access {
            table_id: id,
            player_token: handle.issue_token(Actor::Seat(0)),
            code: Some(code),
        })
    }

    /// Seats a player at the private table of that invitation code.
    pub fn join(&self, code: &str, name: &str, account: Option<Uuid>) -> Result<Access, AppError> {
        let invalid = || AppError::conflict("invalid-code", msg::INVALID_CODE);
        let code = codes::normalized(code).ok_or_else(invalid)?;
        let id = self
            .codes
            .get(&code)
            .map(|entry| *entry)
            .ok_or_else(invalid)?;
        let handle = self.handle(id).ok_or_else(invalid)?;
        let name = required_name(name)?;
        self.seat(&handle, id, name, account)
    }

    /// Seats a player at a table that is still waiting, and gives them their token.
    pub(super) fn seat(
        &self,
        handle: &TableHandle,
        id: Uuid,
        name: String,
        account: Option<Uuid>,
    ) -> Result<Access, AppError> {
        let mut table = handle.state.lock().unwrap();
        let position = table.join(name, account, self.now())?;
        let player_token = handle.issue_token(Actor::Seat(position));
        let version = table.version();
        drop(table);
        handle.publish(version);
        Ok(Access {
            table_id: id,
            player_token,
            code: None,
        })
    }

    /// Opens a public table: bots take its empty seats once the bot-fill wait has passed.
    pub fn open_public(
        &self,
        name: &str,
        variant: Variant,
        account: Option<Uuid>,
    ) -> Result<Access, AppError> {
        let name = required_name(name)?;
        self.ensure_capacity(false)?;
        let now = self.now();
        let lobby = Lobby::public(name, account, variant, now + self.bot_fill_wait);
        let id = Uuid::new_v4();
        let table = Table::online(id, TableKind::Public, None, lobby, self.table_setup(), now);
        let handle = self.insert(table);
        Ok(Access {
            table_id: id,
            player_token: handle.issue_token(Actor::Seat(0)),
            code: None,
        })
    }

    /// Removes the tables nobody has used for the idle expiry, freeing their memory.
    pub fn evict_idle(&self, now: OffsetDateTime) {
        let idle: Vec<Uuid> = self
            .handles()
            .into_iter()
            .filter_map(|handle| {
                let table = handle.state.lock().unwrap();
                (table.last_activity() + self.idle_expiry < now).then_some(table.id)
            })
            .collect();
        for id in idle {
            self.remove(id);
        }
    }

    /// The timed rules of the online tables: bots fill the public tables whose wait is over, take the seats
    /// disconnected for too long, and a trick shown long enough is collected. Returns what the changes left.
    pub fn sweep(&self, now: OffsetDateTime) -> Vec<Outcome> {
        let mut outcomes = Vec::new();
        for handle in self.handles() {
            if handle.kind == TableKind::Local {
                continue;
            }
            let mut table = handle.state.lock().unwrap();
            table.fill_with_bots_if_due(now);
            outcomes.extend(table.replace_expired_disconnections(now, self.reconnect_timeout));
            if let Some(count) = table.trick_review_due(now, self.trick_review) {
                outcomes.extend(table.collect_trick(count, now));
            }
            let version = table.version();
            drop(table);
            handle.publish(version);
        }
        outcomes
    }

    /// Takes a deleted account away from every table: its seats and tokens go, a bot plays its seats in a match, and
    /// a table it leaves empty is removed.
    pub fn release_account(&self, account: Uuid) -> Vec<Outcome> {
        let now = self.now();
        let mut outcomes = Vec::new();
        for handle in self.handles() {
            let mut table = handle.state.lock().unwrap();
            let seats: Vec<usize> = table
                .lobby()
                .occupied()
                .filter(|(_, seat)| seat.account == Some(account))
                .map(|(position, _)| position)
                .collect();
            if seats.is_empty() {
                continue;
            }
            for position in seats {
                handle.revoke(if table.kind == TableKind::Local {
                    Actor::Device
                } else {
                    Actor::Seat(position)
                });
            }
            let outcome = table.release_account(account, now);
            let (id, version) = (table.id, table.version());
            drop(table);
            handle.publish(version);
            if outcome.removed {
                self.remove(id);
            }
            outcomes.push(outcome);
        }
        outcomes
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
