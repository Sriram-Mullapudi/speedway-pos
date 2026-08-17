use crate::models::{Cashier, SessionInfo};
use crate::security::{hash_pin, role_can, verify_pin};
use crate::{audit, AppState};

fn current(state: &tauri::State<'_, AppState>) -> Option<SessionInfo> {
    state.current_session()
}

fn require(state: &tauri::State<'_, AppState>, action: &str) -> Result<SessionInfo, String> {
    let sess = current(state).ok_or("Not signed in")?;
    if role_can(&sess.role, action) {
        Ok(sess)
    } else {
        Err(format!("Permission denied — '{}' requires a manager", action))
    }
}

#[tauri::command]
pub async fn login_with_pin(state: tauri::State<'_, AppState>, pin: String) -> Result<SessionInfo, String> {
    let rows = sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, name, role, pin_hash FROM cashiers WHERE active = 1",
    )
    .fetch_all(&state.pool).await.map_err(|e| e.to_string())?;

    let mut matched: Option<(i64, String, String)> = None;
    for (id, name, role, hash) in rows {
        if verify_pin(&pin, &hash) {
            matched = Some((id, name, role));
            break;
        }
    }

    match matched {
        Some((id, name, role)) => {
            let res = sqlx::query("INSERT INTO cashier_sessions (cashier_id) VALUES (?1)")
                .bind(id).execute(&state.pool).await.map_err(|e| e.to_string())?;
            let info = SessionInfo { session_id: res.last_insert_rowid(), cashier_id: id, name, role };
            *state.session.lock().unwrap() = Some(info.clone());
            audit::write(&state.pool, Some(id), "auth.login.success", Some("cashier"), Some(id), None).await;
            Ok(info)
        }
        None => {
            audit::write(&state.pool, None, "auth.login.failed", Some("cashier"), None, None).await;
            Err("Invalid PIN".into())
        }
    }
}

#[tauri::command]
pub async fn logout_cashier(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if let Some(s) = current(&state) {
        let _ = sqlx::query("UPDATE cashier_sessions SET ended_at = datetime('now') WHERE id = ?1")
            .bind(s.session_id).execute(&state.pool).await;
        audit::write(&state.pool, Some(s.cashier_id), "auth.logout", Some("cashier"), Some(s.cashier_id), None).await;
    }
    *state.session.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn get_current_session(state: tauri::State<'_, AppState>) -> Result<Option<SessionInfo>, String> {
    Ok(current(&state))
}

#[tauri::command]
pub async fn list_cashiers(state: tauri::State<'_, AppState>) -> Result<Vec<Cashier>, String> {
    sqlx::query_as::<_, Cashier>(
        "SELECT id, name, role, active, created_at, updated_at FROM cashiers ORDER BY id",
    )
    .fetch_all(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_cashier(state: tauri::State<'_, AppState>, name: String, role: String, pin: String) -> Result<i64, String> {
    require(&state, "manage_cashiers")?;
    if !["cashier", "manager", "admin"].contains(&role.as_str()) {
        return Err("Invalid role".into());
    }
    let hash = hash_pin(&pin)?;
    let res = sqlx::query("INSERT INTO cashiers (name, role, pin_hash) VALUES (?1, ?2, ?3)")
        .bind(&name).bind(&role).bind(&hash)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    Ok(res.last_insert_rowid())
}

#[tauri::command]
pub async fn update_cashier(
    state: tauri::State<'_, AppState>,
    id: i64, name: String, role: String, active: bool, pin: Option<String>,
) -> Result<(), String> {
    require(&state, "manage_cashiers")?;
    sqlx::query("UPDATE cashiers SET name = ?1, role = ?2, active = ?3, updated_at = datetime('now') WHERE id = ?4")
        .bind(&name).bind(&role).bind(active).bind(id)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    if let Some(pin) = pin {
        if !pin.is_empty() {
            let hash = hash_pin(&pin)?;
            sqlx::query("UPDATE cashiers SET pin_hash = ?1 WHERE id = ?2")
                .bind(&hash).bind(id).execute(&state.pool).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn deactivate_cashier(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    require(&state, "manage_cashiers")?;
    sqlx::query("UPDATE cashiers SET active = 0, updated_at = datetime('now') WHERE id = ?1")
        .bind(id).execute(&state.pool).await.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn require_permission(state: tauri::State<'_, AppState>, action: String) -> Result<bool, String> {
    let sess = current(&state).ok_or("Not signed in")?;
    if role_can(&sess.role, &action) {
        return Ok(true);
    }
    let allowed: Option<i64> = sqlx::query_scalar(
        "SELECT allowed FROM permission_overrides WHERE cashier_id = ?1 AND action = ?2 LIMIT 1",
    )
    .bind(sess.cashier_id).bind(&action)
    .fetch_optional(&state.pool).await.map_err(|e| e.to_string())?;

    if allowed == Some(1) {
        Ok(true)
    } else {
        audit::write(&state.pool, Some(sess.cashier_id), "auth.permission.denied", Some("action"), None, Some(action.clone())).await;
        Err(format!("Permission denied — '{}'", action))
    }
}

#[tauri::command]
pub async fn manager_override(state: tauri::State<'_, AppState>, action: String, manager_pin: String) -> Result<i64, String> {
    let rows = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, role, pin_hash FROM cashiers WHERE active = 1 AND role IN ('manager','admin')",
    )
    .fetch_all(&state.pool).await.map_err(|e| e.to_string())?;
    for (id, role, hash) in rows {
        if verify_pin(&manager_pin, &hash) && role_can(&role, &action) {
            audit::write(&state.pool, Some(id), "auth.manager.override", Some("action"), None, Some(action.clone())).await;
            return Ok(id);
        }
    }
    Err("Manager override failed — wrong PIN or insufficient role".into())
}
