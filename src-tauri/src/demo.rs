//! Demo mode: one-click reset that wipes transactional data and seeds a week
//! of realistic history (shifts, sales, customers, loyalty, promos, drawer
//! events, audit entries) so the app demos well repeatedly.
//! Deterministic — no rand dependency; variety comes from simple arithmetic.

use crate::security::role_can;
use crate::AppState;
use sqlx::SqlitePool;

async fn cashier_id_by_role(pool: &SqlitePool, role: &str) -> Result<i64, String> {
    sqlx::query_scalar("SELECT id FROM cashiers WHERE role = ?1 AND active = 1 LIMIT 1")
        .bind(role)
        .fetch_one(pool)
        .await
        .map_err(|_| format!("No active {} account found", role))
}

#[tauri::command]
pub async fn reset_demo_data(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;
    if !role_can(&sess.role, "settings") {
        return Err("Demo reset requires a manager".into());
    }
    let pool = &state.pool;

    // ---- wipe transactional data (catalog and cashiers survive) ----
    for t in [
        "payments", "transaction_items", "transactions", "cash_drawer_events",
        "shifts", "inventory_movements", "suspended_sales", "customers", "audit_log",
    ] {
        sqlx::query(&format!("DELETE FROM {}", t))
            .execute(pool).await.map_err(|e| e.to_string())?;
    }
    sqlx::query("UPDATE inventory SET quantity_on_hand = 48")
        .execute(pool).await.map_err(|e| e.to_string())?;

    // ---- make the catalog demo-worthy: promos + bonus points on staples ----
    sqlx::query("UPDATE products SET promo_type='bogo', promo_value=0 WHERE sku='0012'") // Doritos
        .execute(pool).await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE products SET promo_type='second_pct', promo_value=30 WHERE sku='0005'") // Budweiser
        .execute(pool).await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE products SET bonus_points=10 WHERE sku IN ('0008','0009')") // spirits
        .execute(pool).await.map_err(|e| e.to_string())?;

    // ---- customers with loyalty history ----
    let customers: [(&str, &str, i64); 5] = [
        ("Maria Lopez", "8135550101", 430),
        ("James Carter", "8135550102", 620),
        ("Priya Patel", "8135550103", 120),
        ("Dan Nguyen", "8135550104", 45),
        ("Ashley Brooks", "8135550105", 510),
    ];
    let mut customer_ids = Vec::new();
    for (name, phone, pts) in customers {
        let r = sqlx::query("INSERT INTO customers (name, phone, loyalty_points) VALUES (?1, ?2, ?3)")
            .bind(name).bind(phone).bind(pts)
            .execute(pool).await.map_err(|e| e.to_string())?;
        customer_ids.push(r.last_insert_rowid());
    }

    // ---- a week of shifts + sales ----
    let cashier = cashier_id_by_role(pool, "cashier").await?;
    let manager = cashier_id_by_role(pool, "manager").await?;

    #[derive(sqlx::FromRow)]
    struct P { id: i64, price: i64, tax_rate: f64, promo_type: String, promo_value: i64, bonus_points: i64 }
    let products = sqlx::query_as::<_, P>(
        "SELECT id, price, tax_rate, promo_type, promo_value, bonus_points FROM products WHERE active = 1 ORDER BY id",
    )
    .fetch_all(pool).await.map_err(|e| e.to_string())?;
    if products.is_empty() {
        return Err("No products to seed sales from".into());
    }

    let mut total_sales = 0i64;
    for day in (0..7).rev() {
        let day_mod = format!("-{} days", day);
        let who = if day % 3 == 0 { manager } else { cashier };

        let r = sqlx::query(
            "INSERT INTO shifts (register_id, cashier_id, opening_float, status, opened_at) \
             VALUES (1, ?1, 10000, 'open', datetime('now', ?2, '+8 hours'))",
        )
        .bind(who).bind(&day_mod)
        .execute(pool).await.map_err(|e| e.to_string())?;
        let shift_id = r.last_insert_rowid();

        sqlx::query(
            "INSERT INTO cash_drawer_events (cashier_id, shift_id, event_type, amount, reason, created_at) \
             VALUES (?1, ?2, 'shift_open', 10000, 'Opening float', datetime('now', ?3, '+8 hours'))",
        )
        .bind(who).bind(shift_id).bind(&day_mod)
        .execute(pool).await.map_err(|e| e.to_string())?;

        let n_sales = 5 + (day * 2) % 6;
        let mut cash_taken = 0i64;

        for k in 0..n_sales {
            let hour_mod = format!("+{} hours", 9 + (k * 11) % 12);
            let p = &products[((day * 7 + k * 3) as usize) % products.len()];
            let qty = 1 + (k % 2) as i64;

            let line_total = crate::pricing::promo_line_total(p.price, qty, &p.promo_type, p.promo_value);
            let tax = crate::pricing::line_tax(line_total, p.tax_rate);
            let subtotal = line_total;
            let total = subtotal + tax;

            let customer_id = if k % 4 == 0 { Some(customer_ids[((day + k) as usize) % customer_ids.len()]) } else { None };
            let points_delta = if customer_id.is_some() {
                crate::pricing::loyalty_earned(total, p.bonus_points * qty)
            } else { 0 };

            let tr = sqlx::query(
                "INSERT INTO transactions \
                 (cashier_id, shift_id, type, status, subtotal, tax, total, customer_id, discount, points_delta, created_at) \
                 VALUES (?1, ?2, 'sale', 'completed', ?3, ?4, ?5, ?6, 0, ?7, datetime('now', ?8, ?9))",
            )
            .bind(who).bind(shift_id).bind(subtotal).bind(tax).bind(total)
            .bind(customer_id).bind(points_delta).bind(&day_mod).bind(&hour_mod)
            .execute(pool).await.map_err(|e| e.to_string())?;
            let txn_id = tr.last_insert_rowid();

            sqlx::query(
                "INSERT INTO transaction_items (transaction_id, product_id, qty, unit_price, line_total) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(txn_id).bind(p.id).bind(qty).bind(p.price).bind(line_total)
            .execute(pool).await.map_err(|e| e.to_string())?;

            sqlx::query("UPDATE inventory SET quantity_on_hand = quantity_on_hand - ?1 WHERE product_id = ?2")
                .bind(qty).bind(p.id)
                .execute(pool).await.map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO inventory_movements (product_id, delta, reason, ref_type, ref_id, user_id, created_at) \
                 VALUES (?1, ?2, 'sale', 'transaction', ?3, ?4, datetime('now', ?5, ?6))",
            )
            .bind(p.id).bind(-qty).bind(txn_id).bind(who).bind(&day_mod).bind(&hour_mod)
            .execute(pool).await.map_err(|e| e.to_string())?;

            let kind = if (k + day) % 2 == 0 { "cash" } else { "card" };
            if kind == "cash" { cash_taken += total; }
            sqlx::query(
                "INSERT INTO payments (transaction_id, kind, amount, tendered, change) \
                 VALUES (?1, ?2, ?3, ?3, 0)",
            )
            .bind(txn_id).bind(kind).bind(total)
            .execute(pool).await.map_err(|e| e.to_string())?;

            if let Some(cid) = customer_id {
                sqlx::query("UPDATE customers SET loyalty_points = loyalty_points + ?1 WHERE id = ?2")
                    .bind(points_delta).bind(cid)
                    .execute(pool).await.map_err(|e| e.to_string())?;
            }
            total_sales += 1;
        }

        // close the shift with a small, varied over/short
        let expected = 10000 + cash_taken;
        let counted = expected + ((day % 3) as i64 - 1) * 137; // -137 / 0 / +137
        sqlx::query(
            "UPDATE shifts SET status='closed', counted_cash=?1, expected_cash=?2, over_short=?3, \
             closed_at = datetime('now', ?4, '+20 hours') WHERE id = ?5",
        )
        .bind(counted).bind(expected).bind(counted - expected).bind(&day_mod).bind(shift_id)
        .execute(pool).await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO cash_drawer_events (cashier_id, shift_id, event_type, amount, reason, created_at) \
             VALUES (?1, ?2, 'shift_close', ?3, 'Closing count', datetime('now', ?4, '+20 hours'))",
        )
        .bind(who).bind(shift_id).bind(counted).bind(&day_mod)
        .execute(pool).await.map_err(|e| e.to_string())?;

        // sprinkle audit history
        sqlx::query(
            "INSERT INTO audit_log (user_id, action, entity, entity_id, created_at) \
             VALUES (?1, 'auth.login.success', 'cashier', ?1, datetime('now', ?2, '+8 hours'))",
        )
        .bind(who).bind(&day_mod)
        .execute(pool).await.map_err(|e| e.to_string())?;
        if day % 2 == 0 {
            sqlx::query(
                "INSERT INTO cash_drawer_events (cashier_id, shift_id, event_type, amount, reason, manager_approved_by, created_at) \
                 VALUES (?1, ?2, 'no_sale', 0, 'Customer change request', ?3, datetime('now', ?4, '+14 hours'))",
            )
            .bind(who).bind(shift_id).bind(manager).bind(&day_mod)
            .execute(pool).await.map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO audit_log (user_id, action, entity, detail, created_at) \
                 VALUES (?1, 'drawer.no_sale', 'drawer', 'Customer change request', datetime('now', ?2, '+14 hours'))",
            )
            .bind(who).bind(&day_mod)
            .execute(pool).await.map_err(|e| e.to_string())?;
        }
    }

    crate::audit::write(pool, Some(sess.cashier_id), "demo.reset", Some("demo"), None,
        Some(format!("{{\"sales_seeded\":{}}}", total_sales))).await;

    Ok(format!("Demo reset complete — {} sales across 7 days, 5 loyalty customers, promos applied.", total_sales))
}
