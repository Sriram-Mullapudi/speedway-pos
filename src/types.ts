export type PromoType = "none" | "bogo" | "second_pct";

export interface Product {
  id: number;
  sku: string;
  name: string;
  price: number; // cents
  cost: number;  // cents
  tax_rate: number;
  age_restricted: boolean;
  bonus_points: number;
  promo_type: PromoType;
  promo_value: number;
  category_id: number | null;
  open_price: boolean;
}

export interface CartLine {
  uid: number; // stable per-line id; two open-price lines can share a product
  product: Product;
  qty: number;
  priceOverride?: number;
}

export interface ReceiptItem {
  name: string;
  qty: number;
  unit_price: number;
  line_total: number;
}

export interface Receipt {
  id: number;
  store_name: string;
  footer: string;
  cashier: string;
  created_at: string;
  subtotal: number;
  tax: number;
  discount: number;
  total: number;
  tender_kind: string;
  tendered: number;
  change: number;
  points_earned: number;
  points_redeemed: number;
  points_balance: number | null;
  items: ReceiptItem[];
}

export interface Category {
  id: number;
  name: string;
}

export interface CatalogRow {
  id: number;
  sku: string;
  barcode: string | null;
  name: string;
  category_id: number | null;
  department: string | null;
  price: number; // cents
  cost: number;  // cents
  tax_rate: number;
  age_restricted: boolean;
  active: boolean;
  on_hand: number;
  reorder_level: number;
  bonus_points: number;
  promo_type: PromoType;
  promo_value: number;
}

export interface Movement {
  id: number;
  product_id: number;
  delta: number;
  reason: string;
  created_at: string;
}

export interface ProductInput {
  id?: number;
  sku: string;
  barcode: string | null;
  name: string;
  category_id: number | null;
  price: number;
  cost: number;
  tax_rate: number;
  age_restricted: boolean;
  reorder_level: number;
  bonus_points: number;
  promo_type: PromoType;
  promo_value: number;
}

export interface SessionInfo {
  session_id: number;
  cashier_id: number;
  name: string;
  role: "cashier" | "manager" | "admin";
}
export interface Cashier {
  id: number;
  name: string;
  role: "cashier" | "manager" | "admin";
  active: boolean;
  created_at: string;
  updated_at: string;
}
export interface Shift {
  id: number;
  register_id: number;
  cashier_id: number;
  opening_float: number;
  counted_cash: number | null;
  expected_cash: number | null;
  over_short: number | null;
  status: "open" | "closed";
  opened_at: string;
  closed_at: string | null;
}
export interface ShiftSummary {
  shift_id: number;
  cashier_id: number;
  opening_float: number;
  cash_sales: number;
  card_sales: number;
  cash_refunds: number;
  cash_in: number;
  cash_out: number;
  gross_sales: number;
  txn_count: number;
  expected_cash: number;
  counted_cash: number | null;
  over_short: number | null;
  status: string;
  opened_at: string;
  closed_at: string | null;
}
export type DrawerEventType =
  | "no_sale" | "paid_in" | "paid_out" | "safe_drop" | "drawer_open" | "shift_open" | "shift_close";

export interface ReportData {
  period: string;
  gross: number;
  tax: number;
  net: number;
  txn_count: number;
  avg_basket: number;
  by_payment: { kind: string; amount: number }[];
  by_department: { department: string; sales: number }[];
  top_products: { name: string; qty: number; revenue: number }[];
}

export type TouchButtonKind = "product" | "department" | "function" | "unused";
export interface TouchButton {
  kind: TouchButtonKind;
  label: string;
  color: string;
  productId?: number | null;
  departmentId?: number | null;
  functionCode?: string | null;
}
export interface TouchLayout {
  rows: number;
  cols: number;
  showNumpad: boolean;
  cells: (TouchButton | null)[]; // length rows*cols, row-major
}

export interface Customer {
  id: number;
  name: string;
  phone: string;
  email: string | null;
  loyalty_points: number; // 1 point = 1 cent
  created_at: string;
}

export interface TxnRow {
  id: number;
  kind: "sale" | "refund";
  status: "completed" | "voided" | "refunded";
  total: number;
  discount: number;
  cashier: string | null;
  customer: string | null;
  created_at: string;
}

export interface SuspendedSale {
  id: number;
  cashier_id: number | null;
  cart_json: string;
  created_at: string;
}

export type SettingsMap = Record<string, string>;

export interface AuditRow {
  id: number;
  user: string | null;
  action: string;
  entity: string | null;
  entity_id: number | null;
  detail: string | null;
  created_at: string;
}
