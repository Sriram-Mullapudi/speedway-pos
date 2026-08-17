use crate::models::{SessionInfo, Shift};
use crate::security::role_can;
use crate::{audit, AppState};
use serde::Serialize;
use sqlx::SqlitePool;

pub fn expected_cash(opening: i64, cash_sales: i64, cash_refunds: i64, cash_in: i64, cash_out: i64) -> i64 {
    opening + cash_sales - cash_refunds + cash_in - cash_out
}
pub fn over_short(counted: i64, expected: i64) -> i64 {
    counted - expected
}

#[derive(Serialize)]
pub struct ShiftSummary {
    pub shift_id: i64,
    pub cashier_id: i64,
    pub opening_float: i64,
    pub cash_sales: i64,
    pub card_sales: i64,
    pub cash_refunds: i64,
    pub cash_in: i64,
    pub cash_out: i64,
    pub gross_sales: i64,
    pub txn_count: i64,
    pub expected_cash: i64,
    pub counted_cash: Option<i64>,
    pub over_short: Option<i64>,
    pub status: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
}

fn current(state: &tauri::State<'_, AppState>) -> Option<SessionInfo> {
    state.current_session()
}

async fn fetch_shift(pool: &SqlitePool, id: i64) -> Result<Shift, String> {
    sqlx::query_as::<_, Shift>(
        "SELECT id, register_id, cashier_id, opening_float, counted_cash, \
                expected_cash, over_short, status, opened_at, closed_at \
         FROM shifts WHERE id = ?1",
    )
    .bind(id).fetch_one(pool).await.map_err(|e| e.to_string())
}

async fn scalar(pool: &SqlitePool, sql: &str, shift_id: i64) -> Result<i64, String> {
    sqlx::query_scalar::<_, i64>(sql).bind(shift_id).fetch_one(pool).await.map_err(|e| e.to_string())
}

async fn build_summary(pool: &SqlitePool, shift: &Shift) -> Result<ShiftSummary, String> {
    let cash_sales = scalar(pool,
        "SELECT COALESCE(SUM(p.amount),0) FROM payments p JOIN transactions t ON t.id = p.transaction_id \
         WHERE t.shift_id = ?1 AND t.type = 'sale' AND t.status = 'completed' AND p.kind = 'cash'", shift.id).await?;
    let card_sales = scalar(pool,
        "SELECT COALESCE(SUM(p.amount),0) FROM payments p JOIN transactions t ON t.id = p.transaction_id \
         WHERE t.shift_id = ?1 AND t.type = 'sale' AND t.status = 'completed' AND p.kind = 'card'", shift.id).await?;
    let cash_refunds = scalar(pool,
        "SELECT COALESCE(SUM(p.amount),0) FROM payments p JOIN transactions t ON t.id = p.transaction_id \
         WHERE t.shift_id = ?1 AND t.type = 'refund' AND p.kind = 'cash'", shift.id).await?;
    let cash_in = scalar(pool,
        "SELECT COALESCE(SUM(amount),0) FROM cash_drawer_events WHERE shift_id = ?1 AND event_type = 'paid_in'", shift.id).await?;
    let cash_out = scalar(pool,
        "SELECT COALESCE(SUM(amount),0) FROM cash_drawer_events WHERE shift_id = ?1 AND event_type IN ('paid_out','safe_drop')", shift.id).await?;
    let gross_sales = scalar(pool,
        "SELECT COALESCE(SUM(total),0) FROM transactions WHERE shift_id = ?1 AND type = 'sale' AND status = 'completed'", shift.id).await?;
    let txn_count = scalar(pool,
        "SELECT COUNT(*) FROM transactions WHERE shift_id = ?1 AND type = 'sale' AND status = 'completed'", shift.id).await?;

    let expected = expected_cash(shift.opening_float, cash_sales, cash_refunds, cash_in, cash_out);

    Ok(ShiftSummary {
        shift_id: shift.id, cashier_id: shift.cashier_id, opening_float: shift.opening_float,
        cash_sales, card_sales, cash_refunds, cash_in, cash_out, gross_sales, txn_count,
        expected_cash: expected, counted_cash: shift.counted_cash, over_short: shift.over_short,
        status: shift.status.clone(), opened_at: shift.opened_at.clone(), closed_at: shift.closed_at.clone(),
    })
}

