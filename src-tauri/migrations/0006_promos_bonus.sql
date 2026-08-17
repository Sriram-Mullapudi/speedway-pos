-- Phase 7: per-product promotions and bonus loyalty points.
ALTER TABLE products ADD COLUMN bonus_points INTEGER NOT NULL DEFAULT 0;
ALTER TABLE products ADD COLUMN promo_type   TEXT    NOT NULL DEFAULT 'none';  -- none|bogo|second_pct
ALTER TABLE products ADD COLUMN promo_value  INTEGER NOT NULL DEFAULT 0;       -- pct for second_pct
