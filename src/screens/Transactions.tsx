import { useEffect, useState } from "react";
import { listTransactions, voidTransaction, refundTransaction, money } from "../api";
import { useSession } from "../sessionStore";
import { ManagerOverrideModal } from "../components/ShiftModals";
import type { TxnRow } from "../types";

type Pending = { action: "void" | "refund"; txn: TxnRow };

export default function Transactions() {
  const isManager = useSession((s) => s.isManager());
  const [rows, setRows] = useState<TxnRow[]>([]);
  const [pending, setPending] = useState<Pending | null>(null);
  const [confirm, setConfirm] = useState<Pending | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function reload() { setRows(await listTransactions()); }
  useEffect(() => { reload().catch(console.error); }, []);

  async function run(p: Pending, approvedBy: number | null) {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      if (p.action === "void") await voidTransaction(p.txn.id, approvedBy);
      else await refundTransaction(p.txn.id, approvedBy);
      setConfirm(null);
      setPending(null);
      reload();
    } catch (e) { setErr(String(e)); }
    finally { setBusy(false); }
  }

  function start(p: Pending) {
    if (isManager) setConfirm(p);
    else setPending(p); // cashier → manager PIN first
  }

  return (
    <div className="page">
      <div className="page-head"><h1>Transactions</h1></div>
      {err && <p className="err">{err}</p>}
      <table className="table">
        <thead>
          <tr><th>#</th><th>Time</th><th>Cashier</th><th>Customer</th><th>Type</th>
            <th className="num">Total</th><th>Status</th><th></th><th></th></tr>
        </thead>
        <tbody>
          {rows.map((t) => (
            <tr key={t.id} className={t.status !== "completed" ? "muted-row" : ""}>
              <td className="mono">{t.id}</td>
              <td className="mono">{t.created_at}</td>
              <td>{t.cashier ?? "—"}</td>
              <td>{t.customer ?? "—"}</td>
              <td>{t.kind}</td>
              <td className="num mono">{money(t.total)}{t.discount > 0 ? " *" : ""}</td>
              <td><span className={`pill ${t.status === "completed" ? "good" : t.status === "voided" ? "low" : "ok"}`}>{t.status}</span></td>
              <td>
                {t.kind === "sale" && t.status === "completed" && (
                  <button className="link danger" onClick={() => start({ action: "void", txn: t })}>Void</button>
                )}
              </td>
              <td>
                {t.kind === "sale" && t.status === "completed" && (
                  <button className="link" onClick={() => start({ action: "refund", txn: t })}>Refund</button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="hint">* includes a loyalty discount. Voids restore stock and reverse points; refunds go back to the original payment method — cash refunds count against the shift drawer.</p>

      {pending && (
        <ManagerOverrideModal
          action={pending.action}
          onApprove={(id) => { const p = pending; setPending(null); setConfirm(null); run(p, id); }}
          onCancel={() => setPending(null)}
        />
      )}
      {confirm && (
        <div className="scrim" onClick={() => setConfirm(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>{confirm.action === "void" ? "Void" : "Refund"} sale #{confirm.txn.id}?</h3>
            <p>
              {confirm.action === "void"
                ? "Marks the sale voided, restores stock, and reverses loyalty points."
                : `Returns ${money(confirm.txn.total)} to the original payment method (cash back to the drawer, card as a mocked reversal), restores stock, and reverses loyalty points.`}
            </p>
            <div className="modal-actions">
              <button className="btn" onClick={() => setConfirm(null)}>Cancel</button>
              <button className="btn primary" disabled={busy} onClick={() => run(confirm, null)}>
                {busy ? "Working…" : `Confirm ${confirm.action}`}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
