//! Tables: who sits where, and the game played at each table.

pub mod messages;
pub mod seats;

pub use seats::{Connection, Lobby, Seat, SeatKind, Status, TableKind, local_seats, required_name};
