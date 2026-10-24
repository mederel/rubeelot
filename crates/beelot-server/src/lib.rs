//! Beelot's web server: local and online tables, accounts, match history and privacy, serving the frontend.

pub mod api;
pub mod app;
pub mod auth;
pub mod clock;
pub mod config;
pub mod db;
pub mod error;
pub mod history;
pub mod jobs;
pub mod limits;
pub mod privacy;
pub mod routes;
pub mod tables;
