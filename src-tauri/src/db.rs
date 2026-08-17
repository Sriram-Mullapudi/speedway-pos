use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous, SqlitePool, SqlitePoolOptions};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Open (creating if needed) the local SQLite file in the OS app-data dir.
/// This is the heart of the offline-first story: the register owns its data
/// and never depends on a network round-trip to make a sale.
pub async fn init_pool(app: &AppHandle) -> anyhow::Result<SqlitePool> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let db_path = dir.join("pos.db");

    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        // Integrity is enforced in the Rust services, and the Phase 3 migration
        // restructures the cashiers table, so we don't rely on SQLite FK checks.
        .foreign_keys(false)
        // WAL survives power loss and app kills far better than the rollback
        // journal, and lets reads proceed during writes. NORMAL sync is the
        // standard WAL pairing: durable at checkpoint, fast at the register.
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    Ok(pool)
}

/// Embedded migrations from ./migrations, applied at startup.
pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
