use std::sync::Arc;

use anyhow::Context;
use beelot_server::app::{AppState, router};
use beelot_server::clock::SystemClock;
use beelot_server::config::Config;
use beelot_server::history::HistoryRecorder;
use beelot_server::jobs;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let bind = config.bind;
    let pool = beelot_server::db::connect(&config)
        .await
        .context("cannot reach the database: is `docker compose up -d --wait` running?")?;
    let history = Arc::new(HistoryRecorder::new(pool.clone()));
    let state = AppState::new(config, Arc::new(SystemClock))
        .with_pool(pool)
        .with_sink(history);
    let cancel = CancellationToken::new();
    let background = jobs::spawn_all(state.clone(), cancel.clone());
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("cannot listen on {bind}"))?;
    tracing::info!(%bind, "listening");
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    cancel.cancel();
    for job in background {
        job.await.ok();
    }
    Ok(())
}

/// Resolves on Ctrl-C or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}
