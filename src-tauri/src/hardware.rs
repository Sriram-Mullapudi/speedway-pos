//! Tauri command surface for the device layer (Phase 12).
//! These commands sit between the frontend and `devices.rs`. They enforce
//! permissions, audit sensitive actions, and — critically — are only ever
//! invoked AFTER a sale has committed, so a hardware failure here can never
//! affect transaction correctness.

use crate::devices::{
    should_auto_open_drawer, CashDrawer, DeviceResult, ReceiptPrinter,
    SimulatedCashDrawer, SimulatedReceiptPrinter,
};
use crate::security::role_can;
use crate::{audit, settings, AppState};
use serde::Serialize;

fn sim_fail(pool_val: &str) -> bool {
    // A settings-driven switch so the simulated-failure path can be exercised
    // from the UI ("Test printer failure"). Defaults off.
    pool_val == "1"
}

#[derive(Serialize)]
pub struct DeviceInfo {
    pub kind: String,
    pub label: String,
    pub mode: String,
    pub status: String,
    pub configurable: bool,
}

/// Reports honest per-device status for Settings → Devices. Simulated devices
/// are labelled "Simulated", never "Connected".
#[tauri::command]
pub async fn list_devices(state: tauri::State<'_, AppState>) -> Result<Vec<DeviceInfo>, String> {
    let printer_mode = settings::get_setting_str(&state.pool, "dev_receipt_mode", "simulated").await;
    let drawer_mode = settings::get_setting_str(&state.pool, "dev_drawer_mode", "simulated").await;
    Ok(vec![
        DeviceInfo { kind: "barcode_scanner".into(), label: "Barcode Scanner".into(),
            mode: "keyboard_wedge".into(), status: "ready".into(), configurable: true },
        DeviceInfo { kind: "receipt_printer".into(), label: "Receipt Printer".into(),
            mode: printer_mode, status: "simulated".into(), configurable: true },
        DeviceInfo { kind: "cash_drawer".into(), label: "Cash Drawer".into(),
            mode: drawer_mode, status: "simulated".into(), configurable: true },
        DeviceInfo { kind: "customer_display".into(), label: "Customer Display".into(),
            mode: "secondary_window".into(), status: "window_available".into(), configurable: true },
        DeviceInfo { kind: "invoice_printer".into(), label: "Invoice / Document Printer".into(),
            mode: "system_print".into(), status: "ready".into(), configurable: false },
        DeviceInfo { kind: "label_printer".into(), label: "Label Printer".into(),
            mode: "system_print".into(), status: "ready".into(), configurable: false },
    ])
}

/// Rebuilds a receipt document string from AUTHORITATIVE stored transaction
/// data (never from frontend-supplied totals) and "prints" it via the
/// configured receipt printer. Used for reprints and the printer self-test.
#[tauri::command]
pub async fn reprint_receipt(
    state: tauri::State<'_, AppState>,
    txn_id: i64,
) -> Result<DeviceResult, String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;

    let doc = render_receipt_from_db(&state.pool, txn_id).await?;
    let fail = sim_fail(&settings::get_setting_str(&state.pool, "dev_printer_forcefail", "0").await);
    let printer = SimulatedReceiptPrinter { fail };
    let result = printer.print_receipt(&doc);

    audit::write(&state.pool, Some(sess.cashier_id), "receipt.reprint", Some("transaction"),
        Some(txn_id), Some(format!("ok={}", result.ok))).await;
    Ok(result)
}

/// Print a test receipt (does not reference a real sale).
#[tauri::command]
pub async fn print_test_receipt(state: tauri::State<'_, AppState>) -> Result<DeviceResult, String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;
    let store = settings::get_setting_str(&state.pool, "store_name", "Speedway Market").await;
    let doc = format!("{}\n--- TEST RECEIPT ---\nItem A         $1.00\nTOTAL          $1.00\nThank you!", store);
    let fail = sim_fail(&settings::get_setting_str(&state.pool, "dev_printer_forcefail", "0").await);
    let result = SimulatedReceiptPrinter { fail }.print_receipt(&doc);
    audit::write(&state.pool, Some(sess.cashier_id), "device.test", Some("receipt_printer"),
        None, Some(format!("ok={}", result.ok))).await;
    Ok(result)
}

