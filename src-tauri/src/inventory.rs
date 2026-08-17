use crate::models::{CatalogRow, Movement};
use crate::AppState;

/// Adjust on-hand and record the reason in the ledger. One DB transaction.
/// `reason` is 'receive' | 'adjust' | 'count'.
#[tauri::command]
pub async fn adjust_stock(
    state: tauri::State<'_, AppState>,
    product_id: i64,
    delta: i64,
    reason: String,
    user_id: i64,
) -> Result<(), String> {
    if !matches!(reason.as_str(), "receive" | "adjust" | "count") {
        return Err("Invalid stock reason".into());
    }

    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE inventory SET quantity_on_hand = quantity_on_hand + ?1 WHERE product_id = ?2",
    )
    .bind(delta).bind(product_id)
    .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO inventory_movements (product_id, delta, reason, user_id) \
         VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(product_id).bind(delta).bind(&reason).bind(user_id)
    .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())
}

/// Products at or below their reorder level (and reorder tracking is on).
#[tauri::command]
pub async fn list_low_stock(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CatalogRow>, String> {
    sqlx::query_as::<_, CatalogRow>(
        "SELECT p.id, p.sku, p.barcode, p.name, p.category_id, \
                c.name AS department, p.price, p.cost, p.tax_rate, \
                p.age_restricted, p.active, \
                COALESCE(i.quantity_on_hand, 0) AS on_hand, \
                COALESCE(i.reorder_level, 0)    AS reorder_level, \
                p.bonus_points, p.promo_type, p.promo_value \
         FROM products p \
         LEFT JOIN categories c ON c.id = p.category_id \
         JOIN inventory i ON i.product_id = p.id \
         WHERE p.active = 1 AND i.reorder_level > 0 \
               AND i.quantity_on_hand <= i.reorder_level \
         ORDER BY (i.quantity_on_hand - i.reorder_level)",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}

/// Recent ledger entries for one product.
#[tauri::command]
pub async fn list_movements(
    state: tauri::State<'_, AppState>,
    product_id: i64,
) -> Result<Vec<Movement>, String> {
    sqlx::query_as::<_, Movement>(
        "SELECT id, product_id, delta, reason, created_at \
         FROM inventory_movements WHERE product_id = ?1 ORDER BY id DESC LIMIT 50",
    )
    .bind(product_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}
