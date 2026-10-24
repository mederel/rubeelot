//! One MariaDB container per test binary, and a fresh database for each test. Docker must be running.

use std::sync::OnceLock;

use sqlx::Executor;
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions};
use testcontainers_modules::mariadb::Mariadb;
use testcontainers_modules::testcontainers::ImageExt;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

/// The container's id and its host port. testcontainers' reaper removes the container once the binary exits.
static SERVER: OnceLock<(String, u16)> = OnceLock::new();

fn server() -> &'static (String, u16) {
    SERVER.get_or_init(|| {
        // The container is started on a runtime of its own, which outlives every test's runtime.
        std::thread::spawn(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let container = runtime
                .block_on(Mariadb::default().with_tag("12.3").start())
                .expect("a MariaDB container: is Docker running?");
            let port = runtime
                .block_on(container.get_host_port_ipv4(3306))
                .unwrap();
            let id = container.id().to_owned();
            std::mem::forget(container);
            std::mem::forget(runtime);
            (id, port)
        })
        .join()
        .unwrap()
    })
}

fn options(port: u16) -> MySqlConnectOptions {
    MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username("root")
}

/// A pool on a new, migrated database of its own.
pub async fn test_pool() -> MySqlPool {
    let (_, port) = server();
    let name = format!("t_{}", uuid::Uuid::new_v4().simple());
    let admin = MySqlPoolOptions::new()
        .max_connections(1)
        .connect_with(options(*port))
        .await
        .unwrap();
    admin
        .execute(format!("CREATE DATABASE {name} CHARACTER SET utf8mb4").as_str())
        .await
        .unwrap();
    admin.close().await;
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect_with(options(*port).database(&name))
        .await
        .unwrap();
    beelot_server::db::migrate(&pool).await.unwrap();
    pool
}
