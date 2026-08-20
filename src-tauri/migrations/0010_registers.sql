-- Phase 16: multi-register foundation (single store, multiple lanes).
-- Forward-only, additive. No sync machinery — just stable register identity.

CREATE TABLE registers (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    global_id  TEXT NOT NULL UNIQUE,        -- stable UUID; the future sync/branch
                                            -- layer will identify terminals by this,
                                            -- not by the local integer id.
    name       TEXT NOT NULL,               -- human label, e.g. "Register 1", "Front"
    active     INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Preserve every existing register_id = 1 row: create the default register with
-- local id 1 and a fixed, recognizable global_id. Backfill is non-destructive.
INSERT INTO registers (id, global_id, name)
VALUES (1, 'reg-00000000-0000-0000-0000-000000000001', 'Register 1');

-- Stamp the owning register's global_id onto shifts and transactions so history
-- is attributable to a stable identity even after multi-branch arrives. Existing
-- rows all belong to the default register.
ALTER TABLE shifts ADD COLUMN register_global_id TEXT NOT NULL
    DEFAULT 'reg-00000000-0000-0000-0000-000000000001';
ALTER TABLE transactions ADD COLUMN register_global_id TEXT NOT NULL
    DEFAULT 'reg-00000000-0000-0000-0000-000000000001';

CREATE INDEX idx_txn_register ON transactions(register_id);
CREATE INDEX idx_shift_register ON shifts(register_id);
