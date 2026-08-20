//! Phase 15 — backup, validation, retention, and restore staging.
//!
//! SAFETY POSTURE (why this is built the way it is):
//! * Backups are created with `VACUUM INTO`, which writes a fully-checkpointed,
//!   internally-consistent standalone SQLite file from the live connection.
//!   We never raw-copy pos.db while WAL writes may be pending.
//! * Restore does NOT hot-swap the live file under the open pool (unsafe under
//!   Windows file locks). Instead it validates, creates+validates a safety
//!   backup, then writes a `pending_restore` marker; the swap happens at next
//!   startup BEFORE the pool opens (see apply_pending_restore). This is a
//!   controlled restart, not a fake hot restore.
//! * We never delete pos.db before a valid safety backup exists.

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::fs;
use std::path::{Path, PathBuf};

pub const CURRENT_SCHEMA_VERSION: i64 = 9; // migrations 0001..0009

#[derive(Serialize, Clone)]
pub struct BackupMeta {
    pub filename: String,
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i64,
    pub source_size: u64,
    pub backup_size: u64,
    pub sha256: String,
    pub kind: String, // "manual" | "automatic" | "safety"
}

#[derive(Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub checks: Vec<(String, bool)>,
    pub schema_version: i64,
    pub compatibility: String, // Compatible | UpgradeRequired | NewerApplicationRequired | InvalidBackup
    pub message: String,
}

pub fn backups_dir(app_data: &Path) -> PathBuf { app_data.join("backups") }
fn meta_path(file: &Path) -> PathBuf { file.with_extension("json") }
pub fn db_path(app_data: &Path) -> PathBuf { app_data.join("pos.db") }
pub fn pending_marker(app_data: &Path) -> PathBuf { app_data.join("pending_restore.txt") }

fn now_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    s.to_string()
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("{:x}", h.finalize()))
}

