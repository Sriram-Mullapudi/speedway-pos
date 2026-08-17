use crate::AppState;

const LAYOUT_KEY: &str = "touchscreen_layout";

/// Returns the saved touchscreen layout JSON, or None if never configured.
#[tauri::command]
pub async fn get_layout(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(LAYOUT_KEY)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

/// Upsert the touchscreen layout JSON (last write wins).
#[tauri::command]
pub async fn save_layout(state: tauri::State<'_, AppState>, layout: String) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(LAYOUT_KEY)
    .bind(&layout)
    .execute(&state.pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

use std::collections::HashMap;
use crate::security::role_can;

pub const KNOWN_KEYS: &[&str] = &[
    "store_name", "receipt_footer", "default_tax_pct",
    "loyalty_threshold", "loyalty_reward", "low_stock_default", "theme",
    "dev_receipt_mode", "dev_drawer_mode", "dev_drawer_card", "dev_printer_forcefail",
    "receipt_paper_width", "receipt_auto_print", "receipt_copies",
    "store_address", "store_phone", "store_tax_id",
];

/// Typed read with default — used by the Rust sale path so business rules
/// come from the database, not the frontend.
pub async fn get_setting_i64(pool: &sqlx::SqlitePool, key: &str, default: i64) -> i64 {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default)
}

pub async fn get_setting_str(pool: &sqlx::SqlitePool, key: &str, default: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| default.to_string())
}

#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().collect())
}

/// Manager-only. Writes each known key and audits the change.
#[tauri::command]
pub async fn save_settings(
    state: tauri::State<'_, AppState>,
    settings: HashMap<String, String>,
) -> Result<(), String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;
    if !role_can(&sess.role, "settings") {
        return Err("Settings require a manager".into());
    }
    for (k, v) in settings.iter() {
        if !KNOWN_KEYS.contains(&k.as_str()) {
            continue; // ignore unknown keys defensively
        }
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(k).bind(v)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    }
    crate::audit::write(&state.pool, Some(sess.cashier_id), "settings.updated", Some("settings"), None,
        Some(serde_json::json!(settings).to_string())).await;
    Ok(())
}
