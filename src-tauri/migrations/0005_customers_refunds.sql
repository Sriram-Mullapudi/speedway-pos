-- Phase 6: loyalty customers, refund/void support, suspended sales.

CREATE TABLE customers (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT NOT NULL,
    phone          TEXT NOT NULL UNIQUE,
    email          TEXT,
    loyalty_points INTEGER NOT NULL DEFAULT 0,  -- 1 point = 1 cent of value
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_customers_phone ON customers(phone);

ALTER TABLE transactions ADD COLUMN customer_id INTEGER;
ALTER TABLE transactions ADD COLUMN original_txn_id INTEGER;
ALTER TABLE transactions ADD COLUMN discount INTEGER NOT NULL DEFAULT 0;
ALTER TABLE transactions ADD COLUMN points_delta INTEGER NOT NULL DEFAULT 0;

CREATE TABLE suspended_sales (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    cashier_id INTEGER,
    cart_json  TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
