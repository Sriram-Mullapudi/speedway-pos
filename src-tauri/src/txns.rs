use crate::models::SessionInfo;
use crate::security::role_can;
use crate::{audit, AppState};
use serde::Serialize;

fn current(state: &tauri::State<'_, AppState>) -> Option<SessionInfo> {
    state.current_session()
}

/// Manager gate shared by void/refund: allowed if the signed-in role can do it,
/// OR a manager already approved via the override PIN flow.
fn gate(sess: &SessionInfo, action: &str, approved: Option<i64>) -> Result<(), String> {
    if role_can(&sess.role, action) || approved.is_some() {
        Ok(())
    } else {
        Err(format!("'{}' requires a manager", action))
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TxnRow {
    pub id: i64,
    pub kind: String,        // 'sale' | 'refund'
    pub status: String,      // completed | voided | refunded
    pub total: i64,
    pub discount: i64,
    pub cashier: Option<String>,
    pub customer: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub async fn list_transactions(state: tauri::State<'_, AppState>) -> Result<Vec<TxnRow>, String> {
    sqlx::query_as::<_, TxnRow>(
        "SELECT t.id, t.type AS kind, t.status, t.total, t.discount, \
                ca.name AS cashier, cu.name AS customer, t.created_at \
         FROM transactions t \
         LEFT JOIN cashiers ca ON ca.id = t.cashier_id \
         LEFT JOIN customers cu ON cu.id = t.customer_id \
         ORDER BY t.id DESC LIMIT 50",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}

/// Void a completed sale: restore stock, reverse loyalty, mark voided.
/// One DB transaction; fully audited.
#[tauri::command]
pub async fn void_transaction(
    state: tauri::State<'_, AppState>,
    txn_id: i64,
    manager_approved_by: Option<i64>,
) -> Result<(), String> {
    let sess = current(&state).ok_or("Not signed in")?;
    gate(&sess, "void", manager_approved_by)?;

    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

    let (kind, status, customer_id, points_delta): (String, String, Option<i64>, i64) =
        sqlx::query_as("SELECT type, status, customer_id, points_delta FROM transactions WHERE id = ?1")
            .bind(txn_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| "Transaction not found".to_string())?;
    if kind != "sale" || status != "completed" {
        return Err("Only completed sales can be voided".into());
    }

    let items: Vec<(i64, i64)> =
        sqlx::query_as("SELECT product_id, qty FROM transaction_items WHERE transaction_id = ?1")
            .bind(txn_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    for (pid, qty) in items {
        sqlx::query("UPDATE inventory SET quantity_on_hand = quantity_on_hand + ?1 WHERE product_id = ?2")
            .bind(qty).bind(pid).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO inventory_movements (product_id, delta, reason, ref_type, ref_id, user_id) \
             VALUES (?1, ?2, 'void', 'transaction', ?3, ?4)",
        )
        .bind(pid).bind(qty).bind(txn_id).bind(sess.cashier_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    if let Some(cid) = customer_id {
        if points_delta != 0 {
            sqlx::query("UPDATE customers SET loyalty_points = MAX(0, loyalty_points - ?1) WHERE id = ?2")
                .bind(points_delta).bind(cid).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
    }

    sqlx::query("UPDATE transactions SET status = 'voided' WHERE id = ?1")
        .bind(txn_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    let detail = serde_json::json!({ "approved_by": manager_approved_by }).to_string();
    audit::write(&state.pool, Some(sess.cashier_id), "sale.void", Some("transaction"), Some(txn_id), Some(detail)).await;
    Ok(())
}

/// Full cash refund of a completed sale: creates a linked refund transaction,
/// restores stock, reverses loyalty, marks the original refunded.
#[tauri::command]
pub async fn refund_transaction(
    state: tauri::State<'_, AppState>,
    txn_id: i64,
    manager_approved_by: Option<i64>,
) -> Result<i64, String> {
    let sess = current(&state).ok_or("Not signed in")?;
    gate(&sess, "refund", manager_approved_by)?;

    let shift_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM shifts WHERE cashier_id = ?1 AND status = 'open' LIMIT 1")
            .bind(sess.cashier_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| e.to_string())?;

    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

    let (kind, status, subtotal, tax, total, customer_id, points_delta):
        (String, String, i64, i64, i64, Option<i64>, i64) = sqlx::query_as(
        "SELECT type, status, subtotal, tax, total, customer_id, points_delta \
         FROM transactions WHERE id = ?1",
    )
    .bind(txn_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| "Transaction not found".to_string())?;
    if kind != "sale" || status != "completed" {
        return Err("Only completed sales can be refunded".into());
    }

    let res = sqlx::query(
        "INSERT INTO transactions \
         (cashier_id, shift_id, type, status, subtotal, tax, total, original_txn_id, customer_id) \
         VALUES (?1, ?2, 'refund', 'completed', ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(sess.cashier_id).bind(shift_id).bind(subtotal).bind(tax).bind(total).bind(txn_id).bind(customer_id)
    .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    let refund_id = res.last_insert_rowid();

    // Refund goes back the way it came: cash refunds reduce the drawer
    // (the shift summary subtracts cash-kind refunds); card refunds are a
    // mocked reversal to the original card and don't touch the drawer.
    let orig_kind: String = sqlx::query_scalar(
        "SELECT kind FROM payments WHERE transaction_id = ?1 ORDER BY id LIMIT 1",
    )
    .bind(txn_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or_else(|| "cash".to_string());

    // A cash refund is money out of a physical drawer; without an open shift
    // it would vanish from the reconciliation. Card reversals don't touch cash.
    if orig_kind == "cash" && shift_id.is_none() {
        return Err("Open a shift before issuing cash refunds".into());
    }

    sqlx::query(
        "INSERT INTO payments (transaction_id, kind, amount, tendered, change) \
         VALUES (?1, ?2, ?3, ?3, 0)",
    )
    .bind(refund_id).bind(&orig_kind).bind(total)
    .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    let items: Vec<(i64, i64)> =
        sqlx::query_as("SELECT product_id, qty FROM transaction_items WHERE transaction_id = ?1")
            .bind(txn_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    for (pid, qty) in items {
        sqlx::query("UPDATE inventory SET quantity_on_hand = quantity_on_hand + ?1 WHERE product_id = ?2")
            .bind(qty).bind(pid).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO inventory_movements (product_id, delta, reason, ref_type, ref_id, user_id) \
             VALUES (?1, ?2, 'refund', 'transaction', ?3, ?4)",
        )
        .bind(pid).bind(qty).bind(refund_id).bind(sess.cashier_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    if let Some(cid) = customer_id {
        if points_delta != 0 {
            sqlx::query("UPDATE customers SET loyalty_points = MAX(0, loyalty_points - ?1) WHERE id = ?2")
                .bind(points_delta).bind(cid).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
    }

    sqlx::query("UPDATE transactions SET status = 'refunded' WHERE id = ?1")
        .bind(txn_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    let detail = serde_json::json!({ "original": txn_id, "approved_by": manager_approved_by }).to_string();
    audit::write(&state.pool, Some(sess.cashier_id), "sale.refund", Some("transaction"), Some(refund_id), Some(detail)).await;
    Ok(refund_id)
}

// ---- suspend / resume ---------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct SuspendedSale {
    pub id: i64,
    pub cashier_id: Option<i64>,
    pub cart_json: String,
    pub created_at: String,
}

#[tauri::command]
pub async fn suspend_sale(state: tauri::State<'_, AppState>, cart_json: String) -> Result<i64, String> {
    let sess = current(&state).ok_or("Not signed in")?;
    let res = sqlx::query("INSERT INTO suspended_sales (cashier_id, cart_json) VALUES (?1, ?2)")
        .bind(sess.cashier_id).bind(&cart_json)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "sale.suspend", Some("suspended_sale"), Some(res.last_insert_rowid()), None).await;
    Ok(res.last_insert_rowid())
}

#[tauri::command]
pub async fn list_suspended(state: tauri::State<'_, AppState>) -> Result<Vec<SuspendedSale>, String> {
    sqlx::query_as::<_, SuspendedSale>(
        "SELECT id, cashier_id, cart_json, created_at FROM suspended_sales ORDER BY id DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}

/// Returns the cart JSON and removes the row (resume is one-shot).
#[tauri::command]
pub async fn resume_sale(state: tauri::State<'_, AppState>, id: i64) -> Result<String, String> {
    let sess = current(&state).ok_or("Not signed in")?;
    let json: String = sqlx::query_scalar("SELECT cart_json FROM suspended_sales WHERE id = ?1")
        .bind(id).fetch_one(&state.pool).await.map_err(|_| "Suspended sale not found".to_string())?;
    sqlx::query("DELETE FROM suspended_sales WHERE id = ?1")
        .bind(id).execute(&state.pool).await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "sale.resume", Some("suspended_sale"), Some(id), None).await;
    Ok(json)
}
