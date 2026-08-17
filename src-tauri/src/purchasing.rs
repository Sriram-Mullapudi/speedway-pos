//! Phase 13 — vendors, purchase orders, receiving, counts, cost history.
//! Every stock change writes an append-only inventory_movements row inside the
//! same DB transaction as its parent operation. Receiving updates current cost
//! (recorded in product_cost_history) but never rewrites historical sale cost.

use crate::security::role_can;
use crate::{audit, AppState};
use serde::{Deserialize, Serialize};

// ---- pack/case conversion (pure, unit-tested) ------------------------------

/// Convert a quantity of cases into selling units. Explicit and auditable —
/// never hidden in UI arithmetic. 3 cases of pack_size 24 = 72 units.
pub fn cases_to_units(cases: i64, pack_size: i64) -> i64 {
    cases * pack_size.max(1)
}

/// Extended cost for a PO line: cases × unit(case) cost.
pub fn extended_cost(cases: i64, unit_cost: i64) -> i64 {
    cases * unit_cost
}

#[cfg(test)]
mod conversion_tests {
    use super::*;
    #[test]
    fn three_cases_of_24_is_72_units() {
        assert_eq!(cases_to_units(3, 24), 72);
    }
    #[test]
    fn pack_size_zero_treated_as_one() {
        assert_eq!(cases_to_units(5, 0), 5);
    }
    #[test]
    fn extended_cost_multiplies() {
        assert_eq!(extended_cost(3, 1250), 3750);
    }
}

// ---- vendors ---------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct Vendor {
    pub id: i64, pub name: String, pub contact: Option<String>,
    pub phone: Option<String>, pub email: Option<String>,
    pub account_no: Option<String>, pub notes: Option<String>, pub active: bool,
}

#[derive(Deserialize)]
pub struct VendorInput {
    pub id: Option<i64>, pub name: String, pub contact: Option<String>,
    pub phone: Option<String>, pub email: Option<String>,
    pub account_no: Option<String>, pub notes: Option<String>,
}

fn require(state: &tauri::State<'_, AppState>, action: &str) -> Result<crate::models::SessionInfo, String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;
    if !role_can(&sess.role, action) {
        return Err(format!("'{}' requires a manager", action));
    }
    Ok(sess)
}

#[tauri::command]
pub async fn list_vendors(state: tauri::State<'_, AppState>) -> Result<Vec<Vendor>, String> {
    sqlx::query_as::<_, Vendor>(
        "SELECT id, name, contact, phone, email, account_no, notes, active FROM vendors ORDER BY active DESC, name",
    ).fetch_all(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_vendor(state: tauri::State<'_, AppState>, input: VendorInput) -> Result<i64, String> {
    let sess = require(&state, "settings")?;
    if input.name.trim().is_empty() { return Err("Vendor name is required".into()); }
    let id = if let Some(id) = input.id {
        sqlx::query("UPDATE vendors SET name=?1, contact=?2, phone=?3, email=?4, account_no=?5, notes=?6 WHERE id=?7")
            .bind(&input.name).bind(&input.contact).bind(&input.phone).bind(&input.email)
            .bind(&input.account_no).bind(&input.notes).bind(id)
            .execute(&state.pool).await.map_err(|e| e.to_string())?;
        id
    } else {
        let r = sqlx::query("INSERT INTO vendors (name, contact, phone, email, account_no, notes) VALUES (?1,?2,?3,?4,?5,?6)")
            .bind(&input.name).bind(&input.contact).bind(&input.phone).bind(&input.email)
            .bind(&input.account_no).bind(&input.notes)
            .execute(&state.pool).await.map_err(|e| e.to_string())?;
        r.last_insert_rowid()
    };
    audit::write(&state.pool, Some(sess.cashier_id), "vendor.upsert", Some("vendor"), Some(id), None).await;
    Ok(id)
}

#[tauri::command]
pub async fn set_vendor_active(state: tauri::State<'_, AppState>, id: i64, active: bool) -> Result<(), String> {
    let sess = require(&state, "settings")?;
    sqlx::query("UPDATE vendors SET active=?1 WHERE id=?2").bind(active).bind(id)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "vendor.active", Some("vendor"), Some(id),
        Some(format!("active={}", active))).await;
    Ok(())
}

// ---- purchase orders -------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct PoRow {
    pub id: i64, pub vendor: String, pub reference: Option<String>,
    pub status: String, pub created_at: String, pub line_count: i64, pub total_cost: i64,
}

