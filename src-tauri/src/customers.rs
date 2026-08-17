use crate::AppState;
use serde::Serialize;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub phone: String,
    pub email: Option<String>,
    pub loyalty_points: i64,
    pub created_at: String,
}

/// Keep only digits; drop a leading US country code. Stored form is 10 digits.
fn normalize_phone(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 11 && digits.starts_with('1') {
        digits[1..].to_string()
    } else {
        digits
    }
}

#[tauri::command]
pub async fn find_customer_by_phone(
    state: tauri::State<'_, AppState>,
    phone: String,
) -> Result<Option<Customer>, String> {
    sqlx::query_as::<_, Customer>(
        "SELECT id, name, phone, email, loyalty_points, created_at \
         FROM customers WHERE phone = ?1",
    )
    .bind(normalize_phone(&phone))
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())
}

/// Live search for the register: matches by name (contains) or by any part
/// of the phone digits — so the last 4 digits are enough.
#[tauri::command]
pub async fn search_customers(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<Customer>, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let digits: String = q.chars().filter(|c| c.is_ascii_digit()).collect();
    let phone_like = if digits.is_empty() { "§none§".to_string() } else { format!("%{}%", digits) };
    let name_like = format!("%{}%", q);
    sqlx::query_as::<_, Customer>(
        "SELECT id, name, phone, email, loyalty_points, created_at \
         FROM customers WHERE name LIKE ?1 OR phone LIKE ?2 ORDER BY name LIMIT 20",
    )
    .bind(name_like)
    .bind(phone_like)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_customer(
    state: tauri::State<'_, AppState>,
    name: String,
    phone: String,
) -> Result<Customer, String> {
    let phone = normalize_phone(&phone);
    if phone.len() != 10 {
        return Err("Enter a valid 10-digit US phone number".into());
    }
    if name.trim().is_empty() {
        return Err("Name is required".into());
    }
    sqlx::query("INSERT INTO customers (name, phone) VALUES (?1, ?2)")
        .bind(name.trim())
        .bind(&phone)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") { "That phone number is already registered".to_string() }
            else { e.to_string() }
        })?;
    find_customer_by_phone(state, phone)
        .await?
        .ok_or_else(|| "Failed to create customer".into())
}

#[tauri::command]
pub async fn list_customers(state: tauri::State<'_, AppState>) -> Result<Vec<Customer>, String> {
    sqlx::query_as::<_, Customer>(
        "SELECT id, name, phone, email, loyalty_points, created_at \
         FROM customers ORDER BY name LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())
}