/// Manual/test drawer open — permission controlled and audited.
#[tauri::command]
pub async fn manual_open_drawer(
    state: tauri::State<'_, AppState>,
    reason: String,
    manager_approved_by: Option<i64>,
) -> Result<DeviceResult, String> {
    let sess = state.session.lock().unwrap().clone().ok_or("Not signed in")?;
    if !role_can(&sess.role, "open_drawer") && manager_approved_by.is_none() {
        return Err("Opening the drawer requires a manager".into());
    }
    let result = SimulatedCashDrawer { fail: false }.open(&reason);
    audit::write(&state.pool, Some(sess.cashier_id), "drawer.manual_open", Some("drawer"),
        None, Some(reason)).await;
    Ok(result)
}

/// Decide + (simulated) open the drawer after a committed cash event. Called
/// AFTER commit; a failure here is returned to the UI but never rolls anything
/// back. Returns whether the drawer was opened.
#[tauri::command]
pub async fn auto_open_drawer(
    state: tauri::State<'_, AppState>,
    event: String,
    tender_kind: Option<String>,
) -> Result<DeviceResult, String> {
    let allow_card = settings::get_setting_i64(&state.pool, "dev_drawer_card", 0).await == 1;
    if !should_auto_open_drawer(&event, tender_kind.as_deref(), allow_card) {
        return Ok(DeviceResult { ok: true, mode: "n/a".into(), message: "No drawer open required".into() });
    }
    Ok(SimulatedCashDrawer { fail: false }.open(&event))
}

// ---- authoritative receipt rendering --------------------------------------

async fn render_receipt_from_db(pool: &sqlx::SqlitePool, txn_id: i64) -> Result<String, String> {
    let (store, footer) = (
        settings::get_setting_str(pool, "store_name", "Speedway Market").await,
        settings::get_setting_str(pool, "receipt_footer", "Thank you!").await,
    );
    let (created, subtotal, tax, discount, total, kind, status): (String, i64, i64, i64, i64, String, String) =
        sqlx::query_as(
            "SELECT created_at, subtotal, tax, discount, total, type, status FROM transactions WHERE id = ?1",
        )
        .bind(txn_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Transaction not found")?;

    let items: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT p.name, ti.qty, ti.line_total FROM transaction_items ti \
         JOIN products p ON p.id = ti.product_id WHERE ti.transaction_id = ?1",
    )
    .bind(txn_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let w = 34usize;
    let line = |l: &str, r: &str| -> String {
        let pad = w.saturating_sub(l.len() + r.len());
        format!("{}{}{}", l, " ".repeat(pad), r)
    };
    let money = |c: i64| format!("${}.{:02}", c / 100, (c % 100).abs());

    let mut out = String::new();
    out.push_str(&store.to_uppercase());
    out.push('\n');
    if status != "completed" {
        out.push_str(&format!("*** {} ***\n", status.to_uppercase()));
    }
    out.push_str(&format!("Sale #{}  {}\n", txn_id, created));
    out.push_str(&format!("{}\n", "-".repeat(w)));
    for (name, qty, lt) in &items {
        out.push_str(&line(&format!("{}x {}", qty, name), &money(*lt)));
        out.push('\n');
    }
    out.push_str(&format!("{}\n", "-".repeat(w)));
    out.push_str(&line("Subtotal", &money(subtotal)));
    out.push('\n');
    out.push_str(&line("Tax", &money(tax)));
    out.push('\n');
    if discount > 0 {
        out.push_str(&line("Discount", &format!("-{}", money(discount))));
        out.push('\n');
    }
    out.push_str(&line("TOTAL", &money(total)));
    out.push_str(&format!("\n{}\n{} ({})\n{}\n", "-".repeat(w), footer, kind, "-".repeat(w)));
    Ok(out)
}
