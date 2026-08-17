-- Money is stored as INTEGER cents everywhere. Never floats for currency.

CREATE TABLE categories (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE products (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    sku            TEXT NOT NULL UNIQUE,
    name           TEXT NOT NULL,
    category_id    INTEGER REFERENCES categories(id),
    price          INTEGER NOT NULL,            -- cents
    cost           INTEGER NOT NULL,            -- cents
    tax_rate       REAL    NOT NULL DEFAULT 0.0,
    age_restricted INTEGER NOT NULL DEFAULT 0,  -- 0/1
    active         INTEGER NOT NULL DEFAULT 1,
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_products_sku  ON products(sku);
CREATE INDEX idx_products_name ON products(name);

CREATE TABLE inventory (
    product_id       INTEGER PRIMARY KEY REFERENCES products(id),
    quantity_on_hand INTEGER NOT NULL DEFAULT 0,
    reorder_level    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE users (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    name     TEXT NOT NULL,
    role     TEXT NOT NULL CHECK (role IN ('cashier','manager')),
    pin_hash TEXT NOT NULL,
    active   INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE transactions (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    cashier_id INTEGER REFERENCES users(id),
    status     TEXT NOT NULL DEFAULT 'completed',
    subtotal   INTEGER NOT NULL,
    tax        INTEGER NOT NULL,
    total      INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE transaction_items (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id INTEGER NOT NULL REFERENCES transactions(id),
    product_id     INTEGER NOT NULL REFERENCES products(id),
    qty            INTEGER NOT NULL,
    unit_price     INTEGER NOT NULL,   -- captured at sale time
    line_total     INTEGER NOT NULL
);
CREATE INDEX idx_items_tx ON transaction_items(transaction_id);

CREATE TABLE payments (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id INTEGER NOT NULL REFERENCES transactions(id),
    kind           TEXT NOT NULL CHECK (kind IN ('cash','card')),
    amount         INTEGER NOT NULL,
    tendered       INTEGER NOT NULL,
    change         INTEGER NOT NULL
);

-- Append-only. No UPDATE/DELETE is ever issued against this table.
CREATE TABLE audit_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER REFERENCES users(id),
    action     TEXT NOT NULL,
    entity     TEXT,
    detail     TEXT,                  -- JSON
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
