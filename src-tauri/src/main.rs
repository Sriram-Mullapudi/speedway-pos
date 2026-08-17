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
            let pool = tauri::async_runtime::block_on(async move {
                let pool = db::init_pool(&handle).await.expect("failed to open database");
                db::run_migrations(&pool).await.expect("failed to run migrations");
                seed::seed_if_empty(&pool).await.expect("failed to seed demo data");
                seed::seed_cashiers(&pool).await.expect("failed to seed cashiers");
                seed::seed_convenience_catalog(&pool).await.expect("failed to seed catalog");
                pool
            });
            app.manage(AppState { pool, session: Mutex::new(None) });
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
            purchasing::reorder_suggestions
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
