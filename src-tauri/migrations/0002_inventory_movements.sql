-- Phase 2: append-only inventory ledger + a barcode field on products.
-- Additive only — existing data and the 0001 schema are untouched.

CREATE TABLE inventory_movements (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id),
    delta      INTEGER NOT NULL,            -- +receive, -sale, ±adjust/count
    reason     TEXT NOT NULL,               -- 'sale'|'receive'|'adjust'|'count'|'void'
    ref_type   TEXT,                        -- e.g. 'transaction'
    ref_id     INTEGER,
    user_id    INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_movements_product ON inventory_movements(product_id);

-- Barcodes may differ from the internal SKU (UPC/EAN). Nullable, additive.
ALTER TABLE products ADD COLUMN barcode TEXT;
CREATE INDEX idx_products_barcode ON products(barcode);
