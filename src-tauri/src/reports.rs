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

/// Optional register filter. Empty string = all registers (backward-compatible
/// default). Given a register id, restricts to that terminal's rows. The id is
/// validated to be a plain integer before interpolation, so it is injection-safe.
fn register_clause(register_id: Option<i64>) -> String {
    match register_id {
        Some(id) => format!("AND t.register_id = {}", id),
        None => String::new(),
    }
}

#[tauri::command]
pub async fn get_report(
    state: tauri::State<'_, AppState>,
    period: String,
    register_id: Option<i64>,
) -> Result<ReportData, String> {
    let clause = period_clause(&period);
    let reg = register_clause(register_id);
    let pool = &state.pool;

    // Totals
    let (gross, tax, txn_count): (i64, i64, i64) = sqlx::query_as(&format!(
        "SELECT COALESCE(SUM(total),0), COALESCE(SUM(tax),0), COUNT(*) \
         FROM transactions t \
         WHERE t.type = 'sale' AND t.status = 'completed' {clause} {reg}"
    ))
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    // By payment method
    let by_payment = sqlx::query_as::<_, (String, i64)>(&format!(
        "SELECT p.kind, COALESCE(SUM(p.amount),0) \
         FROM payments p JOIN transactions t ON t.id = p.transaction_id \
         WHERE t.type = 'sale' AND t.status = 'completed' {clause} {reg} \
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
         WHERE t.type = 'sale' AND t.status = 'completed' {clause} {reg} \
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
         WHERE t.type = 'sale' AND t.status = 'completed' {clause} {reg} \
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

// ===== Phase 14: profit + loss-prevention reporting =====

#[derive(serde::Serialize)]
pub struct ProfitRow {
    pub department: String,
    pub revenue: i64,
    pub cost: i64,       // only from lines with known historical cost
    pub profit: i64,
    pub margin_pct: f64,
    pub costed_pct: f64, // share of revenue that had a known cost (honesty metric)
}

#[derive(serde::Serialize)]
pub struct ProfitReport {
    pub period: String,
    pub total_revenue: i64,
    pub costed_revenue: i64,   // revenue from lines with known cost
    pub total_cost: i64,
    pub gross_profit: i64,
    pub margin_pct: f64,
    pub by_department: Vec<ProfitRow>,
}

/// Profit reporting. Uses per-line historical `unit_cost` captured at sale time.
/// Lines with NULL cost (sold before Phase 11) are EXCLUDED from cost/profit —
/// we report how much revenue was costed so the number is honest, never guessed.
#[tauri::command]
pub async fn get_profit_report(
    state: tauri::State<'_, AppState>,
    period: String,
) -> Result<ProfitReport, String> {
    let clause = period_clause(&period);
    let pool = &state.pool;

    let rows = sqlx::query_as::<_, (String, i64, Option<i64>, Option<i64>)>(&format!(
        "SELECT COALESCE(c.name,'Uncategorized') AS dept, \
                COALESCE(SUM(ti.line_total),0) AS revenue, \
                SUM(CASE WHEN ti.unit_cost IS NOT NULL THEN ti.line_total ELSE 0 END) AS costed_rev, \
                SUM(CASE WHEN ti.unit_cost IS NOT NULL THEN ti.unit_cost * ti.qty ELSE 0 END) AS cost \
         FROM transaction_items ti \
         JOIN transactions t ON t.id = ti.transaction_id \
         LEFT JOIN products p ON p.id = ti.product_id \
         LEFT JOIN categories c ON c.id = p.category_id \
         WHERE t.type='sale' AND t.status='completed' {clause} \
         GROUP BY c.name ORDER BY revenue DESC"
    ))
    .fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut by_department = Vec::new();
    let (mut total_rev, mut costed_rev_sum, mut total_cost) = (0i64, 0i64, 0i64);
    for (dept, revenue, costed_rev, cost) in rows {
        let costed = costed_rev.unwrap_or(0);
        let cost = cost.unwrap_or(0);
        let profit = costed - cost;
        total_rev += revenue;
        costed_rev_sum += costed;
        total_cost += cost;
        let margin_pct = if costed > 0 { (profit as f64 / costed as f64) * 100.0 } else { 0.0 };
        let costed_pct = if revenue > 0 { (costed as f64 / revenue as f64) * 100.0 } else { 0.0 };
        by_department.push(ProfitRow { department: dept, revenue, cost, profit, margin_pct, costed_pct });
    }
    let gross_profit = costed_rev_sum - total_cost;
    let margin_pct = if costed_rev_sum > 0 { (gross_profit as f64 / costed_rev_sum as f64) * 100.0 } else { 0.0 };

    Ok(ProfitReport {
        period, total_revenue: total_rev, costed_revenue: costed_rev_sum,
        total_cost, gross_profit, margin_pct, by_department,
    })
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct LossPreventionRow {
    pub cashier: String,
    pub void_count: i64,
    pub void_amount: i64,
    pub refund_count: i64,
    pub refund_amount: i64,
    pub no_sale_count: i64,
    pub over_short: i64,
}

/// Loss-prevention summary by cashier: voids, refunds, no-sales, and cumulative
/// over/short. The classic shrink signals, per person, for the period.
#[tauri::command]
pub async fn get_loss_prevention(
    state: tauri::State<'_, AppState>,
    period: String,
) -> Result<Vec<LossPreventionRow>, String> {
    let clause = period_clause(&period);
    let pool = &state.pool;
    sqlx::query_as::<_, LossPreventionRow>(&format!(
        "SELECT ca.name AS cashier, \
           COALESCE(SUM(CASE WHEN t.type='sale' AND t.status='voided' THEN 1 ELSE 0 END),0) AS void_count, \
           COALESCE(SUM(CASE WHEN t.type='sale' AND t.status='voided' THEN t.total ELSE 0 END),0) AS void_amount, \
           COALESCE(SUM(CASE WHEN t.type='refund' THEN 1 ELSE 0 END),0) AS refund_count, \
           COALESCE(SUM(CASE WHEN t.type='refund' THEN t.total ELSE 0 END),0) AS refund_amount, \
           (SELECT COUNT(*) FROM cash_drawer_events e JOIN cashiers c2 ON c2.id=e.cashier_id \
              WHERE e.event_type='no_sale' AND c2.id=ca.id) AS no_sale_count, \
           COALESCE((SELECT SUM(over_short) FROM shifts sh WHERE sh.cashier_id=ca.id AND sh.status='closed'),0) AS over_short \
         FROM cashiers ca \
         LEFT JOIN transactions t ON t.cashier_id = ca.id AND 1=1 {clause} \
         GROUP BY ca.id ORDER BY void_amount DESC"
    ))
    .fetch_all(pool).await.map_err(|e| e.to_string())
}

