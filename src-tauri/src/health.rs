//! Phase 15 — system health, diagnostics, and the Tauri command surface for
//! backup/recovery. Thin wrappers over `backup` + `applog`. Diagnostics are
//! carefully sanitized: no PINs, hashes, secrets, customer PII, or txn history.

use crate::backup::{self, BackupMeta, ValidationResult};
use crate::security::role_can;
use crate::{applog, audit, AppState};
use serde::Serialize;
use sqlx::Row;
use std::path::PathBuf;

fn app_data(state: &tauri::State<'_, AppState>) -> PathBuf { state.app_data.clone() }

fn require(state: &tauri::State<'_, AppState>, action: &str) -> Result<crate::models::SessionInfo, String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;
    if !role_can(&sess.role, action) {
        return Err(format!("This action requires a manager"));
    }
    Ok(sess)
}

// ---- health ---------------------------------------------------------------

#[derive(Serialize)]
pub struct HealthReport {
    pub db_status: String,       // Healthy | Attention | Error
    pub schema_version: i64,
    pub db_size: u64,
    pub wal_size: u64,
    pub integrity: String,       // ok | failed | not_run
    pub wal_mode: String,
    pub backup_status: String,   // Healthy | Attention | NeverBackedUp | Failed
    pub last_backup: Option<String>,
    pub last_backup_kind: Option<String>,
    pub auto_frequency: String,
    pub app_version: String,
    pub platform: String,
    pub sync_status: String,     // always Disabled for now
}

#[tauri::command]
pub async fn system_health(state: tauri::State<'_, AppState>, run_integrity: bool) -> Result<HealthReport, String> {
    let dir = app_data(&state);
    let db = backup::db_path(&dir);
    let db_size = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
    let wal_size = std::fs::metadata(db.with_extension("db-wal")).map(|m| m.len()).unwrap_or(0);

    let schema_version = backup::read_user_version(&state.pool).await.unwrap_or(-1);

    let integrity = if run_integrity {
        match sqlx::query("PRAGMA integrity_check").fetch_one(&state.pool).await {
            Ok(row) => if row.try_get::<String, _>(0).unwrap_or_default() == "ok" { "ok" } else { "failed" },
            Err(_) => "failed",
        }
    } else { "not_run" }.to_string();

    let db_status = if integrity == "failed" { "Error" }
        else if schema_version < 0 { "Attention" } else { "Healthy" }.to_string();

    let backups = backup::list_backups(&dir);
    let (backup_status, last_backup, last_backup_kind) = match backups.first() {
        Some(m) => ("Healthy".to_string(), Some(m.created_at.clone()), Some(m.kind.clone())),
        None => ("NeverBackedUp".to_string(), None, None),
    };
    let last_fail = crate::settings::get_setting_str(&state.pool, "backup_last_error", "").await;
    let backup_status = if !last_fail.is_empty() && backups.is_empty() { "Failed".to_string() }
        else if !last_fail.is_empty() { "Attention".to_string() } else { backup_status };

    Ok(HealthReport {
        db_status, schema_version, db_size, wal_size, integrity,
        wal_mode: "WAL".to_string(),
        backup_status, last_backup, last_backup_kind,
        auto_frequency: crate::settings::get_setting_str(&state.pool, "backup_auto_freq", "disabled").await,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        sync_status: "Disabled".to_string(),
    })
}

// ---- backup commands (thin wrappers) --------------------------------------

#[tauri::command]
pub async fn create_manual_backup(state: tauri::State<'_, AppState>) -> Result<BackupMeta, String> {
    let sess = require(&state, "backup")?;
    let dir = app_data(&state);
    match backup::create_backup(&state.pool, &dir, "manual").await {
        Ok(meta) => {
            let _ = crate::settings::set_setting(&state.pool, "backup_last_error", "").await;
            let keep_m = crate::settings::get_setting_i64(&state.pool, "backup_keep_manual", 10).await as usize;
            let keep_a = crate::settings::get_setting_i64(&state.pool, "backup_keep_auto", 7).await as usize;
            let _ = backup::apply_retention(&dir, keep_m, keep_a);
            applog::log(&dir, "INFO", &format!("manual backup created: {}", meta.filename));
            audit::write(&state.pool, Some(sess.cashier_id), "backup.create", Some("backup"), None,
                Some(format!("kind=manual file={}", meta.filename))).await;
            Ok(meta)
        }
        Err(e) => {
            let _ = crate::settings::set_setting(&state.pool, "backup_last_error", &e).await;
            applog::log(&dir, "ERROR", &format!("manual backup failed: {e}"));
            audit::write(&state.pool, Some(sess.cashier_id), "backup.failed", Some("backup"), None,
                Some(e.clone())).await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn list_backups_cmd(state: tauri::State<'_, AppState>) -> Result<Vec<BackupMeta>, String> {
    require(&state, "backup")?;
    Ok(backup::list_backups(&app_data(&state)))
}

#[tauri::command]
pub async fn validate_backup_cmd(state: tauri::State<'_, AppState>, filename: String) -> Result<ValidationResult, String> {
    require(&state, "backup")?;
    Ok(backup::validate_backup(&app_data(&state), &filename).await)
}

#[tauri::command]
pub async fn restore_backup_cmd(state: tauri::State<'_, AppState>, filename: String) -> Result<String, String> {
    let sess = require(&state, "restore")?;
    let dir = app_data(&state);
    audit::write(&state.pool, Some(sess.cashier_id), "restore.requested", Some("backup"), None,
        Some(format!("file={}", filename))).await;
    match backup::stage_restore(&state.pool, &dir, &filename).await {
        Ok(safety) => {
            applog::log(&dir, "INFO", &format!("restore staged: {} (safety {})", filename, safety));
            audit::write(&state.pool, Some(sess.cashier_id), "restore.staged", Some("backup"), None,
                Some(format!("file={} safety={}", filename, safety))).await;
            Ok(format!("Restore staged. The application will apply it on next restart. A safety backup ({safety}) was created."))
        }
        Err(e) => {
            applog::log(&dir, "ERROR", &format!("restore staging failed: {e}"));
            audit::write(&state.pool, Some(sess.cashier_id), "restore.failed", Some("backup"), None, Some(e.clone())).await;
            Err(e)
        }
    }
}

// ---- diagnostics ----------------------------------------------------------

async fn diagnostic_text(state: &tauri::State<'_, AppState>) -> Result<String, String> {
    let dir = app_data(state);
    let db = backup::db_path(&dir);
    let db_size = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
    let wal_size = std::fs::metadata(db.with_extension("db-wal")).map(|m| m.len()).unwrap_or(0);
    let schema_version = backup::read_user_version(&state.pool).await.unwrap_or(-1);
    let backups = backup::list_backups(&dir);
    let (bstatus, last, kind) = match backups.first() {
        Some(m) => ("Healthy", Some(m.created_at.clone()), Some(m.kind.clone())),
        None => ("NeverBackedUp", None, None),
    };
    let freq = crate::settings::get_setting_str(&state.pool, "backup_auto_freq", "disabled").await;
    Ok(format!(
        "Speedway POS Diagnostics\n\
         app_version: {}\nplatform: {}\nschema_version: {}\n\
         db_size_bytes: {}\nwal_mode: WAL\nwal_size_bytes: {}\n\
         backup_status: {}\nlast_backup: {}\nlast_backup_kind: {}\nauto_backup: {}\n\
         sync: Disabled\n",
        env!("CARGO_PKG_VERSION"), std::env::consts::OS, schema_version,
        db_size, wal_size, bstatus,
        last.unwrap_or_else(|| "never".into()), kind.unwrap_or_else(|| "n/a".into()), freq,
    ))
}

#[tauri::command]
pub async fn diagnostic_info(state: tauri::State<'_, AppState>) -> Result<String, String> {
    require(&state, "backup")?;
    diagnostic_text(&state).await
}

#[tauri::command]
pub async fn export_diagnostic_bundle(state: tauri::State<'_, AppState>) -> Result<String, String> {
    require(&state, "backup")?;
    let dir = app_data(&state);
    let summary = diagnostic_text(&state).await?;
    let bundle_dir = dir.join("diagnostics").join(format!("bundle-{}-{}", env!("CARGO_PKG_VERSION"), chrono_stamp()));
    std::fs::create_dir_all(&bundle_dir).map_err(|e| format!("Bundle dir failed: {e}"))?;
    std::fs::write(bundle_dir.join("summary.txt"), &summary).map_err(|e| e.to_string())?;

    // Device snapshot (safe) + sanitized recent logs. No DB, no PII.
    let logs = applog::tail(&dir, 200).join("\n");
    std::fs::write(bundle_dir.join("recent_logs.txt"), logs).map_err(|e| e.to_string())?;

    applog::log(&dir, "INFO", "diagnostic bundle exported");
    Ok(bundle_dir.to_string_lossy().to_string())
}

fn chrono_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0).to_string()
}
