import { invoke } from "@tauri-apps/api/core";
import type { Product, Receipt } from "./types";

// Thin, typed wrappers over the Rust commands. The frontend never computes
// money — it sends intent (product ids + qty + tender) and renders the result.

export const searchProducts = (query: string) =>
  invoke<Product[]>("search_products", { query });

export interface CreateSaleInput {
  items: { product_id: number; qty: number; manual_price?: number | null }[];
  tender: { kind: "cash" | "card"; tendered: number };
  age_verified: boolean;
  customer_id?: number | null;
  redeem_points?: boolean;
}

export const createSale = (payload: CreateSaleInput) =>
  invoke<Receipt>("create_sale", { payload });

export const money = (cents: number) => `$${(cents / 100).toFixed(2)}`;

import type { CatalogRow, Category, Movement, ProductInput } from "./types";

// ---- Phase 2: catalog ----
export const listCatalog = () => invoke<CatalogRow[]>("list_catalog");
export const listCategories = () => invoke<Category[]>("list_categories");
export const upsertProduct = (input: ProductInput) =>
  invoke<number>("upsert_product", { input });
export const setProductActive = (id: number, active: boolean) =>
  invoke<void>("set_product_active", { id, active });

// ---- Phase 2: inventory ----
export const adjustStock = (
  product_id: number,
  delta: number,
  reason: "receive" | "adjust" | "count",
  user_id: number
) => invoke<void>("adjust_stock", { productId: product_id, delta, reason, userId: user_id });
export const listLowStock = () => invoke<CatalogRow[]>("list_low_stock");
export const listMovements = (product_id: number) =>
  invoke<Movement[]>("list_movements", { productId: product_id });

// percentage margin from cost/price, guarding divide-by-zero
export const marginPct = (price: number, cost: number) =>
  price > 0 ? Math.round(((price - cost) / price) * 100) : 0;

import type {
  Cashier as CashierT, SessionInfo, Shift, ShiftSummary, DrawerEventType,
} from "./types";

export const getCurrentSession = () => invoke<SessionInfo | null>("get_current_session");
export const loginWithPin = (pin: string) => invoke<SessionInfo>("login_with_pin", { pin });
export const logoutCashier = () => invoke<void>("logout_cashier");

export const listCashiers = () => invoke<CashierT[]>("list_cashiers");
export const createCashier = (name: string, role: string, pin: string) =>
  invoke<number>("create_cashier", { name, role, pin });
export const updateCashier = (id: number, name: string, role: string, active: boolean, pin: string | null) =>
  invoke<void>("update_cashier", { id, name, role, active, pin });
export const deactivateCashier = (id: number) => invoke<void>("deactivate_cashier", { id });
export const requirePermission = (action: string) => invoke<boolean>("require_permission", { action });
export const managerOverride = (action: string, managerPin: string) =>
  invoke<number>("manager_override", { action, managerPin });

export const openShift = (startingCash: number) => invoke<Shift>("open_shift", { startingCash });
export const getActiveShift = () => invoke<Shift | null>("get_active_shift");
export const getShiftSummary = (shiftId: number) => invoke<ShiftSummary>("get_shift_summary", { shiftId });
export const closeShift = (shiftId: number, countedCash: number) =>
  invoke<ShiftSummary>("close_shift", { shiftId, countedCash });
export const createCashDrawerEvent = (
  eventType: DrawerEventType, amount: number, reason: string | null, managerApprovedBy: number | null
) => invoke<void>("create_cash_drawer_event", { eventType, amount, reason, managerApprovedBy });

import type { ReportData } from "./types";

export const getReport = (period: "today" | "week" | "month" | "all") =>
  invoke<ReportData>("get_report", { period });

import type { TouchLayout } from "./types";

export const getLayout = async (): Promise<TouchLayout | null> => {
  const raw = await invoke<string | null>("get_layout");
  return raw ? (JSON.parse(raw) as TouchLayout) : null;
};
export const saveLayout = (layout: TouchLayout) =>
  invoke<void>("save_layout", { layout: JSON.stringify(layout) });

import type { Customer, TxnRow, SuspendedSale } from "./types";

// ---- Phase 6: customers / loyalty ----
export const createCustomer = (name: string, phone: string) =>
  invoke<Customer>("create_customer", { name, phone });
export const listCustomers = () => invoke<Customer[]>("list_customers");

// ---- Phase 6: transactions ----
export const listTransactions = () => invoke<TxnRow[]>("list_transactions");
export const voidTransaction = (txnId: number, managerApprovedBy: number | null) =>
  invoke<void>("void_transaction", { txnId, managerApprovedBy });
export const refundTransaction = (txnId: number, managerApprovedBy: number | null) =>
  invoke<number>("refund_transaction", { txnId, managerApprovedBy });

// ---- Phase 6: suspend / resume ----
export const suspendSale = (cartJson: string) => invoke<number>("suspend_sale", { cartJson });
export const listSuspended = () => invoke<SuspendedSale[]>("list_suspended");
export const resumeSale = (id: number) => invoke<string>("resume_sale", { id });

// ---- Phase 7: promos, phone formatting, customer search ----
import type { PromoType } from "./types";

/** Mirrors pricing::promo_line_total in Rust; the server remains source of truth. */
export function promoLineTotal(unitPrice: number, qty: number, promoType: PromoType, promoValue: number): number {
  if (promoType === "bogo") {
    const free = Math.floor(qty / 2);
    return unitPrice * (qty - free);
  }
  if (promoType === "second_pct") {
    const discounted = Math.floor(qty / 2);
    const full = qty - discounted;
    const pct = Math.min(100, Math.max(0, promoValue));
    const discPrice = Math.floor((unitPrice * (100 - pct)) / 100);
    return unitPrice * full + discPrice * discounted;
  }
  return unitPrice * qty;
}

