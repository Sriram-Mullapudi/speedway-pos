# Speedway POS

**A profit-first point-of-sale for small convenience & liquor stores — because most owners can't see where their money is going.**

Tauri 2 · Rust · SQLite · React · TypeScript

---

## The problem

A small store rings up sales all day, yet at month-end the profit never matches the effort. The leaks are invisible: thin or unknown product margins, a product mix that doesn't match what actually sells, cash that quietly goes missing from the drawer, and shrink hidden inside voids and no-sales. Affordable POS systems only record transactions; the systems that surface *profit* are priced for chains.

## The solution

Speedway POS is built profit-first for the owner-operator:

- **Margin on every item** — cost and price live on each product; the catalog shows live, color-coded margin %.
- **Drawer accountability** — every shift opens with a float and closes with a count; the system computes expected cash and shows the **over/short** per cashier, per shift.
- **Shrink visibility** — voids, refunds, no-sales, and paid-outs require manager approval and land in an **append-only audit log**.
- **Profit reporting** — department breakdown, top products, payment mix, tax, and average basket for today / week / month.
- **Loyalty that pays back** — customers register by phone; earn 1 pt/$1 (+ per-product bonus points), redeem 500 pts for $10 off. Promotions (BOGO, 2nd-item-% off) are priced server-side.

## Demo in 60 seconds

1. `npm run tauri dev`
2. Sign in as **Manager — PIN 2222** (also: Admin 1234, Cashier 1111)
3. **Settings → Reset demo data** — seeds a week of sales, shifts, customers, and promos
4. Look at **Reports** (department pie, top products), **Shift** (drawer math), **Audit** (who did what), then ring a sale on the **Register** — try a BOGO item and attach a loyalty customer by their last 4 digits.

## Architecture decisions

- **Money is integer cents. Everywhere.** Floats never touch currency. All money math lives in a pure, unit-tested Rust module.
- **Rust is the source of truth.** The UI sends *intent* (product ids, qty, tender, redeem flag). Prices, promos, tax, loyalty, and permissions are recomputed in the backend — a tampered frontend cannot change a total or mint a reward.
- **One sale = one DB transaction.** Items, payment, inventory decrement, loyalty update, and the audit row commit together or not at all. Voids and refunds are equally atomic and reverse inventory *and* loyalty.
- **Append-only ledgers.** Inventory changes flow through `inventory_movements`; sensitive actions flow through `audit_log`. Neither is ever updated or deleted by the app.
- **Offline-first.** The register owns a local SQLite file; a sale never waits on a network. The schema carries what a future sync layer needs.
- **Payments are deliberately mocked.** Real card/EBT processing means PCI scope this project intentionally avoids; refunds still route to the original payment method (cash affects the drawer, card does not).

## Feature map

Register (search + configurable touch grid) · age-verification gate · cash/card tender with change · suspend/resume · loyalty lookup by name or last-4 · promotions · receipts with copy-to-clipboard · product/department CRUD with margins · inventory receive/adjust + movement ledger + low stock · PIN login (argon2) with cashier/manager/admin roles and manager-PIN overrides · shifts with over/short · transactions with void/refund · reports dashboard · touchscreen layout builder · settings (store, loyalty rules, defaults) · audit log viewer · one-click demo reset.

## Run it

Prereqs: [Rust](https://rustup.rs), Node 18+, and the [Tauri 2 OS prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev     # dev
npm run tauri build   # installer
```

The SQLite database, migrations, and seed data are created automatically on first launch.

## Tests

```bash
cd src-tauri && cargo test
```

Covers: pricing and tax rounding, BOGO / second-item-% promotions, loyalty earn and redemption (threshold, reward cap), PIN hashing (argon2 round-trip, never plaintext), the role/permission matrix, and shift expected-cash / over-short math.

## Screenshots

> _Add screenshots/GIF here: lock screen → register grid sale → loyalty redeem → refund → shift close over/short → reports._

## Future work

Line-item/partial refunds · mix-and-match promo rules · offline multi-register sync (change-log + reconciliation server) · department drill-down on the touch grid · receipt printer integration · predictive reorder from the movements ledger · cashier performance dashboard.
