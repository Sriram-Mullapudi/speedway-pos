# Speedway POS — Architecture & Project Plan

A modern, offline-first point-of-sale system for convenience-store / gas-station
retail, built on **Tauri 2 · Rust · SQLite · React + TypeScript**.

This document is the full target architecture. It is deliberately larger than any
single milestone — the point of an architecture is to design the whole so the parts
fit, then build incrementally. The **roadmap (§12)** sequences it MVP-first, and
**§13** maps it back to the working skeleton you already have running.

A note on scope: the system below is what a real multi-register store needs. For a
**portfolio piece**, you do not build all of it — §12 marks a "portfolio sweet spot"
subset that demonstrates the hard, interesting engineering without becoming a
multi-year product. Design the whole; ship the slice.

---

## 1. Tech stack

| Layer | Choice | Why |
|---|---|---|
| Shell | **Tauri 2** | Native window, tiny binary, Rust backend. Real POS-grade. |
| Frontend | **React 18 + TypeScript** | Largest transferable skill set; fast iteration. |
| State | **Zustand** | Minimal, no boilerplate; good for cart/session state. |
| Styling | Hand-rolled CSS tokens (current) → optional Tailwind later | Register UIs are bespoke; tokens keep it disciplined. |
| Backend | **Rust** (Tauri commands + domain modules) | All money/tax/tender logic lives here, pure and testable. |
| Data | **SQLite** via `sqlx` | Embedded, transactional, offline-first by nature. |
| Money | **Integer cents everywhere** | Never floats for currency. |
| Sync (future) | Append-only change log + tiny HTTP server | Registers reconcile when online; never block a sale. |

Guiding principles:

- **Offline-first, server-optional.** A register must keep selling with the network
  down. The cloud is for reporting and multi-store roll-up, never for the sale path.
- **Rust is the source of truth.** The frontend sends *intent* (product ids, qty,
  tender). Rust recomputes every price from the DB and decides the outcome. A tampered
  UI cannot change a total.
- **One sale = one DB transaction.** Items, payments, inventory movements, and the
  audit row commit together or not at all.
- **Everything sensitive is audited.** Voids, refunds, no-sales, price overrides, drawer
  opens, and logins all write append-only `audit_logs` rows.

---

## 2. System architecture

```
┌──────────────────────────── Tauri Window ────────────────────────────┐
│  React + TypeScript (UI)                                              │
│   Sales · Payment · Inventory · Reports · Settings · Touchscreen      │
│        │  typed api.ts wrappers (invoke)                              │
└────────┼─────────────────────────────────────────────────────────────┘
         │  Tauri IPC (commands / events)
┌────────▼─────────────────────────────────────────────────────────────┐
│  Rust backend                                                         │
│   commands/   → thin handlers, validate + map errors                  │
│   domain/     → pricing, tax, tender, shift math (pure, unit-tested)  │
│   services/   → catalog, sales, inventory, reports, shifts, sync      │
│   db/         → sqlx pool, migrations, repositories                   │
└────────┬─────────────────────────────────────────────────────────────┘
         │
┌────────▼───────────┐        ┌──────────────────────────────────────┐
│  SQLite (pos.db)   │        │  Sync engine (Phase 6, optional)     │
│  local, embedded   │◄──────►│  change-log push/pull → cloud API    │
└────────────────────┘        └──────────────────────────────────────┘
```

Hardware integration (printer, drawer, scanner, card terminal) lives behind Rust
service traits so it can be stubbed in dev and swapped for real drivers later.
Barcode scanners are just keyboards — the search box already handles them.

---

## 3. Database schema

SQLite DDL, integer cents, evolving the skeleton's existing tables. `categories`
becomes `departments`; `users` becomes `cashiers`; tax is normalized into its own
table; inventory gains a movement ledger.

