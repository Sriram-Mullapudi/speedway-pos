use crate::models::{Product, SaleSummary};
use crate::AppState;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SaleLine {
    pub product_id: i64,
    pub qty: i64,
    #[serde(default)]
    pub manual_price: Option<i64>, // only honored for open_price items
}

#[derive(Deserialize)]
pub struct Tender {
    pub kind: String, // "cash" | "card"
    pub tendered: i64,
}

#[derive(Deserialize)]
pub struct CreateSalePayload {
    pub items: Vec<SaleLine>,
    pub tender: Tender,
    #[serde(default)]
    pub age_verified: bool,
    #[serde(default)]
    pub customer_id: Option<i64>,
    #[serde(default)]
    pub redeem_points: bool,
}

#[derive(Serialize)]
pub struct ReceiptItem {
    pub name: String,
    pub qty: i64,
    pub unit_price: i64,
    pub line_total: i64,
}

#[derive(Serialize)]
pub struct Receipt {
    pub id: i64,
    pub store_name: String,
    pub footer: String,
    pub cashier: String,
    pub created_at: String,
    pub subtotal: i64,
    pub tax: i64,
    pub discount: i64,
    pub total: i64,
    pub tender_kind: String,
    pub tendered: i64,
    pub change: i64,
    pub points_earned: i64,
    pub points_redeemed: i64,
    pub points_balance: Option<i64>,
    pub items: Vec<ReceiptItem>,
}

