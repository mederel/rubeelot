//! `GET /api/tables/{id}/events`: each player's stream of snapshots. An open stream is the seat's presence: the seat
//! is disconnected once its last stream closes, and connected again when one opens.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::stream::{self, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio_stream::wrappers::WatchStream;

use super::{bearer, table_id};
use crate::api::snapshot;
use crate::app::AppState;
use crate::clock::Clock;
use crate::error::AppError;
use crate::tables::registry::viewer_of;
use crate::tables::{Actor, TableHandle, TableKind};

const KEEP_ALIVE: Duration = Duration::from_secs(15);

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/tables/{id}/events", get(events))
}

/// `EventSource` cannot set headers, so the token may come in the query string, which is never logged.
#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

/// While it lives, the seat has one more open stream; dropping the last one disconnects the seat.
struct Presence {
    handle: Arc<TableHandle>,
    seat: usize,
    clock: Arc<dyn Clock>,
}

impl Drop for Presence {
    fn drop(&mut self) {
        let mut streams = self.handle.streams.lock().unwrap();
        streams[self.seat] = streams[self.seat].saturating_sub(1);
        if streams[self.seat] > 0 {
            return;
        }
        drop(streams);
        let mut table = self.handle.state.lock().unwrap();
        if table.disconnect(self.seat, self.clock.now()).is_ok() {
            let version = table.version();
            drop(table);
            self.handle.publish(version);
        }
    }
}

async fn events(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TokenQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let token = bearer(&headers)
        .or(query.token.as_deref())
        .ok_or_else(AppError::table_unauthorized)?;
    let (handle, actor) = state.registry.authorize(table_id(&id)?, token)?;
    let presence = match actor {
        Actor::Seat(seat) if handle.kind != TableKind::Local => {
            let mut table = handle.state.lock().unwrap();
            if let Err(AppError::Conflict { code, message }) = table.reconnect(seat) {
                let refusal = Event::default()
                    .event("error")
                    .data(json!({"error": code, "message": message}).to_string());
                return Ok(
                    Sse::new(stream::once(async { Ok::<_, Infallible>(refusal) })).into_response(),
                );
            }
            handle.streams.lock().unwrap()[seat] += 1;
            let version = table.version();
            drop(table);
            handle.publish(version);
            Some(Presence {
                handle: handle.clone(),
                seat,
                clock: state.clock.clone(),
            })
        }
        _ => None,
    };
    let viewer = viewer_of(actor);
    let clock = state.clock.clone();
    let changes = WatchStream::new(handle.changes.subscribe());
    let snapshots = changes.map(move |_| {
        // The stream owns the presence: it ends when the client goes.
        let _presence = &presence;
        let table = handle.state.lock().unwrap();
        let view = snapshot(&table, viewer, clock.now());
        let data = serde_json::to_string(&view).expect("a snapshot serializes");
        Ok::<_, Infallible>(Event::default().event("snapshot").data(data))
    });
    Ok(Sse::new(snapshots)
        .keep_alive(KeepAlive::new().interval(KEEP_ALIVE))
        .into_response())
}
