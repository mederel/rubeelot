//! Quick matches: players are seated at the public tables of their variant; a table starts when full, or with bots
//! once its wait is over.

use beelot_game::Variant;
use time::OffsetDateTime;
use uuid::Uuid;

use super::messages as msg;
use super::registry::{Access, TableRegistry};
use super::{Status, TableKind, required_name};
use crate::error::AppError;

impl TableRegistry {
    /// The public tables of the variant still waiting for players, fullest first, then those whose bots come first.
    pub fn open_public_tables(&self, variant: Variant) -> Vec<Uuid> {
        let mut open: Vec<(usize, OffsetDateTime, Uuid)> = Vec::new();
        for id in self.public_ids() {
            let Some(handle) = self.handle(id) else {
                continue;
            };
            let table = handle.state.lock().unwrap();
            let lobby = table.lobby();
            let seated = lobby.occupied().count();
            if lobby.variant() == variant
                && lobby.status() == Status::WaitingForPlayers
                && seated < 4
                && let Some(bot_fill_at) = lobby.bot_fill_at()
            {
                open.push((seated, bot_fill_at, id));
            }
        }
        open.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        open.into_iter().map(|(_, _, id)| id).collect()
    }

    /// Seats a player at that public table; the fourth player starts the match.
    pub fn join_public(
        &self,
        id: Uuid,
        name: &str,
        account: Option<Uuid>,
    ) -> Result<Access, AppError> {
        let name = required_name(name)?;
        let handle = self
            .handle(id)
            .filter(|handle| handle.kind == TableKind::Public)
            .ok_or_else(|| AppError::conflict("table-closed", msg::TABLE_CLOSED))?;
        self.seat(&handle, id, name, account)
    }

    /// Seats the player at the fullest open table of the variant, or opens a new one.
    pub fn quick_match(
        &self,
        name: &str,
        variant: Variant,
        account: Option<Uuid>,
    ) -> Result<Access, AppError> {
        let name = required_name(name)?;
        let _one_at_a_time = self.matchmaking.lock().unwrap();
        for id in self.open_public_tables(variant) {
            // Bots may have filled the table meanwhile.
            if let Ok(access) = self.join_public(id, &name, account) {
                return Ok(access);
            }
        }
        self.open_public(&name, variant, account)
    }
}