export function promoLabel(promoType: PromoType, promoValue: number): string | null {
  if (promoType === "bogo") return "BOGO";
  if (promoType === "second_pct") return `2nd ${promoValue}% off`;
  return null;
}

/** Keep only digits; drop a leading US country code. */
export const normalizePhone = (raw: string) => {
  const d = raw.replace(/\D/g, "");
  return d.length === 11 && d.startsWith("1") ? d.slice(1) : d;
};

/** Display as +1(xxx)xxx-xxxx once 10 digits are present. */
export const formatPhone = (raw: string) => {
  const d = normalizePhone(raw);
  if (d.length !== 10) return raw;
  return `+1(${d.slice(0, 3)})${d.slice(3, 6)}-${d.slice(6)}`;
};

export const searchCustomers = (query: string) =>
  invoke<Customer[]>("search_customers", { query });

// ---- Phase 8: settings, audit viewer, demo mode ----
import type { SettingsMap, AuditRow } from "./types";

export const getSettings = () => invoke<SettingsMap>("get_settings");
export const saveSettings = (settings: SettingsMap) => invoke<void>("save_settings", { settings });
export const listAuditLog = (actionLike: string | null, userId: number | null) =>
  invoke<AuditRow[]>("list_audit_log", { actionLike, userId });
export const resetDemoData = () => invoke<string>("reset_demo_data");

// ---- Phase 9 ----
export const listOpenItems = () => invoke<Product[]>("list_open_items");
export const nextDollar = (cents: number) => Math.ceil(cents / 100) * 100;

/** Turn backend errors into cashier-friendly messages (originals stay in the console for debugging). */
export function friendlyError(e: unknown): string {
  const raw = String(e);
  console.error("[pos]", raw);
  if (raw.includes("AGE_VERIFICATION_REQUIRED")) return "This sale needs an ID check before payment.";
  if (raw.includes("UNIQUE")) return "That value is already in use.";
  return raw.replace(/^Error:\s*/, "");
}

// ---- Phase 12: devices / hardware ----
export interface DeviceInfo {
  kind: string; label: string; mode: string; status: string; configurable: boolean;
}
export interface DeviceResult { ok: boolean; mode: string; message: string; }

export const listDevices = () => invoke<DeviceInfo[]>("list_devices");
export const reprintReceipt = (txnId: number) => invoke<DeviceResult>("reprint_receipt", { txnId });
export const printTestReceipt = () => invoke<DeviceResult>("print_test_receipt");
export const manualOpenDrawer = (reason: string, managerApprovedBy: number | null) =>
  invoke<DeviceResult>("manual_open_drawer", { reason, managerApprovedBy });
export const autoOpenDrawer = (event: string, tenderKind: string | null) =>
  invoke<DeviceResult>("auto_open_drawer", { event, tenderKind });

// ---- Phase 13: purchasing / inventory pro ----
export interface Vendor {
  id: number; name: string; contact: string | null; phone: string | null;
  email: string | null; account_no: string | null; notes: string | null; active: boolean;
}
export interface PoRow {
  id: number; vendor: string; reference: string | null; status: string;
  created_at: string; line_count: number; total_cost: number;
}
export interface ReorderRow {
  product_id: number; name: string; on_hand: number; reorder_level: number;
  min_stock: number; pack_size: number; suggested_cases: number; vendor: string | null;
}

export const listVendors = () => invoke<Vendor[]>("list_vendors");
export const upsertVendor = (input: Partial<Vendor> & { name: string }) => invoke<number>("upsert_vendor", { input });
export const setVendorActive = (id: number, active: boolean) => invoke<void>("set_vendor_active", { id, active });
export const listPurchaseOrders = () => invoke<PoRow[]>("list_purchase_orders");
export const createPurchaseOrder = (
  vendorId: number, reference: string | null, notes: string | null,
  lines: { product_id: number; vendor_sku: string | null; qty_ordered: number; unit_cost: number; pack_size: number }[]
) => invoke<number>("create_purchase_order", { vendorId, reference, notes, lines });
export const setPoStatus = (poId: number, status: string) => invoke<void>("set_po_status", { poId, status });
export const receivePurchaseOrder = (poId: number, receipts: { line_id: number; cases_received: number }[]) =>
  invoke<string>("receive_purchase_order", { poId, receipts });
export const adjustInventory = (productId: number, delta: number, reasonCode: string) =>
  invoke<void>("adjust_inventory", { productId, delta, reasonCode });
export const reorderSuggestions = () => invoke<ReorderRow[]>("reorder_suggestions");

// ---- Phase 14: profit + loss prevention ----
import type { ProfitReport, LossPreventionRow } from "./types";
export const getProfitReport = (period: "today" | "week" | "month" | "all") =>
  invoke<ProfitReport>("get_profit_report", { period });
export const getLossPrevention = (period: "today" | "week" | "month" | "all") =>
  invoke<LossPreventionRow[]>("get_loss_prevention", { period });

/** Client-side CSV export of any row set. */
export function exportCsv(filename: string, rows: Record<string, unknown>[]) {
  if (rows.length === 0) return;
  const headers = Object.keys(rows[0]);
  const escape = (v: unknown) => {
    const s = String(v ?? "");
    return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const csv = [headers.join(","), ...rows.map((r) => headers.map((h) => escape(r[h])).join(","))].join("\n");
  const blob = new Blob([csv], { type: "text/csv" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url; a.download = filename; a.click();
  URL.revokeObjectURL(url);
}
