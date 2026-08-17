import { useEffect, useMemo, useState } from "react";
import { searchProducts, createSale, money, promoLineTotal, promoLabel, friendlyError, type CreateSaleInput } from "../api";
import { ManagerOverrideModal } from "../components/ShiftModals";
import { notify } from "../components/Toast";
import type { Product, Receipt } from "../types";

interface KioskLine { product: Product; qty: number; }

/** Customer-facing kiosk: card-only, age-restricted items blocked, exit requires a manager PIN. */
export default function SelfCheckout({ onExit }: { onExit: () => void }) {
  const [products, setProducts] = useState<Product[]>([]);
  const [query, setQuery] = useState("");
  const [lines, setLines] = useState<KioskLine[]>([]);
  const [done, setDone] = useState<Receipt | null>(null);
  const [exiting, setExiting] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => { searchProducts("").then(setProducts).catch(console.error); }, []);

  const visible = useMemo(
    () => products.filter((p) => p.name.toLowerCase().includes(query.toLowerCase())),
    [products, query]
  );

  const subtotal = lines.reduce((s, l) => s + promoLineTotal(l.product.price, l.qty, l.product.promo_type, l.product.promo_value), 0);
  const tax = lines.reduce((s, l) => s + Math.round(promoLineTotal(l.product.price, l.qty, l.product.promo_type, l.product.promo_value) * l.product.tax_rate), 0);
  const total = subtotal + tax;

  function add(p: Product) {
    if (p.age_restricted) { notify("Age-restricted item — please see the cashier", "err"); return; }
    setLines((ls) => {
      const ex = ls.find((l) => l.product.id === p.id);
      if (ex) return ls.map((l) => (l.product.id === p.id ? { ...l, qty: l.qty + 1 } : l));
      return [...ls, { product: p, qty: 1 }];
    });
  }
  function setQty(id: number, qty: number) {
    setLines((ls) => ls.map((l) => (l.product.id === id ? { ...l, qty } : l)).filter((l) => l.qty > 0));
  }

  async function pay() {
    if (lines.length === 0 || busy) return;
    setBusy(true);
    const input: CreateSaleInput = {
      items: lines.map((l) => ({ product_id: l.product.id, qty: l.qty })),
      tender: { kind: "card", tendered: total },
      age_verified: false,
    };
    try {
      const r = await createSale(input);
      setDone(r);
      setLines([]);
      setTimeout(() => setDone(null), 6000);
    } catch (e) { notify(friendlyError(e), "err"); }
    finally { setBusy(false); }
  }

  if (done) {
    return (
      <div className="kiosk">
        <div className="kiosk-thanks">
          <div className="kt-big">✓ Payment approved</div>
          <div className="kt-total">{money(done.total)}</div>
          <div className="kt-sub">Sale #{done.id} · Thank you, come again!</div>
          <button className="btn primary" onClick={() => setDone(null)}>Start new order</button>
        </div>
      </div>
    );
  }

  return (
    <div className="kiosk">
      <header className="kiosk-head">
        <div className="kh-title">Self Checkout</div>
        <div className="kh-sub">Scan or tap your items · Card payment only · Age-restricted items at the counter</div>
        <button className="kh-exit" onClick={() => setExiting(true)}>Staff</button>
      </header>
      <div className="kiosk-body">
        <div className="kiosk-left">
          <input className="search" placeholder="Scan barcode or search…" value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && visible[0]) { add(visible[0]); setQuery(""); } }} autoFocus />
          <div className="grid kiosk-grid">
            {visible.map((p) => (
              <button key={p.id} className={`card ${p.age_restricted ? "locked" : ""}`} onClick={() => add(p)}>
                {p.age_restricted && <span className="badge">SEE STAFF</span>}
                {promoLabel(p.promo_type, p.promo_value) && <span className="badge promo">{promoLabel(p.promo_type, p.promo_value)}</span>}
                <div className="nm">{p.name}</div>
                <div className="pr">{money(p.price)}</div>
              </button>
            ))}
          </div>
        </div>
        <aside className="cart kiosk-cart">
          <h2>Your order</h2>
          <div className="lines">
            {lines.length === 0 ? (
              <div className="empty">Scan an item to begin.</div>
            ) : lines.map((l) => (
              <div key={l.product.id} className="line">
                <div className="info">
                  <div className="nm">{l.product.name}</div>
                  <div className="ea">{money(l.product.price)} ea</div>
                </div>
                <div className="qty">
                  <button onClick={() => setQty(l.product.id, l.qty - 1)}>–</button>
                  <span>{l.qty}</span>
                  <button onClick={() => setQty(l.product.id, l.qty + 1)}>+</button>
                </div>
                <div className="lt">{money(promoLineTotal(l.product.price, l.qty, l.product.promo_type, l.product.promo_value))}</div>
              </div>
            ))}
          </div>
          <div className="totals">
            <div className="row"><span>Subtotal</span><span>{money(subtotal)}</span></div>
            <div className="row"><span>Tax</span><span>{money(tax)}</span></div>
            <div className="row grand"><span>Total</span><span>{money(total)}</span></div>
          </div>
          <button className="charge" disabled={lines.length === 0 || busy} onClick={pay}>
            {busy ? "Processing…" : `Pay ${money(total)} with card`}
          </button>
        </aside>
      </div>
      {exiting && (
        <ManagerOverrideModal action="settings"
          onApprove={() => { setExiting(false); onExit(); }}
          onCancel={() => setExiting(false)} />
      )}
    </div>
  );
}
