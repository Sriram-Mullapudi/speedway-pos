-- Phase 3: cashier / session / shift layer.
ALTER TABLE users RENAME TO users_legacy;

CREATE TABLE cashiers (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    role       TEXT NOT NULL CHECK (role IN ('cashier','manager','admin')),
    pin_hash   TEXT NOT NULL,
    active     INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO cashiers (id, name, role, pin_hash, active)
SELECT id, name, role, pin_hash, active FROM users_legacy;
DROP TABLE users_legacy;

CREATE TABLE cashier_sessions (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    cashier_id INTEGER NOT NULL REFERENCES cashiers(id),
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at   TEXT
);

CREATE TABLE shifts (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    register_id   INTEGER NOT NULL DEFAULT 1,
    cashier_id    INTEGER NOT NULL REFERENCES cashiers(id),
    opening_float INTEGER NOT NULL DEFAULT 0,
    counted_cash  INTEGER,
    expected_cash INTEGER,
    over_short    INTEGER,
    status        TEXT NOT NULL DEFAULT 'open',
    opened_at     TEXT NOT NULL DEFAULT (datetime('now')),
    closed_at     TEXT
);
CREATE INDEX idx_shifts_cashier ON shifts(cashier_id, status);

CREATE TABLE cash_drawer_events (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    cashier_id          INTEGER REFERENCES cashiers(id),
    shift_id            INTEGER REFERENCES shifts(id),
    register_id         INTEGER NOT NULL DEFAULT 1,
    event_type          TEXT NOT NULL,
    amount              INTEGER NOT NULL DEFAULT 0,
    reason              TEXT,
    manager_approved_by INTEGER REFERENCES cashiers(id),
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_drawer_shift ON cash_drawer_events(shift_id);

CREATE TABLE permission_overrides (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    cashier_id INTEGER NOT NULL REFERENCES cashiers(id),
    action     TEXT NOT NULL,
    allowed    INTEGER NOT NULL DEFAULT 1
);

ALTER TABLE transactions ADD COLUMN shift_id INTEGER;
ALTER TABLE transactions ADD COLUMN type TEXT NOT NULL DEFAULT 'sale';
ALTER TABLE audit_log ADD COLUMN entity_id INTEGER;
