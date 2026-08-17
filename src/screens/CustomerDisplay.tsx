import { useEffect, useState } from "react";
import { money } from "../api";

/** Customer-safe checkout snapshot — deliberately excludes cost, margin,
 *  audit, drawer, PIN, permissions, and internal errors. */
export interface CustomerView {
  phase: "idle" | "cart" | "paid";
  store: string;
  items: { name: string; qty: number; unit: number; line: number }[];
  subtotal: number;
  tax: number;
  total: number;
  customer?: string | null;
  points?: number | null;
  tendered?: number;
  change?: number;
  pointsEarned?: number;
}

const IDLE: CustomerView = { phase: "idle", store: "Speedway Market", items: [], subtotal: 0, tax: 0, total: 0 };

export default function CustomerDisplay() {
  const [view, setView] = useState<CustomerView>(IDLE);

  useEffect(() => {
    const ch = new BroadcastChannel("speedway-customer-display");
    ch.onmessage = (e) => setView(e.data as CustomerView);
    ch.postMessage({ __ready: true });
    return () => ch.close();
  }, []);

  if (view.phase === "idle") {
    return (
      <div className="cust-display idle">
        <div className="cd-logo">{view.store}</div>
        <div className="cd-welcome">Welcome</div>
        <div className="cd-sub">Please wait while we serve you</div>
      </div>
    );
  }

  if (view.phase === "paid") {
    return (
      <div className="cust-display paid">
        <div className="cd-check">✓</div>
        <div className="cd-big">Payment Complete</div>
        <div className="cd-total">{money(view.total)}</div>
        {view.tendered != null && <div className="cd-row"><span>Tendered</span><span>{money(view.tendered)}</span></div>}
        {view.change != null && <div className="cd-change"><span>Change Due</span><span>{money(view.change)}</span></div>}
        {view.pointsEarned ? <div className="cd-points">You earned {view.pointsEarned} points</div> : null}
        <div className="cd-thanks">Thank you — see you again!</div>
      </div>
    );
  }

  return (
    <div className="cust-display cart">
      <div className="cd-head">{view.store}</div>
      <div className="cd-items">
        {view.items.length === 0 ? (
          <div className="cd-empty">Scanning your items…</div>
        ) : view.items.map((it, i) => (
          <div className="cd-item" key={i}>
            <span className="cd-i-name">{it.qty}× {it.name}</span>
            <span className="cd-i-price">{money(it.line)}</span>
          </div>
        ))}
      </div>
      <div className="cd-totals">
        <div className="cd-row"><span>Subtotal</span><span>{money(view.subtotal)}</span></div>
        <div className="cd-row"><span>Tax</span><span>{money(view.tax)}</span></div>
        <div className="cd-row cd-grand"><span>Total</span><span>{money(view.total)}</span></div>
        {view.customer && <div className="cd-loyalty">{view.customer} · {view.points ?? 0} pts</div>}
      </div>
    </div>
  );
}

/** Publisher used by the register to push customer-safe snapshots. */
export function publishCustomerView(v: CustomerView) {
  try {
    const ch = new BroadcastChannel("speedway-customer-display");
    ch.postMessage(v);
    ch.close();
  } catch { /* display not available — checkout continues normally */ }
}
