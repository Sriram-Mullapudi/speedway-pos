use crate::AppState;
use serde::Serialize;

#[derive(Serialize)]
pub struct PaymentRow { pub kind: String, pub amount: i64 }
#[derive(Serialize)]
pub struct DeptRow { pub department: String, pub sales: i64 }
#[derive(Serialize)]
pub struct TopRow { pub name: String, pub qty: i64, pub revenue: i64 }

#[derive(Serialize)]
pub struct ReportData {
    pub period: String,
    pub gross: i64,
    pub tax: i64,
    pub net: i64,
    pub txn_count: i64,
    pub avg_basket: i64,
    pub by_payment: Vec<PaymentRow>,
    pub by_department: Vec<DeptRow>,
    pub top_products: Vec<TopRow>,
}

/// Fixed, non-user SQL fragment for the time window.
fn period_clause(period: &str) -> &'static str {
    match period {
        "today" => "AND t.created_at >= date('now')",
        "week" => "AND t.created_at >= date('now','-7 days')",
        "month" => "AND t.created_at >= date('now','-30 days')",
        _ => "", // 'all'
    }
}

#[tauri::command]
pub async fn get_report(
    state: tauri::State<'_, AppState>,
    period: String,
) -> Result<ReportData, String> {
    let clause = period_clause(&period);
    let pool = &state.pool;

    // Totals
    let (gross, tax, txn_count): (i64, i64, i64) = sqlx::query_as(&format!(
        "SELECT COALESCE(SUM(total),0), COALESCE(SUM(tax),0), COUNT(*) \
         FROM transactions t \
         WHERE t.type = 'sale' AND t.status = 'completed' {clause}"
    ))
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    // By payment method
    let by_payment = sqlx::query_as::<_, (String, i64)>(&format!(
        "SELECT p.kind, COALESCE(SUM(p.amount),0) \
         FROM payments p JOIN transactions t ON t.id = p.transaction_id \
         WHERE t.type = 'sale' AND t.status = 'completed' {clause} \
         GROUP BY p.kind ORDER BY 2 DESC"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .map(|(kind, amount)| PaymentRow { kind, amount })
    .collect();

    // By department (via product -> category)
    let by_department = sqlx::query_as::<_, (String, i64)>(&format!(
        "SELECT COALESCE(c.name,'Uncategorized'), COALESCE(SUM(ti.line_total),0) \
         FROM transaction_items ti \
         JOIN transactions t ON t.id = ti.transaction_id \
         LEFT JOIN products pr ON pr.id = ti.product_id \
         LEFT JOIN categories c ON c.id = pr.category_id \
         WHERE t.type = 'sale' AND t.status = 'completed' {clause} \
         GROUP BY c.name ORDER BY 2 DESC"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .map(|(department, sales)| DeptRow { department, sales })
    .collect();

    // Top products by units sold
    let top_products = sqlx::query_as::<_, (String, i64, i64)>(&format!(
        "SELECT pr.name, COALESCE(SUM(ti.qty),0), COALESCE(SUM(ti.line_total),0) \
         FROM transaction_items ti \
         JOIN transactions t ON t.id = ti.transaction_id \
         JOIN products pr ON pr.id = ti.product_id \
         WHERE t.type = 'sale' AND t.status = 'completed' {clause} \
         GROUP BY pr.id ORDER BY SUM(ti.qty) DESC LIMIT 10"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .map(|(name, qty, revenue)| TopRow { name, qty, revenue })
    .collect();

    let net = gross - tax;
    let avg_basket = if txn_count > 0 { gross / txn_count } else { 0 };

    Ok(ReportData {
        period,
        gross, tax, net, txn_count, avg_basket,
        by_payment, by_department, top_products,
    })
}