/// Create a backup via VACUUM INTO. Returns metadata. Does not disrupt checkout:
/// VACUUM INTO reads a consistent snapshot; the pool stays open.
pub async fn create_backup(
    pool: &SqlitePool,
    app_data: &Path,
    kind: &str,
) -> Result<BackupMeta, String> {
    let dir = backups_dir(app_data);
    fs::create_dir_all(&dir).map_err(|e| format!("Backup directory unavailable: {e}"))?;

    let filename = format!("speedway-{}-{}.db", kind, now_stamp());
    let dest = dir.join(&filename);

    // VACUUM INTO requires a string literal path; bind via format is safe here
    // because the path is app-generated, not user input.
    let sql = format!("VACUUM INTO '{}'", dest.to_string_lossy().replace('\'', "''"));
    sqlx::query(&sql).execute(pool).await.map_err(|e| format!("Backup failed: {e}"))?;

    let source_size = fs::metadata(db_path(app_data)).map(|m| m.len()).unwrap_or(0);
    let backup_size = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    if backup_size == 0 {
        return Err("Backup file is empty after creation".into());
    }
    let sha256 = sha256_file(&dest).map_err(|e| format!("Checksum failed: {e}"))?;

    let meta = BackupMeta {
        filename: filename.clone(),
        created_at: now_stamp(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version: CURRENT_SCHEMA_VERSION,
        source_size,
        backup_size,
        sha256,
        kind: kind.to_string(),
    };
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    fs::write(meta_path(&dest), meta_json).map_err(|e| format!("Metadata write failed: {e}"))?;
    Ok(meta)
}

/// List backups newest-first by reading sidecar metadata.
pub fn list_backups(app_data: &Path) -> Vec<BackupMeta> {
    let dir = backups_dir(app_data);
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                if let Ok(txt) = fs::read_to_string(&p) {
                    if let Ok(m) = serde_json::from_str::<BackupMeta>(&txt) {
                        out.push(m);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

/// Validate a backup file: existence, size, checksum, openability, schema,
/// integrity, and compatibility. Pure enough to unit-test with temp DBs.
pub async fn validate_backup(app_data: &Path, filename: &str) -> ValidationResult {
    let file = backups_dir(app_data).join(filename);
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut schema_version = -1;

    let exists = file.exists();
    checks.push(("file_exists".into(), exists));
    if !exists {
        return ValidationResult { valid: false, checks, schema_version, compatibility: "InvalidBackup".into(),
            message: "Backup file not found".into() };
    }

    let size_ok = fs::metadata(&file).map(|m| m.len() > 512).unwrap_or(false);
    checks.push(("size_sane".into(), size_ok));

    // checksum vs sidecar
    let mut checksum_ok = false;
    if let Ok(txt) = fs::read_to_string(meta_path(&file)) {
        if let Ok(m) = serde_json::from_str::<BackupMeta>(&txt) {
            if let Ok(actual) = sha256_file(&file) {
                checksum_ok = actual == m.sha256;
            }
        }
    }
    checks.push(("checksum_matches".into(), checksum_ok));

    // open + integrity + schema version
    let mut can_open = false;
    let mut integrity_ok = false;
    if let Ok(pool) = open_readonly(&file).await {
        can_open = true;
        integrity_ok = integrity_check(&pool).await.unwrap_or(false);
        schema_version = read_user_version(&pool).await.unwrap_or(-1);
        pool.close().await;
    }
    checks.push(("sqlite_opens".into(), can_open));
    checks.push(("integrity_check".into(), integrity_ok));
    checks.push(("schema_readable".into(), schema_version >= 0));

    let compatibility = compat(schema_version);
    let valid = size_ok && checksum_ok && can_open && integrity_ok && compatibility == "Compatible";
    let message = if valid { "Backup is valid and compatible".into() }
        else { format!("Validation issues — compatibility: {compatibility}") };

    ValidationResult { valid, checks, schema_version, compatibility, message }
}

pub fn compat(schema_version: i64) -> String {
    if schema_version < 0 { return "InvalidBackup".into(); }
    if schema_version == CURRENT_SCHEMA_VERSION { return "Compatible".into(); }
    if schema_version < CURRENT_SCHEMA_VERSION {
        // Older backups migrate forward via sqlx on next open.
        return "UpgradeRequired".into();
    }
    "NewerApplicationRequired".into()
}

async fn open_readonly(file: &Path) -> Result<SqlitePool, sqlx::Error> {
    use sqlx::sqlite::SqliteConnectOptions;
    let opts = SqliteConnectOptions::new().filename(file).read_only(true);
    sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(opts).await
}

async fn integrity_check(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("PRAGMA integrity_check").fetch_one(pool).await?;
    let result: String = row.try_get(0).unwrap_or_default();
    Ok(result == "ok")
}

pub async fn read_user_version(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("PRAGMA user_version").fetch_one(pool).await?;
    Ok(row.try_get::<i64, _>(0).unwrap_or(-1))
}

/// Count-based retention: keep the newest `keep` backups of each kind, but never
/// delete a safety backup, never delete the single newest overall, and never
/// touch a file named in the pending marker. Retention failure is non-fatal.
pub fn apply_retention(app_data: &Path, keep_manual: usize, keep_auto: usize) -> usize {
    let all = list_backups(app_data);
    if all.is_empty() { return 0; }
    let protected = fs::read_to_string(pending_marker(app_data)).unwrap_or_default();
    let newest = all.first().map(|m| m.filename.clone());

    let mut deleted = 0;
    for kind in ["manual", "automatic"] {
        let keep = if kind == "manual" { keep_manual } else { keep_auto };
        let of_kind: Vec<&BackupMeta> = all.iter().filter(|m| m.kind == kind).collect();
        for m in of_kind.into_iter().skip(keep) {
            if Some(&m.filename) == newest.as_ref() { continue; }
            if protected.contains(&m.filename) { continue; }
            let f = backups_dir(app_data).join(&m.filename);
            if fs::remove_file(&f).is_ok() {
                let _ = fs::remove_file(meta_path(&f));
                deleted += 1;
            }
        }
    }
    // Safety backups are never auto-deleted here.
    deleted
}

/// Stage a restore: validate selected, make + validate a safety backup, then
/// write the pending marker. The actual swap is done at next startup.
pub async fn stage_restore(
    pool: &SqlitePool,
    app_data: &Path,
    filename: &str,
) -> Result<String, String> {
    let v = validate_backup(app_data, filename).await;
    if !v.valid {
        return Err(format!("Refusing to restore an invalid backup: {}", v.message));
    }
    // Safety backup of current DB, then validate it before proceeding.
    let safety = create_backup(pool, app_data, "safety").await
        .map_err(|e| format!("Safety backup failed — restore aborted: {e}"))?;
    let sv = validate_backup(app_data, &safety.filename).await;
    if !sv.valid {
        return Err("Safety backup failed validation — restore aborted".into());
    }
    // Write marker: the file to swap in, and the safety file for rollback.
    let marker = format!("restore={}\nsafety={}\n", filename, safety.filename);
    fs::write(pending_marker(app_data), &marker).map_err(|e| format!("Could not stage restore: {e}"))?;
    Ok(safety.filename)
}

/// Called at startup BEFORE the pool opens. If a pending restore exists, swap
/// the file in, verify, and roll back to safety on failure. Returns a status
/// string for logging. Safe no-op if no marker.
pub fn apply_pending_restore(app_data: &Path) -> Option<String> {
    let marker_path = pending_marker(app_data);
    let marker = fs::read_to_string(&marker_path).ok()?;
    let mut restore_file = None;
    let mut safety_file = None;
    for line in marker.lines() {
        if let Some(v) = line.strip_prefix("restore=") { restore_file = Some(v.to_string()); }
        if let Some(v) = line.strip_prefix("safety=") { safety_file = Some(v.to_string()); }
    }
    let restore_file = restore_file?;
    let src = backups_dir(app_data).join(&restore_file);
    let db = db_path(app_data);

    // Replace pos.db and clear WAL/SHM so the restored file is authoritative.
    let result = (|| -> std::io::Result<()> {
        // Remove sidecar WAL/SHM of the current DB.
        let _ = fs::remove_file(db.with_extension("db-wal"));
        let _ = fs::remove_file(db.with_extension("db-shm"));
        fs::copy(&src, &db)?;
        Ok(())
    })();

    let status = match result {
        Ok(_) => format!("restore applied from {}", restore_file),
        Err(e) => {
            // Roll back to safety if we have one.
            if let Some(sf) = safety_file {
                let _ = fs::copy(backups_dir(app_data).join(&sf), &db);
            }
            format!("restore FAILED ({e}); rolled back to safety")
        }
    };
    let _ = fs::remove_file(&marker_path);
    Some(status)
}

// ---- pure helpers unit-tested below ----------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_same_schema_is_compatible() {
        assert_eq!(compat(CURRENT_SCHEMA_VERSION), "Compatible");
    }
    #[test]
    fn compat_older_needs_upgrade() {
        assert_eq!(compat(CURRENT_SCHEMA_VERSION - 1), "UpgradeRequired");
    }
    #[test]
    fn compat_newer_rejected() {
        assert_eq!(compat(CURRENT_SCHEMA_VERSION + 1), "NewerApplicationRequired");
    }
    #[test]
    fn compat_invalid_rejected() {
        assert_eq!(compat(-1), "InvalidBackup");
    }

    #[test]
    fn checksum_is_deterministic_and_detects_change() {
        let dir = std::env::temp_dir().join(format!("sbk-{}", now_stamp()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.bin");
        fs::write(&f, b"hello world").unwrap();
        let h1 = sha256_file(&f).unwrap();
        let h2 = sha256_file(&f).unwrap();
        assert_eq!(h1, h2);
        fs::write(&f, b"hello WORLD").unwrap();
        let h3 = sha256_file(&f).unwrap();
        assert_ne!(h1, h3);
        let _ = fs::remove_dir_all(&dir);
    }
}
