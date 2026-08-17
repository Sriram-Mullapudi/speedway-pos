use serde::Serialize;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Product {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub price: i64, // cents
    pub cost: i64,  // cents
    pub tax_rate: f64,
    pub age_restricted: bool,
    pub bonus_points: i64,
    pub promo_type: String,
    pub promo_value: i64,
    pub category_id: Option<i64>,
    pub open_price: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SaleSummary {
    pub id: i64,
    pub total: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Category {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CatalogRow {
    pub id: i64,
    pub sku: String,
    pub barcode: Option<String>,
    pub name: String,
    pub category_id: Option<i64>,
    pub department: Option<String>,
    pub price: i64,
    pub cost: i64,
    pub tax_rate: f64,
    pub age_restricted: bool,
    pub active: bool,
    pub on_hand: i64,
    pub reorder_level: i64,
    pub bonus_points: i64,
    pub promo_type: String,
    pub promo_value: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Movement {
    pub id: i64,
    pub product_id: i64,
    pub delta: i64,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub session_id: i64,
    pub cashier_id: i64,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Cashier {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Shift {
    pub id: i64,
    pub register_id: i64,
    pub cashier_id: i64,
    pub opening_float: i64,
    pub counted_cash: Option<i64>,
    pub expected_cash: Option<i64>,
    pub over_short: Option<i64>,
    pub status: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
}
