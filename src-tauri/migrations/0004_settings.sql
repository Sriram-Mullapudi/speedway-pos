-- Phase 5: generic key/value settings store (holds the touchscreen layout JSON).
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