```sql
-- Reference data ---------------------------------------------------------
CREATE TABLE taxes (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    rate       REAL NOT NULL,            -- e.g. 0.07
    is_default INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE departments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL UNIQUE,
    color           TEXT,                -- touchscreen button color
    tax_id          INTEGER REFERENCES taxes(id),
    age_restricted  INTEGER NOT NULL DEFAULT 0,
    ebt_eligible    INTEGER NOT NULL DEFAULT 0,
    sort_order      INTEGER NOT NULL DEFAULT 0
);

-- Catalog ----------------------------------------------------------------
CREATE TABLE products (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    sku            TEXT NOT NULL UNIQUE,
    barcode        TEXT,                 -- UPC/EAN; may differ from sku
    name           TEXT NOT NULL,
    department_id  INTEGER REFERENCES departments(id),
    price          INTEGER NOT NULL,     -- cents
    cost           INTEGER NOT NULL,     -- cents
    tax_id         INTEGER REFERENCES taxes(id),
    age_restricted INTEGER NOT NULL DEFAULT 0,
    ebt_eligible   INTEGER NOT NULL DEFAULT 0,
    track_inventory INTEGER NOT NULL DEFAULT 1,
    image_path     TEXT,                 -- for image product buttons
    active         INTEGER NOT NULL DEFAULT 1,
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_products_sku     ON products(sku);
CREATE INDEX idx_products_barcode ON products(barcode);

-- CRV / bottle-deposit style auto-added fees
CREATE TABLE product_links (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id      INTEGER NOT NULL REFERENCES products(id),
    linked_product_id INTEGER NOT NULL REFERENCES products(id),
    link_type       TEXT NOT NULL,       -- 'crv' | 'deposit'
    qty             INTEGER NOT NULL DEFAULT 1
);

-- Inventory: cached on-hand + an append-only movement ledger
CREATE TABLE inventory (
    product_id       INTEGER PRIMARY KEY REFERENCES products(id),
    quantity_on_hand INTEGER NOT NULL DEFAULT 0,
    reorder_level    INTEGER NOT NULL DEFAULT 0,
    reorder_qty      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE inventory_movements (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id),
    delta      INTEGER NOT NULL,          -- +receive, -sale, ±adjust
    reason     TEXT NOT NULL,             -- 'sale'|'receive'|'adjust'|'void'|'count'
    ref_type   TEXT,                      -- 'transaction' etc.
    ref_id     INTEGER,
    user_id    INTEGER REFERENCES cashiers(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- People & devices -------------------------------------------------------
CREATE TABLE cashiers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    role        TEXT NOT NULL CHECK (role IN ('cashier','manager','admin')),
    pin_hash    TEXT NOT NULL,            -- argon2
    permissions TEXT,                     -- JSON overrides
    active      INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE registers (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    name   TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE shifts (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    register_id   INTEGER NOT NULL REFERENCES registers(id),
    cashier_id    INTEGER NOT NULL REFERENCES cashiers(id),
    opened_at     TEXT NOT NULL DEFAULT (datetime('now')),
    closed_at     TEXT,
    opening_float INTEGER NOT NULL DEFAULT 0,   -- cents
    closing_count INTEGER,                       -- counted cash
    expected_cash INTEGER,                       -- float + cash sales - payouts
    status        TEXT NOT NULL DEFAULT 'open'   -- 'open'|'closed'
);

-- Customers / loyalty ----------------------------------------------------
CREATE TABLE customers (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT,
    phone          TEXT UNIQUE,
    email          TEXT,
    loyalty_points INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sales ------------------------------------------------------------------
CREATE TABLE transactions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    register_id     INTEGER REFERENCES registers(id),
    shift_id        INTEGER REFERENCES shifts(id),
    cashier_id      INTEGER REFERENCES cashiers(id),
    customer_id     INTEGER REFERENCES customers(id),
    type            TEXT NOT NULL DEFAULT 'sale',   -- 'sale'|'refund'
    status          TEXT NOT NULL DEFAULT 'completed',
                    -- 'completed'|'suspended'|'voided'|'refunded'
    original_txn_id INTEGER REFERENCES transactions(id),  -- for refunds
    subtotal        INTEGER NOT NULL,
    tax             INTEGER NOT NULL,
    total           INTEGER NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE transaction_items (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id INTEGER NOT NULL REFERENCES transactions(id),
    product_id     INTEGER NOT NULL REFERENCES products(id),
    department_id  INTEGER REFERENCES departments(id),  -- denormalized for reports
    qty            INTEGER NOT NULL,
    unit_price     INTEGER NOT NULL,
    tax_amount     INTEGER NOT NULL DEFAULT 0,
    line_total     INTEGER NOT NULL,
    voided         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_items_tx ON transaction_items(transaction_id);

CREATE TABLE payments (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id INTEGER NOT NULL REFERENCES transactions(id),
    kind           TEXT NOT NULL,   -- 'cash'|'card'|'ebt_food'|'ebt_cash'|'gift'|'check'
    amount         INTEGER NOT NULL,
    tendered       INTEGER NOT NULL,
    change         INTEGER NOT NULL DEFAULT 0,
    ref            TEXT,            -- auth/approval code
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Configuration ----------------------------------------------------------
CREATE TABLE touchscreen_buttons (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    page          INTEGER NOT NULL DEFAULT 0,
    row           INTEGER NOT NULL,
    col           INTEGER NOT NULL,
    kind          TEXT NOT NULL,   -- 'product'|'department'|'function'|'menu'|'unused'
    product_id    INTEGER REFERENCES products(id),
    department_id INTEGER REFERENCES departments(id),
    function_code TEXT,            -- 'void'|'refund'|'no_sale'|'safe_drop'|...
    label         TEXT,
    color         TEXT,
    icon          TEXT
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL          -- JSON
);

-- Generated Z/X reports, cached so they're immutable once cut
CREATE TABLE reports (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    type         TEXT NOT NULL,   -- 'z'|'x'|'shift'|'tax'|'department'
    period_start TEXT NOT NULL,
    period_end   TEXT NOT NULL,
    register_id  INTEGER REFERENCES registers(id),
    shift_id     INTEGER REFERENCES shifts(id),
    payload      TEXT NOT NULL,   -- JSON snapshot
    generated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Append-only. No UPDATE/DELETE ever issued.
CREATE TABLE audit_logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER REFERENCES cashiers(id),
    action     TEXT NOT NULL,    -- 'sale.void','price.override','drawer.open',...
    entity     TEXT,
    entity_id  INTEGER,
    detail     TEXT,             -- JSON
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**Sync-ready note:** every mutable table carries `updated_at` (add to the rest as you
go) and nothing is hard-deleted on the sale path. A future `change_log` table records
row-level changes for push/pull. You don't build sync now — you just never paint
yourself into a corner that prevents it.

---

## 4. Tauri command structure

Commands are thin; they validate input, call a service, and map errors to strings.
Grouped by module:

```
auth::         login · logout · current_user · list_cashiers · upsert_cashier
catalog::      search_products · get_product · upsert_product · delete_product
               · import_products · list_departments · upsert_department · list_taxes