#[tauri::command]
pub async fn open_shift(state: tauri::State<'_, AppState>, starting_cash: i64) -> Result<Shift, String> {
    let sess = current(&state).ok_or("Not signed in")?;
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM shifts WHERE cashier_id = ?1 AND status = 'open' LIMIT 1")
        .bind(sess.cashier_id).fetch_optional(&state.pool).await.map_err(|e| e.to_string())?;
    if existing.is_some() {
        return Err("You already have an open shift".into());
    }
    let res = sqlx::query("INSERT INTO shifts (register_id, cashier_id, opening_float) VALUES (1, ?1, ?2)")
        .bind(sess.cashier_id).bind(starting_cash).execute(&state.pool).await.map_err(|e| e.to_string())?;
    let shift_id = res.last_insert_rowid();
    sqlx::query("INSERT INTO cash_drawer_events (cashier_id, shift_id, event_type, amount, reason) VALUES (?1, ?2, 'shift_open', ?3, 'Opening float')")
        .bind(sess.cashier_id).bind(shift_id).bind(starting_cash).execute(&state.pool).await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "shift.opened", Some("shift"), Some(shift_id), None).await;
    fetch_shift(&state.pool, shift_id).await
}

#[tauri::command]
pub async fn get_active_shift(state: tauri::State<'_, AppState>) -> Result<Option<Shift>, String> {
    let Some(sess) = current(&state) else { return Ok(None); };
    sqlx::query_as::<_, Shift>(
        "SELECT id, register_id, cashier_id, opening_float, counted_cash, expected_cash, over_short, status, opened_at, closed_at \
         FROM shifts WHERE cashier_id = ?1 AND status = 'open' ORDER BY id DESC LIMIT 1",
    )
    .bind(sess.cashier_id).fetch_optional(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_shift_summary(state: tauri::State<'_, AppState>, shift_id: i64) -> Result<ShiftSummary, String> {
    let shift = fetch_shift(&state.pool, shift_id).await?;
    build_summary(&state.pool, &shift).await
}

#[tauri::command]
pub async fn close_shift(state: tauri::State<'_, AppState>, shift_id: i64, counted_cash: i64) -> Result<ShiftSummary, String> {
    let sess = current(&state).ok_or("Not signed in")?;
    let shift = fetch_shift(&state.pool, shift_id).await?;
    if shift.cashier_id != sess.cashier_id && !role_can(&sess.role, "shift_close_override") {
        return Err("You can only close your own shift".into());
    }
    if shift.status == "closed" {
        return Err("Shift is already closed".into());
    }
    let summary = build_summary(&state.pool, &shift).await?;
    let expected = summary.expected_cash;
    let os = over_short(counted_cash, expected);
    sqlx::query("UPDATE shifts SET status = 'closed', closed_at = datetime('now'), counted_cash = ?1, expected_cash = ?2, over_short = ?3 WHERE id = ?4")
        .bind(counted_cash).bind(expected).bind(os).bind(shift_id).execute(&state.pool).await.map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO cash_drawer_events (cashier_id, shift_id, event_type, amount, reason) VALUES (?1, ?2, 'shift_close', ?3, 'Closing count')")
        .bind(sess.cashier_id).bind(shift_id).bind(counted_cash).execute(&state.pool).await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "shift.closed", Some("shift"), Some(shift_id), None).await;
    Ok(ShiftSummary { counted_cash: Some(counted_cash), over_short: Some(os), status: "closed".into(), ..summary })
}

#[tauri::command]
pub async fn create_cash_drawer_event(
    state: tauri::State<'_, AppState>,
    event_type: String, amount: i64, reason: Option<String>, manager_approved_by: Option<i64>,
) -> Result<(), String> {
    let sess = current(&state).ok_or("Not signed in")?;
    let sensitive = matches!(event_type.as_str(), "no_sale" | "paid_out");
    if sensitive && !role_can(&sess.role, "no_sale") && manager_approved_by.is_none() {
        return Err("Manager approval required".into());
    }
    let shift_id: i64 = sqlx::query_scalar("SELECT id FROM shifts WHERE cashier_id = ?1 AND status = 'open' LIMIT 1")
        .bind(sess.cashier_id).fetch_optional(&state.pool).await.map_err(|e| e.to_string())?
        .ok_or("Open a shift before drawer operations")?;
    sqlx::query("INSERT INTO cash_drawer_events (cashier_id, shift_id, event_type, amount, reason, manager_approved_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
        .bind(sess.cashier_id).bind(shift_id).bind(&event_type).bind(amount).bind(&reason).bind(manager_approved_by)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), &format!("drawer.{}", event_type), Some("drawer"), Some(shift_id), reason).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn expected_cash_formula() { assert_eq!(expected_cash(10000, 5000, 1000, 2000, 500), 15500); }
    #[test] fn over_when_more() { assert_eq!(over_short(16000, 15500), 500); }
    #[test] fn short_when_less() { assert_eq!(over_short(15000, 15500), -500); }
    #[test] fn balanced_is_zero() { assert_eq!(over_short(15500, 15500), 0); }
}
