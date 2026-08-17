import { useEffect, useMemo, useRef, useState } from "react";
import {
  searchProducts, createSale, getLayout, listCategories, listOpenItems, money, nextDollar,
  autoOpenDrawer,
  searchCustomers, createCustomer, formatPhone, normalizePhone,
  promoLineTotal, promoLabel, friendlyError,
  suspendSale, listSuspended, resumeSale,
  type CreateSaleInput,
} from "../api";
import { useCart } from "../store";
import { useSession } from "../sessionStore";
import { DrawerModal } from "../components/ShiftModals";
import { notify } from "../components/Toast";
import { useScanner } from "../useScanner";
import { publishCustomerView } from "./CustomerDisplay";
import type { Category, Customer, Product, Receipt, SuspendedSale, TouchLayout } from "../types";

type DeptTab = { id: number | "all" | "custom"; label: string };

export default function Register({ onNav }: { onNav?: (view: string) => void }) {
  const activeShift = useSession((s) => s.activeShift);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Product[]>([]);
  const [allProducts, setAllProducts] = useState<Product[]>([]);
  const [openItems, setOpenItems] = useState<Product[]>([]);
  const [cats, setCats] = useState<Category[]>([]);
  const [layout, setLayout] = useState<TouchLayout | null>(null);
  const [dept, setDept] = useState<number | "all" | "custom">("all");
  const [pad, setPad] = useState(""); // numpad digits, cents-style entry
  const [stage, setStage] = useState<"idle" | "age" | "tender" | "receipt">("idle");
  const [presetTender, setPresetTender] = useState<number | null>(null);
  const [receipt, setReceipt] = useState<Receipt | null>(null);
  const [lastReceipt, setLastReceipt] = useState<Receipt | null>(null);
  const [customer, setCustomer] = useState<Customer | null>(null);
  const [custModal, setCustModal] = useState(false);
  const [suspList, setSuspList] = useState<SuspendedSale[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [drawer, setDrawer] = useState<{ type: "no_sale" | "paid_in" | "paid_out" | "safe_drop"; reason: string } | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const cart = useCart();

  useEffect(() => {
    searchProducts("").then((p) => { setResults(p); setAllProducts(p); }).catch(console.error);
    listCategories().then(setCats).catch(console.error);
    listOpenItems().then(setOpenItems).catch(console.error);
    getLayout().then((l) => { if (l && l.cells.some((c) => c)) setLayout(l); }).catch(console.error);
    searchRef.current?.focus();
  }, []);

  useEffect(() => {
    const id = setTimeout(() => { searchProducts(query).then(setResults).catch(console.error); }, 80);
    return () => clearTimeout(id);
  }, [query]);

  const productMap = useMemo(() => {
    const m = new Map<number, Product>();
    allProducts.forEach((p) => m.set(p.id, p));
    return m;
  }, [allProducts]);

  const tabs: DeptTab[] = useMemo(() => {
    const t: DeptTab[] = [{ id: "all", label: "All" }];
    cats.forEach((c) => t.push({ id: c.id, label: c.name }));
    if (layout) t.push({ id: "custom", label: "★ Custom" });
    return t;
  }, [cats, layout]);

  const visible = useMemo(() => {
    const base = query.trim() ? results : allProducts;
    if (dept === "all" || dept === "custom") return base;
    return base.filter((p) => p.category_id === dept);
  }, [results, allProducts, dept, query]);

  // Push a customer-safe snapshot to the customer display (no cost/margin/audit).
  useEffect(() => {
    publishCustomerView({
      phase: cart.lines.length ? "cart" : "idle",
      store: "Speedway Market",
      items: cart.lines.map((l) => ({
        name: l.product.name, qty: l.qty,
        unit: l.priceOverride ?? l.product.price,
        line: (l.priceOverride ?? l.product.price) * l.qty,
      })),
      subtotal: cart.subtotal(), tax: cart.tax(), total: cart.total(),
      customer: customer?.name ?? null, points: customer?.loyalty_points ?? null,
    });
  }, [cart.lines, customer, cart]);

  const padCents = pad ? parseInt(pad, 10) : 0;

  function tapDept(id: number | "all" | "custom") {
    // Classic register behavior: amount on the pad + a department key = open-price line.
    if (padCents > 0 && typeof id === "number") {
      const open = openItems.find((o) => o.category_id === id);
      if (!open) { notify("No open-price item for that department", "err"); return; }
      cart.addManual(open, padCents);
      setPad("");
      return;
    }
    setDept(id);
  }

  function beginCharge(preset: number | null = null) {
    if (cart.lines.length === 0 || !activeShift) return;
    if (preset !== null && preset < cart.total()) { notify("Quick tender is less than the total", "err"); return; }
    setPresetTender(preset);
    setStage(cart.hasAgeRestricted() ? "age" : "tender");
  }

  // Barcode scanner (keyboard-wedge): look the code up against the authoritative
  // catalog by SKU/barcode/name; unknown codes give a clear cashier response.
  async function onScan(code: string) {
    try {
      const matches = await searchProducts(code);
      const exact = matches.find((p) => p.sku === code) ?? matches[0];
      if (exact) { cart.add(exact); notify(`Added ${exact.name}`); }
      else notify(`Unknown barcode: ${code}`, "err");
    } catch (e) { notify(String(e), "err"); }
  }
  useScanner(onScan);

  async function finalize(tender: { kind: "cash" | "card"; tendered: number }, redeem: boolean) {
    const input: CreateSaleInput = {
      items: cart.lines.map((l) => ({
        product_id: l.product.id, qty: l.qty,
        manual_price: l.priceOverride ?? null,
      })),
      tender,
      age_verified: cart.hasAgeRestricted(),
      customer_id: customer?.id ?? null,
      redeem_points: redeem,
    };
    if (busy) return; // hard guard against double taps on Complete sale
    setBusy(true);
    try {
      const r = await createSale(input);
      setReceipt(r);
      setLastReceipt(r);
      // Post-commit hardware action — never affects the committed sale.
      autoOpenDrawer("sale", tender.kind).catch(() => {});
      publishCustomerView({
        phase: "paid", store: "Speedway Market", items: [],
        subtotal: r.subtotal, tax: r.tax, total: r.total,
        tendered: r.tendered, change: r.change, pointsEarned: r.points_earned,
      });
      setStage("receipt");
      cart.clear();
      setCustomer(null);
      setPresetTender(null);
    } catch (e) {
      notify(friendlyError(e), "err");
    } finally {
      setBusy(false);
    }
  }

  async function doSuspend() {
    if (cart.lines.length === 0) return;
    await suspendSale(JSON.stringify(cart.lines));
    cart.clear();
    setCustomer(null);
    notify("Sale suspended");
  }
  async function openResume() { setSuspList(await listSuspended()); }
  async function doResume(id: number) {
    let json: string;
    try { json = await resumeSale(id); } catch (e) { notify(friendlyError(e), "err"); return; }
    const lines = JSON.parse(json) as { product: Product; qty: number; priceOverride?: number }[];
    cart.clear();
    lines.forEach((l) => {
      if (l.priceOverride != null) cart.addManual(l.product, l.priceOverride);
      else for (let i = 0; i < l.qty; i++) cart.add(l.product);
    });
    setSuspList(null);
  }

  // Retail-convention shortcuts: F2 search · F4 pay · F6 suspend · F7 recall · Esc close
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.key === "F2") { e.preventDefault(); searchRef.current?.focus(); }
      else if (e.key === "F4") { e.preventDefault(); beginCharge(null); }
      else if (e.key === "F6") { e.preventDefault(); doSuspend(); }
      else if (e.key === "F7") { e.preventDefault(); openResume(); }
      else if (e.key === "Escape") {
        setStage("idle"); setCustModal(false); setSuspList(null); setDrawer(null); setPresetTender(null);
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  });

  // Function buttons grouped by intent (Sale / Drawer / Operations / Manager).
  // Grouping is purely visual — every command is unchanged.
  type FnBtn = { label: string; danger?: boolean; onClick: () => void };
  const fnGroups: { title: string; btns: FnBtn[] }[] = [
    { title: "Sale", btns: [
      { label: "Void Last", danger: true, onClick: () => { const l = cart.lines[cart.lines.length - 1]; if (l) cart.remove(l.uid); } },
      { label: "Cancel Sale", danger: true, onClick: () => { cart.clear(); setCustomer(null); } },
      { label: "Suspend", onClick: doSuspend },
      { label: "Recall", onClick: openResume },
      { label: "Refund", danger: true, onClick: () => onNav?.("transactions") },
    ]},
    { title: "Drawer", btns: [
      { label: "No Sale", onClick: () => setDrawer({ type: "no_sale", reason: "" }) },
      { label: "Safe Drop", onClick: () => setDrawer({ type: "safe_drop", reason: "Safe drop" }) },
      { label: "Paid In", onClick: () => setDrawer({ type: "paid_in", reason: "" }) },
      { label: "Paid Out", onClick: () => setDrawer({ type: "paid_out", reason: "" }) },
      { label: "Lotto Payout", onClick: () => setDrawer({ type: "paid_out", reason: "Lottery payout" }) },
    ]},
    { title: "Operations", btns: [
      { label: "Last Receipt", onClick: () => { if (lastReceipt) { setReceipt(lastReceipt); setStage("receipt"); } else notify("No receipt yet"); } },
      { label: "Recent Txns", onClick: () => onNav?.("transactions") },
      { label: "X / Z Report", onClick: () => onNav?.("shift") },
    ]},
    { title: "Manager", btns: [
      { label: "Reports", onClick: () => onNav?.("reports") },
    ]},
  ];

  return (
    <div className="register classic">
      <section className="catalog">
        {!activeShift && (
          <div className="shift-banner">No open shift — open one from the header to start selling.</div>
        )}

        <input
          ref={searchRef}
          className="search"
          placeholder="Scan barcode or search by name / SKU…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && results[0]) { cart.add(results[0]); setQuery(""); } }}
        />

        <div className="dept-tabs">
          {tabs.map((t) => (
            <button key={String(t.id)}
              className={`dept-tab ${dept === t.id ? "on" : ""} ${padCents > 0 && typeof t.id === "number" ? "armed" : ""}`}
              onClick={() => tapDept(t.id)}>
              {t.label}
            </button>
          ))}
          {padCents > 0 && <span className="pad-hint">→ tap a department to ring {money(padCents)}</span>}
        </div>

        {dept === "custom" && layout ? (
          <div className="touch-grid" style={{ gridTemplateColumns: `repeat(${layout.cols}, 1fr)` }}>
            {layout.cells.map((cell, i) => {
              if (!cell) return <div key={i} className="touch-cell empty" />;
              const prod = cell.kind === "product" && cell.productId != null ? productMap.get(cell.productId) : undefined;
              return (
                <button key={i} className="touch-cell" style={{ background: cell.color, color: "#04130a" }}
                  onClick={() => {
                    if (cell.kind === "product" && prod) cart.add(prod);
                    else if (cell.kind === "function" && cell.functionCode === "clear") cart.clear();
                    else if (cell.kind === "function" && cell.functionCode === "void_last" && cart.lines.length) cart.remove(cart.lines[cart.lines.length - 1].uid);
                  }}>
                  <span className="tc-label">{cell.label}</span>
                  {prod && <span className="tc-price">{money(prod.price)}</span>}
                </button>
              );
            })}
          </div>
        ) : (
          <div className="grid classic-grid">
            {visible.map((p) => (
              <button key={p.id} className="card" onClick={() => cart.add(p)}>
                {p.age_restricted && <span className="badge">ID REQ</span>}
                {promoLabel(p.promo_type, p.promo_value) && (
                  <span className="badge promo">{promoLabel(p.promo_type, p.promo_value)}</span>
                )}
                <div className="nm">{p.name}</div>
                <div className="pr">{money(p.price)}</div>
              </button>
            ))}
            {visible.length === 0 && <div className="empty" style={{ gridColumn: "1/-1", marginTop: 30 }}>No products here yet.</div>}
          </div>
        )}

        <div className="fn-bar">
          <div className="fn-groups">
            {fnGroups.map((g) => (
              <div key={g.title} className="fn-group" role="group" aria-label={g.title}>
                <div className="fn-group-title">{g.title}</div>
                <div className="fn-group-btns">
                  {g.btns.map((b) => (
                    <button key={b.label} className={`fn-btn ${b.danger ? "danger" : ""}`} onClick={b.onClick}>{b.label}</button>
                  ))}
                </div>
              </div>
            ))}
          </div>
          <div className="numpad">
            {["7","8","9","4","5","6","1","2","3","0","00"].map((d) => (
              <button key={d} className="np-key" onClick={() => setPad((p) => (p + d).slice(0, 7))}>{d}</button>
            ))}
            <button className="np-key ghost" aria-label="Backspace" onClick={() => setPad((p) => p.slice(0, -1))}>⌫</button>
            <button className="np-key clear" aria-label="Clear amount" onClick={() => setPad("")}>C</button>
            <div className="np-display mono" aria-live="polite">{padCents > 0 ? money(padCents) : "$0.00"}</div>
          </div>
        </div>
      </section>

      <aside className="cart">
        <div className="cart-head">
          <h2>Current Sale</h2>
          <div className="cart-tools">
            <button className="link" onClick={openResume}>Resume</button>
          </div>
        </div>
        <div className="cust-row">
          {customer ? (
            <div className="cust-chip">
              <span>{customer.name}</span>
              <span className="cust-pts">{customer.loyalty_points} pts</span>
              <button className="cust-x" aria-label="Remove customer" onClick={() => setCustomer(null)}>×</button>
            </div>
          ) : (
            <button className="link" onClick={() => setCustModal(true)}>+ Add customer (loyalty)</button>
          )}
        </div>
        <div className="lines">
          {cart.lines.length === 0 ? (
            <div className="empty">No items yet. Scan something to start.</div>
          ) : (
            cart.lines.map((l) => (
              <div key={l.uid} className="line">
                <div className="info">
                  <div className="nm">{l.product.name}</div>
                  <div className="ea">{money(l.priceOverride ?? l.product.price)} ea</div>
                </div>
                <div className="qty">
                  <button aria-label={`Decrease ${l.product.name}`} onClick={() => cart.setQty(l.uid, l.qty - 1)}>–</button>
                  <span>{l.qty}</span>
                  <button aria-label={`Increase ${l.product.name}`} onClick={() => cart.setQty(l.uid, l.qty + 1)}>+</button>
                </div>
                <div className="lt">{money(promoLineTotal(l.priceOverride ?? l.product.price, l.qty, l.product.promo_type, l.product.promo_value))}</div>
              </div>
            ))
          )}
        </div>
        <div className="totals">
          <div className="row"><span>Subtotal</span><span>{money(cart.subtotal())}</span></div>
          <div className="row"><span>Tax</span><span>{money(cart.tax())}</span></div>
          <div className="row grand"><span>Total</span><span>{money(cart.total())}</span></div>
        </div>
        <div className="quick-tender">
          {([
            ["Exact", () => cart.total()],
            ["Next $", () => nextDollar(cart.total())],
            ["$1", () => 100], ["$5", () => 500],
            ["$10", () => 1000], ["$20", () => 2000],
            ["$50", () => 5000], ["$100", () => 10000],
          ] as [string, () => number][]).map(([label, amt]) => (
            <button key={label} disabled={!activeShift || cart.lines.length === 0}
              onClick={() => beginCharge(amt())}>{label}</button>
          ))}
        </div>
        <button className="charge" disabled={cart.lines.length === 0 || !activeShift} onClick={() => beginCharge(null)}>
          {activeShift ? `Charge ${money(cart.total())}` : "Open a shift first"}
        </button>
      </aside>

      {stage === "age" && <AgeModal onConfirm={() => setStage("tender")} onCancel={() => { setStage("idle"); setPresetTender(null); }} />}
      {stage === "tender" && (
        <TenderModal total={cart.total()} customer={customer} preset={presetTender} busy={busy}
          onCancel={() => { setStage("idle"); setPresetTender(null); }} onSubmit={finalize} />
      )}
      {stage === "receipt" && receipt && <ReceiptModal receipt={receipt} onClose={() => setStage("idle")} />}
      {custModal && (
        <CustomerModal onAttach={(c) => { setCustomer(c); setCustModal(false); }} onClose={() => setCustModal(false)} />
      )}
      {drawer && (
        <DrawerModal initialType={drawer.type} initialReason={drawer.reason} onClose={() => setDrawer(null)} />
      )}
      {suspList && (
        <div className="scrim" onClick={() => setSuspList(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Suspended sales</h3>
            {suspList.length === 0 ? <p>Nothing suspended.</p> : suspList.map((sp) => {
              const n = (JSON.parse(sp.cart_json) as unknown[]).length;
              return (
                <div className="srow" key={sp.id}>
                  <span>#{sp.id} · {n} item{n === 1 ? "" : "s"} · {sp.created_at}</span>
                  <button className="link" onClick={() => doResume(sp.id)}>Recall</button>
                </div>
              );
            })}
            <div className="modal-actions">
              <button className="btn" onClick={() => setSuspList(null)}>Close</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function AgeModal({ onConfirm, onCancel }: { onConfirm: () => void; onCancel: () => void }) {
  return (
    <div className="scrim" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>Age verification required</h3>
        <p>This sale contains an age-restricted item. Confirm the customer is of legal age with a valid ID.</p>
        <div className="modal-actions">
          <button className="btn" onClick={onCancel}>Cancel</button>
          <button className="btn primary" onClick={onConfirm}>ID verified</button>
        </div>
      </div>
    </div>
  );
}

function TenderModal({ total, customer, preset, busy, onCancel, onSubmit }: {
  total: number; customer: Customer | null; preset: number | null; busy: boolean; onCancel: () => void;
  onSubmit: (t: { kind: "cash" | "card"; tendered: number }, redeem: boolean) => void;
}) {
  const [kind, setKind] = useState<"cash" | "card">("cash");
  const [redeem, setRedeem] = useState(false);
  const pts = customer?.loyalty_points ?? 0;
  const discount = redeem ? Math.min(1000, total) : 0;
  const due = total - discount;
  const [tendered, setTendered] = useState(preset ?? total);
  const quick = [due, nextDollar(due), 500, 1000, 2000, 10000];
  const change = kind === "cash" ? Math.max(0, tendered - due) : 0;
  const short = kind === "cash" && tendered < due;

  return (
    <div className="scrim" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>Take payment</h3>
        <div className="tender-tabs">
          <button className={kind === "cash" ? "on" : ""} onClick={() => { setKind("cash"); setTendered(preset ?? due); }}>Cash</button>
          <button className={kind === "card" ? "on" : ""} onClick={() => { setKind("card"); setTendered(due); }}>Card</button>
        </div>
        {kind === "cash" && (
          <div className="cashgrid">{quick.map((c, i) => (<button key={i} onClick={() => setTendered(c)}>{money(c)}</button>))}</div>
        )}
        {customer && pts >= 500 && (
          <label className="field check">
            <input type="checkbox" checked={redeem} onChange={(e) => { setRedeem(e.target.checked); setTendered(total - (e.target.checked ? Math.min(1000, total) : 0)); }} />
            Redeem 500 pts (−{money(Math.min(1000, total))})
          </label>
        )}
        {customer && pts < 500 && (
          <p className="note">{customer.name} has {pts} pts — {500 - pts} more to a $10 reward.</p>
        )}
        {discount > 0 && <div className="due"><span>Loyalty discount</span><span>−{money(discount)}</span></div>}
        <div className="due"><span>Total due</span><span>{money(due)}</span></div>
        {kind === "cash" && <div className="due change"><span>Change</span><span>{money(change)}</span></div>}
        {kind === "card" && <p>Card flow is mocked — no real processing (PCI scope is out of scope by design).</p>}
        <div className="modal-actions">
          <button className="btn" onClick={onCancel}>Cancel</button>
          <button className="btn primary" disabled={short || busy}
            onClick={() => onSubmit({ kind, tendered: kind === "card" ? due : tendered }, redeem)}>
            {busy ? "Processing…" : "Complete sale"}
          </button>
        </div>
      </div>
    </div>
  );
}

function ReceiptModal({ receipt, onClose }: { receipt: Receipt; onClose: () => void }) {
  function receiptText(): string {
    const w = 34;
    const line = (l: string, r: string) => l.padEnd(w - r.length, " ") + r;
    const rows = [
      receipt.store_name.toUpperCase(),
      `Sale #${receipt.id} · ${receipt.created_at}`,
      `Cashier: ${receipt.cashier}`,
      "-".repeat(w),
      ...receipt.items.map((it) => line(`${it.qty}x ${it.name}`.slice(0, w - 8), money(it.line_total))),
      "-".repeat(w),
      line("Subtotal", money(receipt.subtotal)),
      line("Tax", money(receipt.tax)),
      ...(receipt.discount > 0 ? [line("Loyalty discount", `-${money(receipt.discount)}`)] : []),
      line("TOTAL", money(receipt.total)),
      line(`Paid (${receipt.tender_kind})`, money(receipt.tendered)),
      line("Change", money(receipt.change)),
      ...(receipt.points_balance != null
        ? [line("Points earned", `+${receipt.points_earned}`),
           ...(receipt.points_redeemed > 0 ? [line("Points redeemed", `-${receipt.points_redeemed}`)] : []),
           line("Points balance", String(receipt.points_balance))]
        : []),
      "-".repeat(w),
      receipt.footer,
    ];
    return rows.join("\n");
  }
  async function copy() {
    try { await navigator.clipboard.writeText(receiptText()); } catch { /* clipboard unavailable */ }
  }
  return (
    <div className="scrim" onClick={onClose}>
      <div className="modal receipt-modal" onClick={(e) => e.stopPropagation()}>
        <div className="receipt-paper">
          <div className="rc-store">{receipt.store_name}</div>
          <div className="rc-meta">Sale #{receipt.id} · {receipt.created_at}</div>
          <div className="rc-meta">Cashier: {receipt.cashier}</div>
          <div className="rc-rule" />
          <div className="receipt-items">
            {receipt.items.map((it, i) => (
              <div className="ri" key={i}><span>{it.qty}× {it.name}</span><span>{money(it.line_total)}</span></div>
            ))}
          </div>
          <div className="rc-rule" />
          <div className="due"><span>Subtotal</span><span>{money(receipt.subtotal)}</span></div>
          <div className="due"><span>Tax</span><span>{money(receipt.tax)}</span></div>
          {receipt.discount > 0 && <div className="due"><span>Loyalty discount</span><span>−{money(receipt.discount)}</span></div>}
          <div className="due rc-total"><span>Total</span><span>{money(receipt.total)}</span></div>
          <div className="due"><span>Paid ({receipt.tender_kind})</span><span>{money(receipt.tendered)}</span></div>
          <div className="due change"><span>Change</span><span>{money(receipt.change)}</span></div>
          {receipt.points_balance != null && (
            <>
              <div className="rc-rule" />
              <div className="due"><span>Points earned</span><span>+{receipt.points_earned}</span></div>
              {receipt.points_redeemed > 0 && <div className="due"><span>Points redeemed</span><span>−{receipt.points_redeemed}</span></div>}
              <div className="due"><span>Points balance</span><span>{receipt.points_balance}</span></div>
            </>
          )}
          <div className="rc-footer">{receipt.footer}</div>
        </div>
        <div className="modal-actions">
          <button className="btn" onClick={copy}>Copy receipt</button>
          <button className="btn primary" onClick={onClose}>New sale</button>
        </div>
      </div>
    </div>
  );
}

function CustomerModal({ onAttach, onClose }: { onAttach: (c: Customer) => void; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [results, setResults] = useState<Customer[]>([]);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [phone, setPhone] = useState("");
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    const id = setTimeout(() => {
      if (q.trim()) searchCustomers(q).then(setResults).catch(console.error);
      else setResults([]);
    }, 120);
    return () => clearTimeout(id);
  }, [q]);

  async function register() {
    setErr(null);
    if (normalizePhone(phone).length !== 10) { setErr("Enter a valid 10-digit US phone number"); return; }
    try {
      const c = await createCustomer(name, normalizePhone(phone));
      onAttach(c);
    } catch (e) { setErr(String(e)); }
  }

  return (
    <div className="scrim" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>Loyalty customer</h3>
        {!creating ? (
          <>
            <label className="field">Search by name or last 4 digits
              <input value={q} autoFocus placeholder="e.g. 'Maria' or '1234'"
                onChange={(e) => setQ(e.target.value)} />
            </label>
            <div className="cust-results">
              {results.map((c) => (
                <button key={c.id} className="cust-result" onClick={() => onAttach(c)}>
                  <span className="cr-name">{c.name}</span>
                  <span className="cr-phone mono">{formatPhone(c.phone)}</span>
                  <span className="cr-pts">{c.loyalty_points} pts</span>
                </button>
              ))}
              {q.trim() && results.length === 0 && <div className="empty small">No matches.</div>}
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={onClose}>Cancel</button>
              <button className="btn primary" onClick={() => setCreating(true)}>+ New customer</button>
            </div>
          </>
        ) : (
          <>
            <label className="field">Name
              <input value={name} autoFocus onChange={(e) => setName(e.target.value)} />
            </label>
            <label className="field">Phone
              <input value={phone} inputMode="tel" placeholder="+1(813)555-1234"
                onChange={(e) => setPhone(e.target.value)}
                onBlur={() => setPhone(formatPhone(phone))}
                onKeyDown={(e) => { if (e.key === "Enter") register(); }} />
            </label>
            {err && <p className="err">{err}</p>}
            <div className="modal-actions">
              <button className="btn" onClick={() => setCreating(false)}>Back</button>
              <button className="btn primary" onClick={register}>Register & attach</button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
