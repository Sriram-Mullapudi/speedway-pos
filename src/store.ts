import { create } from "zustand";
import { promoLineTotal } from "./api";
import type { CartLine, Product } from "./types";

let nextUid = 1;

interface CartState {
  lines: CartLine[];
  add: (p: Product) => void;
  addManual: (p: Product, price: number) => void;
  setQty: (uid: number, qty: number) => void;
  remove: (uid: number) => void;
  clear: () => void;
  subtotal: () => number;
  tax: () => number;
  total: () => number;
  hasAgeRestricted: () => boolean;
}

export const useCart = create<CartState>((set, get) => ({
  lines: [],
  add: (p) =>
    set((s) => {
      // Normal items merge into one line; open-price lines never merge.
      const existing = s.lines.find((l) => l.product.id === p.id && l.priceOverride == null);
      if (existing) {
        return {
          lines: s.lines.map((l) =>
            l.uid === existing.uid ? { ...l, qty: l.qty + 1 } : l
          ),
        };
      }
      return { lines: [...s.lines, { uid: nextUid++, product: p, qty: 1 }] };
    }),
  addManual: (p, price) =>
    set((s) => ({ lines: [...s.lines, { uid: nextUid++, product: p, qty: 1, priceOverride: price }] })),
  setQty: (uid, qty) =>
    set((s) => ({
      lines: s.lines
        .map((l) => (l.uid === uid ? { ...l, qty } : l))
        .filter((l) => l.qty > 0),
    })),
  remove: (uid) =>
    set((s) => ({ lines: s.lines.filter((l) => l.uid !== uid) })),
  clear: () => set({ lines: [] }),
  subtotal: () =>
    get().lines.reduce(
      (sum, l) => sum + promoLineTotal(l.priceOverride ?? l.product.price, l.qty, l.product.promo_type, l.product.promo_value),
      0
    ),
  tax: () =>
    get().lines.reduce(
      (sum, l) =>
        sum +
        Math.round(
          promoLineTotal(l.priceOverride ?? l.product.price, l.qty, l.product.promo_type, l.product.promo_value) *
            l.product.tax_rate
        ),
      0
    ),
  total: () => get().subtotal() + get().tax(),
  hasAgeRestricted: () => get().lines.some((l) => l.product.age_restricted),
}));
