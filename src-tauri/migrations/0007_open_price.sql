-- Phase 9: open-price (manual price entry) items.
ALTER TABLE products ADD COLUMN open_price INTEGER NOT NULL DEFAULT 0;