sales::        create_sale · suspend_sale · resume_sale · list_suspended
               · void_transaction · refund_transaction · list_recent_sales
inventory::    adjust_stock · receive_stock · list_low_stock · movements_for
shifts::       open_shift · close_shift · current_shift · cash_drop · open_drawer
reports::      sales_summary · top_products · department_breakdown · tax_totals
               · payment_totals · void_refund_report · z_report
touchscreen::  get_layout · save_layout
settings::     get_settings · update_settings
sync::         (Phase 6) push_changes · pull_changes · sync_status
```

---

## 5. Example TypeScript types

```typescript
// money is always integer cents
export type Cents = number;

export interface Department {
  id: number; name: string; color: string | null;
  tax_id: number | null; age_restricted: boolean; ebt_eligible: boolean;
}

export interface Product {
  id: number; sku: string; barcode: string | null; name: string;
  department_id: number | null; price: Cents; cost: Cents;
  tax_rate: number; age_restricted: boolean; ebt_eligible: boolean;
  image_path: string | null; active: boolean;
}

export interface CartLine { product: Product; qty: number; }

export type TenderKind = "cash" | "card" | "ebt_food" | "ebt_cash" | "gift";
export interface Tender { kind: TenderKind; tendered: Cents; }

export interface CreateSaleInput {
  cashier_id: number;
  shift_id: number;
  items: { product_id: number; qty: number }[];
  payments: Tender[];            // array → supports split tender
  age_verified: boolean;
}

export interface Receipt {
  id: number; subtotal: Cents; tax: Cents; total: Cents;
  payments: { kind: TenderKind; amount: Cents; change: Cents }[];
  items: { name: string; qty: number; unit_price: Cents; line_total: Cents }[];
}

