-- Phase 13: vendors, purchasing, receiving, cost history, counts, pack conversion.
-- Forward-only, additive. Reuses the existing inventory_movements ledger for all
-- stock changes. No global-UUID/sync columns yet (deferred), but new domain
-- tables use clean integer PKs that won't block a later multi-branch design.

CREATE TABLE vendors (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    contact    TEXT,
    phone      TEXT,
    email      TEXT,
    account_no TEXT,
    notes      TEXT,
    active     INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Product references a preferred vendor and pack/reorder metadata.
ALTER TABLE products ADD COLUMN preferred_vendor_id INTEGER REFERENCES vendors(id);
ALTER TABLE products ADD COLUMN pack_size INTEGER NOT NULL DEFAULT 1;   -- selling units per case
ALTER TABLE products ADD COLUMN min_stock INTEGER NOT NULL DEFAULT 0;
ALTER TABLE products ADD COLUMN vendor_sku TEXT;

CREATE TABLE purchase_orders (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor_id     INTEGER NOT NULL REFERENCES vendors(id),
    reference     TEXT,
    status        TEXT NOT NULL DEFAULT 'draft', -- draft|ordered|partial|received|closed|cancelled
    notes         TEXT,
    created_by    INTEGER REFERENCES cashiers(id),
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    expected_at   TEXT,
    closed_at     TEXT
);
CREATE INDEX idx_po_vendor ON purchase_orders(vendor_id);
CREATE INDEX idx_po_status ON purchase_orders(status);

CREATE TABLE purchase_order_lines (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    po_id        INTEGER NOT NULL REFERENCES purchase_orders(id),
    product_id   INTEGER NOT NULL REFERENCES products(id),
    vendor_sku   TEXT,
    qty_ordered  INTEGER NOT NULL,            -- in cases (pack units)
    qty_received INTEGER NOT NULL DEFAULT 0,  -- in cases
    unit_cost    INTEGER NOT NULL,            -- cents, cost per case
    pack_size    INTEGER NOT NULL DEFAULT 1   -- snapshot of selling units/case at PO time
);
CREATE INDEX idx_pol_po ON purchase_order_lines(po_id);

-- Append-only product cost history (current cost changes; never rewrites sales).
CREATE TABLE product_cost_history (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id),
    prior_cost INTEGER,
    new_cost   INTEGER NOT NULL,
    source     TEXT NOT NULL,                 -- 'receiving'|'manual'|'import'
    ref_type   TEXT,
    ref_id     INTEGER,
    user_id    INTEGER REFERENCES cashiers(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_cost_hist_product ON product_cost_history(product_id);

-- Physical / cycle count sessions and their lines.
CREATE TABLE count_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT NOT NULL DEFAULT 'physical', -- physical|cycle
    status      TEXT NOT NULL DEFAULT 'open',     -- open|completed|cancelled
    notes       TEXT,
    created_by  INTEGER REFERENCES cashiers(id),
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT
);

CREATE TABLE count_lines (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   INTEGER NOT NULL REFERENCES count_sessions(id),
    product_id   INTEGER NOT NULL REFERENCES products(id),
    expected_qty INTEGER NOT NULL,   -- on-hand snapshot when counted
    counted_qty  INTEGER NOT NULL,
    variance     INTEGER NOT NULL    -- counted - expected
);
CREATE INDEX idx_count_lines_session ON count_lines(session_id);
