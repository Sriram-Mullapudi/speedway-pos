use crate::models::{CatalogRow, Category};
use crate::AppState;
use serde::Deserialize;

/// Full catalog with department name, on-hand, and reorder level joined in.
/// Margin is derived on the frontend from price/cost.
#[tauri::command]
pub async fn list_catalog(
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
         LEFT JOIN inventory  i ON i.product_id = p.id \
         ORDER BY p.name",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_categories(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Category>, String> {
    sqlx::query_as::<_, Category>("SELECT id, name FROM categories ORDER BY name")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct ProductInput {
    pub id: Option<i64>, // present = update, absent = insert
    pub sku: String,
    pub barcode: Option<String>,
    pub name: String,
    pub category_id: Option<i64>,
    pub price: i64,
    pub cost: i64,
    pub tax_rate: f64,
    pub age_restricted: bool,
    pub reorder_level: i64,
    #[serde(default)]
    pub bonus_points: i64,
    #[serde(default = "default_promo")]
    pub promo_type: String,
    #[serde(default)]
    pub promo_value: i64,
}

fn default_promo() -> String { "none".into() }

/// Insert or update a product. On insert, also creates its inventory row.
#[tauri::command]
pub async fn upsert_product(
    state: tauri::State<'_, AppState>,
    input: ProductInput,
) -> Result<i64, String> {
    if input.name.trim().is_empty() || input.sku.trim().is_empty() {
        return Err("SKU and name are required".into());
    }

    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

    let product_id = if let Some(id) = input.id {
        sqlx::query(
            "UPDATE products SET sku = ?1, barcode = ?2, name = ?3, category_id = ?4, \
             price = ?5, cost = ?6, tax_rate = ?7, age_restricted = ?8, \
             bonus_points = ?9, promo_type = ?10, promo_value = ?11, \
             updated_at = datetime('now') WHERE id = ?12",
        )
        .bind(&input.sku).bind(&input.barcode).bind(&input.name).bind(input.category_id)
        .bind(input.price).bind(input.cost).bind(input.tax_rate).bind(input.age_restricted)
        .bind(input.bonus_points).bind(&input.promo_type).bind(input.promo_value)
        .bind(id)
        .execute(&mut *tx).await
        .map_err(|e| friendly(e))?;
        id
    } else {
        let res = sqlx::query(
            "INSERT INTO products \
             (sku, barcode, name, category_id, price, cost, tax_rate, age_restricted, \
              bonus_points, promo_type, promo_value) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )
        .bind(&input.sku).bind(&input.barcode).bind(&input.name).bind(input.category_id)
        .bind(input.price).bind(input.cost).bind(input.tax_rate).bind(input.age_restricted)
        .bind(input.bonus_points).bind(&input.promo_type).bind(input.promo_value)
        .execute(&mut *tx).await
        .map_err(|e| friendly(e))?;
        res.last_insert_rowid()
    };

    // Upsert the inventory row's reorder level (create with 0 on-hand if new).
    sqlx::query(
        "INSERT INTO inventory (product_id, quantity_on_hand, reorder_level) \
         VALUES (?1, 0, ?2) \
         ON CONFLICT(product_id) DO UPDATE SET reorder_level = excluded.reorder_level",
    )
    .bind(product_id).bind(input.reorder_level)
    .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(product_id)
}

/// Soft delete / restore. We never hard-delete — transaction history references
/// products, and an audit trail must stay intact.
#[tauri::command]
pub async fn set_product_active(
    state: tauri::State<'_, AppState>,
    id: i64,
    active: bool,
) -> Result<(), String> {
    sqlx::query("UPDATE products SET active = ?1, updated_at = datetime('now') WHERE id = ?2")
        .bind(active).bind(id)
        .execute(&state.pool).await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn friendly(e: sqlx::Error) -> String {
    let s = e.to_string();
    if s.contains("UNIQUE") && s.contains("sku") {
        "That SKU is already in use".into()
    } else {
        s
    }
}

/// Open-price items (one per department + a generic Misc) used by the
/// register's manual price entry. Hidden from normal product search.
#[tauri::command]
pub async fn list_open_items(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::models::Product>, String> {
    sqlx::query_as::<_, crate::models::Product>(
        "SELECT id, sku, name, price, cost, tax_rate, age_restricted, \
                bonus_points, promo_type, promo_value, category_id, open_price \
         FROM products WHERE active = 1 AND open_price = 1 ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}