#[derive(Deserialize)]
pub struct PoLineInput { pub product_id: i64, pub vendor_sku: Option<String>, pub qty_ordered: i64, pub unit_cost: i64, pub pack_size: i64 }

#[tauri::command]
pub async fn list_purchase_orders(state: tauri::State<'_, AppState>) -> Result<Vec<PoRow>, String> {
    sqlx::query_as::<_, PoRow>(
        "SELECT po.id, v.name AS vendor, po.reference, po.status, po.created_at, \
                COUNT(l.id) AS line_count, COALESCE(SUM(l.qty_ordered * l.unit_cost),0) AS total_cost \
         FROM purchase_orders po JOIN vendors v ON v.id = po.vendor_id \
         LEFT JOIN purchase_order_lines l ON l.po_id = po.id \
         GROUP BY po.id ORDER BY po.id DESC LIMIT 100",
    ).fetch_all(&state.pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_purchase_order(
    state: tauri::State<'_, AppState>,
    vendor_id: i64, reference: Option<String>, notes: Option<String>, lines: Vec<PoLineInput>,
) -> Result<i64, String> {
    let sess = require(&state, "settings")?;
    if lines.is_empty() { return Err("A purchase order needs at least one line".into()); }
    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;
    let r = sqlx::query("INSERT INTO purchase_orders (vendor_id, reference, notes, created_by, status) VALUES (?1,?2,?3,?4,'draft')")
        .bind(vendor_id).bind(&reference).bind(&notes).bind(sess.cashier_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    let po_id = r.last_insert_rowid();
    for l in &lines {
        if l.qty_ordered <= 0 { return Err("Order quantity must be positive".into()); }
        sqlx::query("INSERT INTO purchase_order_lines (po_id, product_id, vendor_sku, qty_ordered, unit_cost, pack_size) VALUES (?1,?2,?3,?4,?5,?6)")
            .bind(po_id).bind(l.product_id).bind(&l.vendor_sku).bind(l.qty_ordered).bind(l.unit_cost).bind(l.pack_size.max(1))
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "po.create", Some("purchase_order"), Some(po_id), None).await;
    Ok(po_id)
}

#[tauri::command]
pub async fn set_po_status(state: tauri::State<'_, AppState>, po_id: i64, status: String) -> Result<(), String> {
    let sess = require(&state, "settings")?;
    if !matches!(status.as_str(), "ordered" | "cancelled" | "closed") {
        return Err("Invalid status transition".into());
    }
    sqlx::query("UPDATE purchase_orders SET status=?1 WHERE id=?2").bind(&status).bind(po_id)
        .execute(&state.pool).await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), &format!("po.{}", status), Some("purchase_order"), Some(po_id), None).await;
    Ok(())
}

#[derive(Deserialize)]
pub struct ReceiveLine { pub line_id: i64, pub cases_received: i64 }

/// Receive (fully or partially) against a PO. Converts cases→units, writes an
/// append-only movement per line, updates on-hand, updates current product cost
/// (recording cost history), and never touches historical sale cost.
/// Over-receiving beyond ordered quantity is rejected.
#[tauri::command]
pub async fn receive_purchase_order(
    state: tauri::State<'_, AppState>,
    po_id: i64, receipts: Vec<ReceiveLine>,
) -> Result<String, String> {
    let sess = require(&state, "settings")?;
    if receipts.is_empty() { return Err("Nothing to receive".into()); }
    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;
    let mut received_units = 0i64;

    for r in &receipts {
        if r.cases_received <= 0 { continue; }
        let (product_id, qty_ordered, qty_received, unit_cost, pack_size): (i64, i64, i64, i64, i64) =
            sqlx::query_as("SELECT product_id, qty_ordered, qty_received, unit_cost, pack_size FROM purchase_order_lines WHERE id=?1 AND po_id=?2")
                .bind(r.line_id).bind(po_id)
                .fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?
                .ok_or("PO line not found")?;

        if qty_received + r.cases_received > qty_ordered {
            return Err("Cannot receive more than ordered".into());
        }
        let units = cases_to_units(r.cases_received, pack_size);
        received_units += units;

        // update received count
        sqlx::query("UPDATE purchase_order_lines SET qty_received = qty_received + ?1 WHERE id=?2")
            .bind(r.cases_received).bind(r.line_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;

        // on-hand + ledger movement
        sqlx::query("INSERT INTO inventory (product_id, quantity_on_hand, reorder_level) VALUES (?1, 0, 0) ON CONFLICT(product_id) DO NOTHING")
            .bind(product_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        sqlx::query("UPDATE inventory SET quantity_on_hand = quantity_on_hand + ?1 WHERE product_id = ?2")
            .bind(units).bind(product_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        sqlx::query("INSERT INTO inventory_movements (product_id, delta, reason, ref_type, ref_id, user_id) VALUES (?1,?2,'receive','purchase_order',?3,?4)")
            .bind(product_id).bind(units).bind(po_id).bind(sess.cashier_id)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;

        // current cost update per unit (case cost / pack), with history — never rewrites sales
        let per_unit_cost = unit_cost / pack_size.max(1);
        let prior: i64 = sqlx::query_scalar("SELECT cost FROM products WHERE id=?1")
            .bind(product_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        if per_unit_cost > 0 && per_unit_cost != prior {
            sqlx::query("UPDATE products SET cost=?1, updated_at=datetime('now') WHERE id=?2")
                .bind(per_unit_cost).bind(product_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
            sqlx::query("INSERT INTO product_cost_history (product_id, prior_cost, new_cost, source, ref_type, ref_id, user_id) VALUES (?1,?2,?3,'receiving','purchase_order',?4,?5)")
                .bind(product_id).bind(prior).bind(per_unit_cost).bind(po_id).bind(sess.cashier_id)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
    }

    // status: partial vs received
    let (ordered, received): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(qty_ordered),0), COALESCE(SUM(qty_received),0) FROM purchase_order_lines WHERE po_id=?1")
        .bind(po_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
    let new_status = if received >= ordered { "received" } else { "partial" };
    sqlx::query("UPDATE purchase_orders SET status=?1 WHERE id=?2").bind(new_status).bind(po_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "po.receive", Some("purchase_order"), Some(po_id),
        Some(format!("units={}", received_units))).await;
    Ok(format!("Received {} units", received_units))
}

// ---- adjustments & counts --------------------------------------------------

#[tauri::command]
pub async fn adjust_inventory(
    state: tauri::State<'_, AppState>,
    product_id: i64, delta: i64, reason_code: String,
) -> Result<(), String> {
    let sess = require(&state, "settings")?;
    if !matches!(reason_code.as_str(), "damage" | "spoilage" | "shrink" | "correction") {
        return Err("Invalid adjustment reason".into());
    }
    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE inventory SET quantity_on_hand = quantity_on_hand + ?1 WHERE product_id = ?2")
        .bind(delta).bind(product_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO inventory_movements (product_id, delta, reason, ref_type, user_id) VALUES (?1,?2,?3,'adjustment',?4)")
        .bind(product_id).bind(delta).bind(&reason_code).bind(sess.cashier_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    audit::write(&state.pool, Some(sess.cashier_id), "inventory.adjust", Some("product"), Some(product_id),
        Some(format!("{}:{}", reason_code, delta))).await;
    Ok(())
}

// ---- reorder suggestions ---------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct ReorderRow {
    pub product_id: i64, pub name: String, pub on_hand: i64,
    pub reorder_level: i64, pub min_stock: i64, pub pack_size: i64,
    pub suggested_cases: i64, pub vendor: Option<String>,
}

/// Managers see suggestions; we never auto-order. Suggested cases = enough to
/// bring on-hand up to (reorder_level + pack_size), rounded to whole cases.
#[tauri::command]
pub async fn reorder_suggestions(state: tauri::State<'_, AppState>) -> Result<Vec<ReorderRow>, String> {
    sqlx::query_as::<_, ReorderRow>(
        "SELECT p.id AS product_id, p.name, COALESCE(i.quantity_on_hand,0) AS on_hand, \
                COALESCE(i.reorder_level,0) AS reorder_level, p.min_stock, p.pack_size, \
                CAST( (MAX(0, (COALESCE(i.reorder_level,0) + p.pack_size) - COALESCE(i.quantity_on_hand,0)) + p.pack_size - 1) / p.pack_size AS INTEGER) AS suggested_cases, \
                v.name AS vendor \
         FROM products p LEFT JOIN inventory i ON i.product_id = p.id \
         LEFT JOIN vendors v ON v.id = p.preferred_vendor_id \
         WHERE p.active = 1 AND COALESCE(i.quantity_on_hand,0) <= COALESCE(i.reorder_level,0) \
         ORDER BY on_hand ASC LIMIT 100",
    ).fetch_all(&state.pool).await.map_err(|e| e.to_string())
}
