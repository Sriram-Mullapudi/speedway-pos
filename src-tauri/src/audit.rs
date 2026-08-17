//! Append-only audit writer. Best-effort — never fails the calling flow.
use sqlx::SqlitePool;

pub async fn write(
    pool: &SqlitePool,
    user_id: Option<i64>,
    action: &str,
    entity: Option<&str>,
    entity_id: Option<i64>,
    detail: Option<String>,
) {
    let _ = sqlx::query(
        "INSERT INTO audit_log (user_id, action, entity, entity_id, detail) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(user_id).bind(action).bind(entity).bind(entity_id).bind(detail)
    .execute(pool).await;
}

use crate::AppState;
use serde::Serialize;

#[derive(Serialize, sqlx::FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub user: Option<String>,
    pub action: String,
    pub entity: Option<String>,
    pub entity_id: Option<i64>,
    pub detail: Option<String>,
    pub created_at: String,
}

/// Manager/admin-only read of the append-only audit trail, newest first,
/// filterable by action substring and user.
#[tauri::command]
pub async fn list_audit_log(
    state: tauri::State<'_, AppState>,
    action_like: Option<String>,
    user_id: Option<i64>,
) -> Result<Vec<AuditRow>, String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;
    if !crate::security::role_can(&sess.role, "settings") {
        return Err("The audit log requires a manager".into());
    }
    let action = format!("%{}%", action_like.unwrap_or_default());
    sqlx::query_as::<_, AuditRow>(
        "SELECT a.id, c.name AS user, a.action, a.entity, a.entity_id, a.detail, a.created_at \
         FROM audit_log a LEFT JOIN cashiers c ON c.id = a.user_id \
         WHERE a.action LIKE ?1 AND (?2 IS NULL OR a.user_id = ?2) \
         ORDER BY a.id DESC LIMIT 300",
    )
    .bind(action)
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}
