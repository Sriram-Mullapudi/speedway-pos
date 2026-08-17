# Portfolio notes — the engineering story

This file explains the *why* behind the design, in the order interviewers tend to ask.

## Why Tauri (not Electron)?

Real registers are desktop apps. Tauri gives a native window with a ~10 MB footprint, a Rust process for the business logic, and a standard React frontend — so the project demonstrates both mainstream web skills and systems-level work in one codebase. Electron would have been faster to ship but says less.

## Why does Rust own the business logic?

Because the frontend cannot be trusted. Every command re-derives the facts: `create_sale` re-reads prices from the database and ignores whatever the client claims; loyalty redemption checks the balance server-side; permission checks run in Rust even though the UI also hides buttons. The client sends *intent*, the backend decides *outcome*. This is the difference between a demo and a system that would survive a hostile user.

## Why SQLite?

A register must keep selling when the internet is down — offline-first isn't a feature here, it's the defining constraint of the domain. An embedded, transactional database on the device is the correct architecture; a cloud sync layer is an *addition*, never a dependency of the sale path. SQLite's single-writer model also matches a single-seat register perfectly.

## Why integer cents?

`0.1 + 0.2 !== 0.3`. Currency in floats eventually produces off-by-a-cent bugs that destroy trust in a financial system. Every amount in the schema, the Rust domain layer, and the TypeScript types is an integer number of cents; formatting to dollars happens only at render time. The pricing module (`src-tauri/src/pricing.rs`) is pure and unit-tested: line totals, tax rounding, BOGO and second-item-percent promotions, and loyalty earn/redeem.

## Why are card payments mocked?

Real card processing puts the application in PCI-DSS scope: certified hardware, network segmentation, audits. That's an integration project, not a portfolio one — and knowing where that boundary is matters more than pretending to cross it. The mock still models the parts that affect the rest of the system: refunds route to the original payment method, and only cash movements touch the drawer math.

## How audit logging works

A single append-only writer (`audit.rs`) records logins and failed PINs, manager overrides, voids, refunds, drawer events, shift open/close, inventory adjustments, and settings changes — with the acting user, entity, and a JSON detail payload. The application never issues UPDATE or DELETE against this table. Writes are best-effort by design: an audit failure must never block a sale. A manager-only viewer screen filters by action and user.

## How inventory consistency is protected

On-hand quantity is never edited directly by business flows. Sales, voids, refunds, receives, and adjustments each write a row to the `inventory_movements` ledger *and* update the cached on-hand inside the same database transaction as the parent operation. That means the current number is always explainable as the sum of its history — and features like sales-velocity reorder suggestions fall out of the ledger for free.

## What "real POS thinking" shows up here

Shift over/short math (float + cash sales − cash refunds + paid-in − paid-out vs counted); manager-PIN overrides so a cashier can get one-off approval without logging out; age-restriction enforced in the backend, not just prompted in the UI; soft-delete on products because transaction history must keep resolving; refunds that respect the tender type; loyalty reversal on void/refund via a stored `points_delta`; and a demo reset that seeds a believable week of history so the reporting screens tell a story the moment someone opens the app.
