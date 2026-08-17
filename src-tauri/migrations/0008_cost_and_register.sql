-- Phase 11: historical-cost + per-line tax + register identity groundwork.
-- Forward-only and additive. Safe to run on existing databases.

-- Historical cost captured at sale time. NULLABLE by design: rows created
-- before this migration have no reliable historical cost, and we do NOT
-- invent financial history. Profit reporting must treat NULL as "cost
-- unknown" and exclude those lines from margin math rather than guess.
ALTER TABLE transaction_items ADD COLUMN unit_cost INTEGER;

-- Per-line tax, computed by the backend with the same rounding policy as the
-- transaction-level tax. Defaults to 0 for historical rows.
ALTER TABLE transaction_items ADD COLUMN tax_amount INTEGER NOT NULL DEFAULT 0;

-- Register identity on each sale. Defaults to 1 (matches shifts.register_id,
-- which is currently a fixed single-register value). This is groundwork only:
-- a full multi-register model (registers table, device identity, per-register
-- config) is deferred to a later phase. Stamping sales now means that model
-- will have real data to group by when it arrives.
ALTER TABLE transactions ADD COLUMN register_id INTEGER NOT NULL DEFAULT 1;