export interface Shift {
  id: number; register_id: number; cashier_id: number;
  opened_at: string; closed_at: string | null;
  opening_float: Cents; status: "open" | "closed";
}

export interface ZReport {
  shift_id: number; period_start: string; period_end: string;
  gross_sales: Cents; net_sales: Cents; tax_collected: Cents;
  by_payment: Record<TenderKind, Cents>;
  by_department: { department: string; sales: Cents }[];
  transaction_count: number; void_count: number; refund_total: Cents;
  expected_cash: Cents; counted_cash: Cents | null; over_short: Cents | null;
}

export type ButtonKind = "product" | "department" | "function" | "menu" | "unused";
export interface TouchButton {
  id: number; page: number; row: number; col: number; kind: ButtonKind;
  product_id: number | null; department_id: number | null;
  function_code: string | null; label: string; color: string; icon: string | null;
}
```

---

## 6. Example Rust commands

Two of the meatier new ones — these are the kind an interviewer will ask you to walk
through. (`create_sale` already exists in the skeleton; extend it to accept a
`payments` array for split tender.)

```rust
/// Void a completed transaction. Reverses inventory, marks the row, and writes
/// an immutable audit entry. Manager-only — enforced in the service, not the UI.
#[tauri::command]
pub async fn void_transaction(
    state: tauri::State<'_, AppState>,
    txn_id: i64,
    user_id: i64,
    reason: String,
) -> Result<(), String> {
    let mut tx = state.pool.begin().await.map_err(|e| e.to_string())?;

    // Restore stock for every line.
    let items = sqlx::query_as::<_, (i64, i64)>(
        "SELECT product_id, qty FROM transaction_items WHERE transaction_id = ?1",
    )
    .bind(txn_id).fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;

    for (product_id, qty) in items {
        sqlx::query(
            "UPDATE inventory SET quantity_on_hand = quantity_on_hand + ?1 \
             WHERE product_id = ?2",
        ).bind(qty).bind(product_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO inventory_movements (product_id, delta, reason, ref_type, ref_id, user_id) \
             VALUES (?1, ?2, 'void', 'transaction', ?3, ?4)",
        ).bind(product_id).bind(qty).bind(txn_id).bind(user_id)
         .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    sqlx::query("UPDATE transactions SET status = 'voided' WHERE id = ?1")
        .bind(txn_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;

    let detail = serde_json::json!({ "reason": reason }).to_string();
    sqlx::query(
        "INSERT INTO audit_logs (user_id, action, entity, entity_id, detail) \
         VALUES (?1, 'sale.void', 'transaction', ?2, ?3)",
    ).bind(user_id).bind(txn_id).bind(detail)
     .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())
}

/// Close a shift and cut the Z-report: aggregate the shift's sales, compute
/// expected vs counted cash, snapshot it immutably into `reports`.
#[tauri::command]
pub async fn close_shift(
    state: tauri::State<'_, AppState>,
    shift_id: i64,
    counted_cash: i64,
) -> Result<crate::domain::ZReport, String> {
    let report = crate::services::reports::build_z_report(&state.pool, shift_id)
        .await.map_err(|e| e.to_string())?;

    let over_short = counted_cash - report.expected_cash;
    sqlx::query(
        "UPDATE shifts SET closed_at = datetime('now'), status = 'closed', \
         closing_count = ?1 WHERE id = ?2",
    ).bind(counted_cash).bind(shift_id)
     .execute(&state.pool).await.map_err(|e| e.to_string())?;

    let payload = serde_json::to_string(&report).map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO reports (type, period_start, period_end, shift_id, payload) \
         VALUES ('z', ?1, ?2, ?3, ?4)",
    ).bind(&report.period_start).bind(&report.period_end).bind(shift_id).bind(payload)
     .execute(&state.pool).await.map_err(|e| e.to_string())?;

    Ok(ZReport { over_short: Some(over_short), counted_cash: Some(counted_cash), ..report })
}
```

The aggregation itself (`build_z_report`) is pure-ish service code over SQL `SUM`/
`GROUP BY` — the right place to unit-test the over/short math.

---

## 7. Core POS screens & navigation

**Register (sales) screen** — your current screen, extended:
- Product grid + scan/search box (done). Add the **configurable touchscreen grid** as
  an alternate mode driven by `touchscreen_buttons`.
- Cart panel with line edit, **void line**, **suspend sale** (parks the cart, clears
  the register for the next customer), **resume**.
- **Number pad** for quantity / price-embedded barcodes / manual entry; with
  hide-numpad and reverse-order options from settings.
- **Payment screen**: split tender (cash + card + EBT in one sale), EBT food vs cash
  buckets, change due, cashback config. Card and EBT are **mocked placeholders** (real
  processing = PCI/EBT certification, deliberately out of scope).
- **Age verification** gate (done) + optional forced ID scan from settings.
- **Refund** (against an original transaction) and **no-sale / drawer open** functions.

**Navigation / menu structure:**
```
Dashboard
Register (POS)
Catalog ─ Products · Departments · Taxes
Inventory ─ On-hand · Receive · Low stock · Movements
Customers / Loyalty
Transactions ─ History · Suspended · Refunds/Voids
Reports ─ Z-Report · Shift · Sales summary · Top products · Department
          · Tax · Payment methods · Not-found SKUs · Lottery/Tobacco/Alcohol