/// Free-text search across name and SKU. Powers the scan/search box.
#[tauri::command]
pub async fn search_products(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<Product>, String> {
    let like = format!("%{}%", query.trim());
    sqlx::query_as::<_, Product>(
        "SELECT id, sku, name, price, cost, tax_rate, age_restricted, bonus_points, promo_type, promo_value, category_id, open_price \
         FROM products \
         WHERE active = 1 AND open_price = 0 AND (name LIKE ?1 OR sku LIKE ?1) \
         ORDER BY name LIMIT 50",
    )
    .bind(&like)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}

/// The heart of the system. Note the deliberate choices:
///   * Prices are re-read from the DB, never trusted from the client.
///   * The whole sale is one DB transaction — it commits or it doesn't.
///   * Age-restricted items require explicit verification.
///   * Every completed sale writes an immutable audit_log row.
#[tauri::command]
pub async fn create_sale(
    state: tauri::State<'_, AppState>,
    payload: CreateSalePayload,
) -> Result<Receipt, String> {
    if payload.items.is_empty() {
        return Err("Cart is empty".into());
    }

    let (cashier_id, cashier_name) = {
        let guard = state.session.lock().unwrap();
        guard.as_ref().map(|s| (s.cashier_id, s.name.clone()))
    }
    .ok_or("No cashier is signed in")?;

    // Business rules come from settings (with sane defaults), read in Rust.
    let loyalty_threshold = crate::settings::get_setting_i64(&state.pool, "loyalty_threshold", 500).await;
    let loyalty_reward = crate::settings::get_setting_i64(&state.pool, "loyalty_reward", 1000).await;
    let store_name = crate::settings::get_setting_str(&state.pool, "store_name", "Speedway Market").await;
    let footer = crate::settings::get_setting_str(&state.pool, "receipt_footer", "Thank you — see you soon!").await;

    // Resolve the open shift and its register. register_id is stamped onto the
    // sale from the shift (single-register today; groundwork for Phase 17's
    // full multi-register model).
    let (shift_id, register_id, register_global_id): (i64, i64, String) = sqlx::query_as(
        "SELECT id, register_id, register_global_id FROM shifts WHERE cashier_id = ?1 AND status = 'open' ORDER BY id DESC LIMIT 1",
    )
    .bind(cashier_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("No open shift — open a shift to start selling")?;

    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

    let mut subtotal = 0i64;
    let mut tax = 0i64;
    let mut bonus_pts = 0i64;
    let mut manual_lines = 0i64;
    let mut needs_age = false;
    // (product_id, name, qty, unit_price, unit_cost, line_total, line_tax)
    let mut prepared: Vec<(i64, String, i64, i64, i64, i64, i64)> = Vec::new();

    for line in &payload.items {
        if line.qty <= 0 {
            return Err("Quantity must be positive".into());
        }
        let p = sqlx::query_as::<_, Product>(
            "SELECT id, sku, name, price, cost, tax_rate, age_restricted, bonus_points, promo_type, promo_value, category_id, open_price \
             FROM products WHERE id = ?1 AND active = 1",
        )
        .bind(line.product_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| format!("Unknown product {}", line.product_id))?;

        if p.age_restricted {
            needs_age = true;
        }
        // Manual price entry is only valid for explicitly open-price items,
        // is bounded, and is audited — never a silent price override.
        let unit_price = match line.manual_price {
            Some(mp) => {
                if !p.open_price {
                    return Err(format!("{} does not allow manual pricing", p.name));
                }
                if mp <= 0 || mp > 1_000_000 {
                    return Err("Manual price out of range".into());
                }
                manual_lines += 1;
                mp
            }
            None => p.price,
        };
        let line_total = crate::pricing::promo_line_total(unit_price, line.qty, &p.promo_type, p.promo_value);
        let line_tax = crate::pricing::line_tax(line_total, p.tax_rate);
        // Historical cost captured from the authoritative product record at
        // sale time — the frontend never supplies this. Future edits to
        // products.cost must not change this row. (Rule is unit-tested as
        // pricing::historical_unit_cost.)
        let unit_cost = crate::pricing::historical_unit_cost(p.cost, None);
        bonus_pts += p.bonus_points * line.qty;
        subtotal += line_total;
        tax += line_tax;
        prepared.push((p.id, p.name, line.qty, unit_price, unit_cost, line_total, line_tax));
    }

    if needs_age && !payload.age_verified {
        // Sentinel string the UI checks for to trigger the ID prompt.
        return Err("AGE_VERIFICATION_REQUIRED".into());
    }

    let gross = subtotal + tax;

    // Loyalty: earn 1 point per $1 spent (plus per-product bonus points).
    // Reward: 500 points can be redeemed for $10 off (one reward per sale).
    // All computed server-side — the client only sends intent.
    let mut discount = 0i64;
    let mut redeemed_points = 0i64;
    if let (Some(cid), true) = (payload.customer_id, payload.redeem_points) {
        let pts: i64 = sqlx::query_scalar("SELECT loyalty_points FROM customers WHERE id = ?1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| "Unknown customer".to_string())?;
        let (d, r) = crate::pricing::loyalty_redemption(pts, true, loyalty_threshold, loyalty_reward, gross)?;
        discount = d;
        redeemed_points = r;
    }
    let total = gross - discount;
    let points_earned = crate::pricing::loyalty_earned(total, bonus_pts);
    let points_delta = points_earned - redeemed_points;

    if payload.tender.tendered < total {
        return Err("Insufficient tender".into());
    }
    let change = if payload.tender.kind == "cash" {
        payload.tender.tendered - total
    } else {
        0
    };

    let res = sqlx::query(
        "INSERT INTO transactions \
         (cashier_id, shift_id, register_id, register_global_id, type, status, subtotal, tax, total, customer_id, discount, points_delta) \
         VALUES (?1, ?2, ?3, ?10, 'sale', 'completed', ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(cashier_id)
    .bind(shift_id)
    .bind(register_id)
    .bind(subtotal)
    .bind(tax)
    .bind(total)
    .bind(payload.customer_id)
    .bind(discount)
    .bind(points_delta)
    .bind(&register_global_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let tx_id = res.last_insert_rowid();

    let mut receipt_items = Vec::new();
    for (pid, name, qty, unit_price, unit_cost, line_total, line_tax) in prepared {
        sqlx::query(
            "INSERT INTO transaction_items \
             (transaction_id, product_id, qty, unit_price, unit_cost, line_total, tax_amount) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(tx_id)
        .bind(pid)
        .bind(qty)
        .bind(unit_price)
        .bind(unit_cost)
        .bind(line_total)
        .bind(line_tax)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "UPDATE inventory SET quantity_on_hand = quantity_on_hand - ?1 \
             WHERE product_id = ?2",
        )
        .bind(qty)
        .bind(pid)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        // Record the decrement in the append-only ledger.
        sqlx::query(
            "INSERT INTO inventory_movements \
             (product_id, delta, reason, ref_type, ref_id, user_id) \
             VALUES (?1, ?2, 'sale', 'transaction', ?3, ?4)",
        )
        .bind(pid)
        .bind(-qty)
        .bind(tx_id)
        .bind(cashier_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        receipt_items.push(ReceiptItem { name, qty, unit_price, line_total });
    }

    sqlx::query(
        "INSERT INTO payments (transaction_id, kind, amount, tendered, change) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(tx_id)
    .bind(&payload.tender.kind)
    .bind(total)
    .bind(payload.tender.tendered)
    .bind(change)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    let detail = serde_json::json!({
        "transaction_id": tx_id,
        "total": total,
        "age_verified": payload.age_verified
    })
    .to_string();
    sqlx::query(
        "INSERT INTO audit_log (user_id, action, entity, detail) \
         VALUES (?1, 'sale.completed', 'transaction', ?2)",
    )
    .bind(cashier_id)
    .bind(detail)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    let mut points_balance: Option<i64> = None;
    if let Some(cid) = payload.customer_id {
        sqlx::query("UPDATE customers SET loyalty_points = loyalty_points + ?1 WHERE id = ?2")
            .bind(points_delta)
            .bind(cid)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        points_balance = sqlx::query_scalar("SELECT loyalty_points FROM customers WHERE id = ?1")
            .bind(cid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    let created_at: String = sqlx::query_scalar("SELECT created_at FROM transactions WHERE id = ?1")
        .bind(tx_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    if manual_lines > 0 {
        crate::audit::write(&state.pool, Some(cashier_id), "price.manual", Some("transaction"), Some(tx_id),
            Some(format!("{{\"manual_lines\":{}}}", manual_lines))).await;
    }

    Ok(Receipt {
        id: tx_id,
        store_name,
        footer,
        cashier: cashier_name,
        created_at,
        subtotal,
        tax,
        discount,
        total,
        tender_kind: payload.tender.kind.clone(),
        tendered: payload.tender.tendered,
        change,
        points_earned,
        points_redeemed: redeemed_points,
        points_balance,
        items: receipt_items,
    })
}

#[tauri::command]
pub async fn list_recent_sales(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SaleSummary>, String> {
    sqlx::query_as::<_, SaleSummary>(
        "SELECT id, total, created_at FROM transactions ORDER BY id DESC LIMIT 20",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}
