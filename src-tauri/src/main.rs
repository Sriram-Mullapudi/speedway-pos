#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audit;
mod auth;
mod catalog;
mod commands;
mod customers;
mod db;
mod demo;
mod devices;
mod hardware;
mod purchasing;
mod backup;
mod health;
mod applog;
mod registers;
mod inventory;
mod models;
mod pricing;
mod reports;
mod security;
mod seed;
mod settings;
mod shifts;
mod txns;

use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub session: Mutex<Option<crate::models::SessionInfo>>,
    pub app_data: std::path::PathBuf,
}

impl AppState {
    /// Snapshot of the signed-in cashier, if any. The single source of truth
    /// for session reads — command modules delegate here.
    pub fn current_session(&self) -> Option<crate::models::SessionInfo> {
        self.session.lock().unwrap().clone()
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            let app_data = handle.path().app_data_dir().expect("no app data dir");
            let _ = std::fs::create_dir_all(&app_data);

            // CRITICAL: apply any staged restore BEFORE the pool opens, so we
            // swap the database file while nothing holds a connection to it.
            if let Some(status) = backup::apply_pending_restore(&app_data) {
                applog::log(&app_data, "INFO", &format!("startup restore: {status}"));
            }

            let ad = app_data.clone();
            let pool = tauri::async_runtime::block_on(async move {
                let pool = db::init_pool(&handle).await.expect("failed to open database");
                db::run_migrations(&pool).await.expect("failed to run migrations");
                // Stamp schema version into PRAGMA user_version so backups and
                // health checks can read it cheaply. Not a schema change.
                let _ = sqlx::query(&format!("PRAGMA user_version = {}", backup::CURRENT_SCHEMA_VERSION))
                    .execute(&pool).await;
                seed::seed_if_empty(&pool).await.expect("failed to seed demo data");
                seed::seed_cashiers(&pool).await.expect("failed to seed cashiers");
                seed::seed_convenience_catalog(&pool).await.expect("failed to seed catalog");
                pool
            });
            applog::log(&ad, "INFO", "application started");
            app.manage(AppState { pool: pool.clone(), session: Mutex::new(None), app_data: ad.clone() });

            // Automatic backup eligibility — checked once at startup, never after
            // a sale, and never blocking. A failure only flags health "Attention".
            let bg_pool = pool.clone();
            let bg_dir = ad.clone();
            tauri::async_runtime::spawn(async move {
                let freq = crate::settings::get_setting_str(&bg_pool, "backup_auto_freq", "disabled").await;
                if freq == "disabled" { return; }
                let days = match freq.as_str() { "daily" => 1, "every3" => 3, "weekly" => 7, _ => 0 };
                if days == 0 { return; }
                // Eligible if newest automatic backup is older than `days`.
                let backups = crate::backup::list_backups(&bg_dir);
                let newest_auto = backups.iter().find(|m| m.kind == "automatic");
                let eligible = match newest_auto {
                    None => true,
                    Some(m) => {
                        let last: u64 = m.created_at.parse().unwrap_or(0);
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs()).unwrap_or(0);
                        now.saturating_sub(last) >= (days as u64) * 86_400
                    }
                };
                if eligible {
                    match crate::backup::create_backup(&bg_pool, &bg_dir, "automatic").await {
                        Ok(meta) => {
                            let _ = crate::settings::set_setting(&bg_pool, "backup_last_error", "").await;
                            let keep_a = crate::settings::get_setting_i64(&bg_pool, "backup_keep_auto", 7).await as usize;
                            let keep_m = crate::settings::get_setting_i64(&bg_pool, "backup_keep_manual", 10).await as usize;
                            let _ = crate::backup::apply_retention(&bg_dir, keep_m, keep_a);
                            crate::applog::log(&bg_dir, "INFO", &format!("automatic backup created: {}", meta.filename));
                        }
                        Err(e) => {
                            let _ = crate::settings::set_setting(&bg_pool, "backup_last_error", &e).await;
                            crate::applog::log(&bg_dir, "ERROR", &format!("automatic backup failed: {e}"));
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search_products,
            commands::create_sale,
            commands::list_recent_sales,
            catalog::list_catalog,
            catalog::list_categories,
            catalog::upsert_product,
            catalog::set_product_active,
            catalog::list_open_items,
            inventory::adjust_stock,
            inventory::list_low_stock,
            inventory::list_movements,
            auth::login_with_pin,
            auth::logout_cashier,
            auth::get_current_session,
            auth::list_cashiers,
            auth::create_cashier,
            auth::update_cashier,
            auth::deactivate_cashier,
            auth::require_permission,
            auth::manager_override,
            shifts::open_shift,
            shifts::get_active_shift,
            shifts::get_shift_summary,
            shifts::close_shift,
            shifts::create_cash_drawer_event,
            reports::get_report,
            reports::get_profit_report,
            reports::get_loss_prevention,
            settings::get_layout,
            settings::save_layout,
            customers::find_customer_by_phone,
            customers::search_customers,
            customers::create_customer,
            customers::list_customers,
            txns::list_transactions,
            txns::void_transaction,
            txns::refund_transaction,
            txns::suspend_sale,
            txns::list_suspended,
            txns::resume_sale,
            settings::get_settings,
            settings::save_settings,
            audit::list_audit_log,
            demo::reset_demo_data,
            hardware::list_devices,
            hardware::reprint_receipt,
            hardware::print_test_receipt,
            hardware::manual_open_drawer,
            hardware::auto_open_drawer,
            purchasing::list_vendors,
            purchasing::upsert_vendor,
            purchasing::set_vendor_active,
            purchasing::list_purchase_orders,
            purchasing::create_purchase_order,
            purchasing::set_po_status,
            purchasing::receive_purchase_order,
            purchasing::adjust_inventory,
            purchasing::reorder_suggestions,
            health::system_health,
            health::create_manual_backup,
            health::list_backups_cmd,
            health::validate_backup_cmd,
            health::restore_backup_cmd,
            health::diagnostic_info,
            health::export_diagnostic_bundle,
            registers::list_registers,
            registers::upsert_register,
            registers::set_register_active,
            registers::get_active_register,
            registers::set_active_register
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