Settings ─ Cashiers/Users · Registers · Touchscreen Config · Receipt Config
           · Sales Config · Printer · Integrations · Backup/Sync · Audit Logs
```
Implement as a left rail with sections; the Register stays a dedicated full-screen
mode (cashiers shouldn't see admin chrome mid-sale).

**Touchscreen configuration (the standout build):** a grid editor with configurable
rows × columns, drag-and-drop placement of product / department / function / menu /
unused buttons, per-button color and icon, multiple pages. Persists to
`touchscreen_buttons`; the Register reads it live. This is a great portfolio centerpiece
because it's a real builder UI with persistence, not CRUD.

---

## 8. Reports

All read from the transactions you're already generating:

- **Z-Report / X-Report** (shift and day/week/month): gross, net, tax, by-payment,
  by-department, counts, over/short. Z closes the shift; X is a mid-shift peek.
- **Sales summary** · **Top products** · **Department breakdown** (the pie in your
  screenshots) · **Cashier sales** · **Register sales**.
- **Tax totals** · **Payment-method totals** · **Void/refund report**.
- **Inventory low-stock** (on-hand ≤ reorder level).
- **Category compliance**: lottery / tobacco / alcohol totals (filter by department
  flags) — maps to the "scan data" reporting these stores file.
- **Not-found SKUs**: log every failed lookup at the register; this report drives the
  smart-create workflow (§9).

Render with a chart lib (Recharts/Chart.js). Reports are read-only aggregates, so they
parallelize nicely with the rest of the build.

---

## 9. Admin settings

Stored in `settings` (JSON) and enforced in Rust:
- Cashier logins, multiple cashiers, role-based permissions (cashier/manager/admin).
- Open cash drawer on login · drawer threshold warnings · block transaction while
  drawer open · no-sale requires reason.
- Force ID scan for age validation.
- Product-deletion permission · price-edit permission (manager-gated overrides).
- Offline card-processing placeholder · cashback config.
- Receipt config (header/footer, logo, what prints) · printer settings.
- Integrations (fuel controller, loyalty, accounting) — surfaced as a list even if
  stubbed.

The rule: **the UI hides what you can't do; the backend enforces it.** Never trust the
client for authorization.

---

## 10. Innovative features (with a realism flag)

Ordered roughly easy→hard. ⭐ = high portfolio payoff for the effort.

- ⭐ **Smart not-found-SKU creation** — failed scan opens a quick-add modal pre-filled
  with the scanned barcode; product is live in one step. Cheap, very impressive.
- ⭐ **Predictive low-stock + reorder suggestions** — from sales velocity in
  `inventory_movements` (units/day → days-of-cover). Pure SQL + a little math.
- ⭐ **Cashier performance dashboard** — sales/hour, items/txn, void rate, over/short
  trend per cashier. Falls out of data you already have.
- ⭐ **Offline-first register mode with a sync status indicator** — the architectural
  centerpiece; you can demo it by pulling the network.
- **AI product-import cleanup** — paste a messy distributor CSV; an LLM normalizes
  names, guesses departments/tax, flags age-restricted. Realistic via an API call;
  keep it an explicit, reviewable step, not silent.
- **Image-based product buttons** — `image_path` on products; nice for the touchscreen.
- **Automatic Z-report generation** — scheduled cut at close-of-day.
- **Real-time dashboard** — Tauri events push live totals to the dashboard as sales land.
- **Voice search** — Web Speech API in the search box. Flashy, low-priority.
- **Plugin integration system** — define a trait + manifest so integrations (loyalty,
  fuel, accounting) load uniformly. Ambitious; design the seam, build one example.

Honest take for a portfolio: pick **two or three ⭐ items** and execute them well. The
not-found-SKU flow, predictive reorder, and a working offline-sync indicator together
tell a stronger story than ten half-built gimmicks.

---

## 11. Folder structure

```
pos/
├─ src/                          # React + TypeScript
│  ├─ main.tsx
│  ├─ App.tsx                    # router / shell
│  ├─ api/                       # typed Tauri wrappers, by module
│  │  ├─ catalog.ts  sales.ts  inventory.ts  reports.ts  shifts.ts  settings.ts
│  ├─ store/                     # zustand: cart, session, settings
│  ├─ screens/
│  │  ├─ register/   payment/   inventory/   reports/   settings/
│  │  ├─ touchscreen-config/     dashboard/   transactions/
│  ├─ components/                # shared UI
│  ├─ types.ts
│  └─ styles/
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs                 # builder, state, handler registration
│  │  ├─ commands/               # thin handlers, by module
│  │  ├─ domain/                 # pricing, tax, tender, shift math (pure, tested)
│  │  ├─ services/               # catalog, sales, inventory, reports, shifts, sync
│  │  ├─ db.rs                   # pool + migrations
│  │  └─ models.rs
│  ├─ migrations/                # 0001_init.sql, 0002_departments.sql, ...
│  ├─ capabilities/  icons/
│  ├─ Cargo.toml  tauri.conf.json
├─ package.json  vite.config.ts  tsconfig.json
└─ ARCHITECTURE.md  README.md
```

This is a refactor of the skeleton's flat `commands.rs` into `commands/ + domain/ +
services/` — do it when the file gets uncomfortable, not before.

---

## 12. Development roadmap

Each phase ends with something demoable. **MVP = Phases 0–4.**

| Phase | Build | Status |
|---|---|---|
| **0** | Scaffold: Tauri + React + SQLite + migrations + seed | ✅ done |
| **1** | Register loop: scan/search → cart → tender → receipt; age gate; audit log | ✅ done |
| **2** | Catalog & inventory: product/department/tax CRUD, on-hand + movements ledger, low-stock, margin view | next |
| **3** | Auth & shifts: PIN login, roles, open/close shift, cash drawer, no-sale | |
| **4** | Payments & reports: split tender + EBT, refund/void/suspend, Z-report + top-products + department breakdown | ← **MVP complete** |
| **5** | Touchscreen config builder (grid editor, drag-drop, persistence) | |
| **6** | Offline-sync engine: change log + tiny server + sync-status UI | |
| **7** | Innovation pass: smart not-found SKU, predictive reorder, cashier dashboard, AI import | |
| **8** | Polish: receipt/printer config, real-time dashboard, packaging, demo video | |

**Portfolio sweet spot:** Phases 0–4 plus **one** of {Phase 5 touchscreen builder,
Phase 6 sync engine} plus **two** ⭐ innovation items. That's a finished, genuinely
impressive system without an open-ended timeline.

---

## 13. Where you are now

Already working from the skeleton: the Tauri shell, SQLite with migrations + seeded
catalog, the full Register checkout loop (scan/search → cart → cash/card tender →
receipt), the age-verification gate enforced on both sides, integer-cents money with a
unit-tested pricing module, and per-sale audit logging.

**Immediate next step (Phase 2):** the catalog + inventory back-office — product /
department / tax CRUD, the `inventory_movements` ledger with a derived on-hand, a
low-stock view, and a live margin (cost vs price) column. It's backend-light (a handful
of new commands) and gives you a second screen plus the data that every report and the
predictive-reorder feature later depend on.
