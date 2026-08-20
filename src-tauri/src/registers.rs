//! Phase 16 — multi-register foundation (single store, multiple lanes).
//!
//! Each terminal is a "register" with a stable `global_id` (UUID). The local
//! integer id stays for cheap joins; the global_id is what a future sync/branch
//! layer will use to identify a terminal across databases without integer
//! collisions. This phase adds identity and per-register reporting only — NO
//! sync, no cross-terminal communication.

use crate::security::role_can;
use crate::{audit, settings, AppState};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

/// Generate a UUIDv4-style string using the OS RNG already used for PIN salts.
/// Pure w.r.t. its RNG input; format is unit-tested. We prefix "reg-" so a
/// register id is recognizable in logs and diagnostics.
pub fn new_register_global_id() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    format!("reg-{}", format_uuid_v4(&mut bytes))
}

/// Apply RFC-4122 v4 version/variant bits and format as canonical UUID.
/// Split out so it can be tested with fixed bytes.
pub fn format_uuid_v4(bytes: &mut [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 10xx
    let h: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Register {
    pub id: i64,
    pub global_id: String,
    pub name: String,
    pub active: bool,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct RegisterInput {
    pub id: Option<i64>,
    pub name: String,
}

fn require(state: &tauri::State<'_, AppState>, action: &str) -> Result<crate::models::SessionInfo, String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;
    if !role_can(&sess.role, action) {
        return Err("This action requires a manager".into());
    }
    Ok(sess)
}

#[tauri::command]
pub async fn list_registers(state: tauri::State<'_, AppState>) -> Result<Vec<Register>, String> {
    sqlx::query_as::<_, Register>(
        "SELECT id, global_id, name, active, created_at FROM registers ORDER BY active DESC, id",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_register(state: tauri::State<'_, AppState>, input: RegisterInput) -> Result<i64, String> {
    let sess = require(&state, "settings")?;
    if input.name.trim().is_empty() {
        return Err("Register name is required".into());
    }
    let id = if let Some(id) = input.id {
        sqlx::query("UPDATE registers SET name = ?1 WHERE id = ?2")
            .bind(&input.name).bind(id)
            .execute(&state.pool).await.map_err(|e| e.to_string())?;
        id
    } else {
        let gid = new_register_global_id();
        let r = sqlx::query("INSERT INTO registers (global_id, name) VALUES (?1, ?2)")
            .bind(&gid).bind(&input.name)
            .execute(&state.pool).await.map_err(|e| e.to_string())?;
        r.last_insert_rowid()
    };
    audit::write(&state.pool, Some(sess.cashier_id), "register.upsert", Some("register"), Some(id), None).await;
    Ok(id)
}

#[tauri::command]
pub async fn set_register_active(state: tauri::State<'_, AppState>, id: i64, active: bool) -> Result<(), String> {
    let sess = require(&state, "settings")?;
    // Never allow deactivating the last active register.
    if !active {
        let active_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM registers WHERE active = 1")
            .fetch_one(&state.pool).await.map_err(|e| e.to_string())?;
        if active_count <= 1 {
            return Err("Cannot deactivate the only active register".into());
        }
    }
    sqlx::query("UPDATE registers SET active = ?1 WHERE id = ?2")
        .bind(active).bind(id)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "register.active", Some("register"), Some(id),
        Some(format!("active={}", active))).await;
    Ok(())
}

/// Which register is THIS terminal? Persisted per-machine in settings. Defaults
/// to register 1 (the backfilled default) so an un-configured install just works.
#[tauri::command]
pub async fn get_active_register(state: tauri::State<'_, AppState>) -> Result<Register, String> {
    let id = settings::get_setting_str(&state.pool, "active_register_id", "1").await;
    let id: i64 = id.parse().unwrap_or(1);
    sqlx::query_as::<_, Register>(
        "SELECT id, global_id, name, active, created_at FROM registers WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Configured register not found".into())
}

/// Set which register this terminal is. Manager action; persisted locally.
#[tauri::command]
pub async fn set_active_register(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let sess = require(&state, "settings")?;
    // Verify it exists and is active.
    let ok: Option<i64> = sqlx::query_scalar("SELECT id FROM registers WHERE id = ?1 AND active = 1")
        .bind(id).fetch_optional(&state.pool).await.map_err(|e| e.to_string())?;
    if ok.is_none() {
        return Err("That register does not exist or is inactive".into());
    }
    settings::set_setting(&state.pool, "active_register_id", &id.to_string()).await?;
    audit::write(&state.pool, Some(sess.cashier_id), "register.select", Some("register"), Some(id), None).await;
    Ok(())
}

/// Resolve (id, global_id) for the terminal's active register — used by the sale
/// and shift paths to stamp ownership.
pub async fn active_register_ids(pool: &sqlx::SqlitePool) -> (i64, String) {
    let id: i64 = settings::get_setting_str(pool, "active_register_id", "1").await.parse().unwrap_or(1);
    let gid: Option<String> = sqlx::query_scalar("SELECT global_id FROM registers WHERE id = ?1")
        .bind(id).fetch_optional(pool).await.ok().flatten();
    (id, gid.unwrap_or_else(|| "reg-00000000-0000-0000-0000-000000000001".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_has_canonical_shape() {
        let mut bytes = [0u8; 16];
        let s = format_uuid_v4(&mut bytes);
        // 8-4-4-4-12 hyphenation
        let parts: Vec<&str> = s.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    #[test]
    fn uuid_sets_version_and_variant_bits() {
        let mut bytes = [0xffu8; 16];
        let s = format_uuid_v4(&mut bytes);
        // version nibble (first char of 3rd group) must be '4'
        let third = s.split('-').nth(2).unwrap();
        assert_eq!(&third[0..1], "4");
        // variant (first char of 4th group) must be 8,9,a,or b
        let fourth = s.split('-').nth(3).unwrap();
        assert!(["8", "9", "a", "b"].contains(&&fourth[0..1]));
    }

    #[test]
    fn prefixed_id_is_recognizable() {
        let id = new_register_global_id();
        assert!(id.starts_with("reg-"));
        assert_eq!(id.len(), 4 + 36); // "reg-" + canonical UUID
    }
}
