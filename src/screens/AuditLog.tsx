import { useEffect, useState } from "react";
import { listAuditLog, listCashiers } from "../api";
import type { AuditRow, Cashier } from "../types";

export default function AuditLog() {
  const [rows, setRows] = useState<AuditRow[]>([]);
  const [cashiers, setCashiers] = useState<Cashier[]>([]);
  const [action, setAction] = useState("");
  const [userId, setUserId] = useState<number | "">("");
  const [err, setErr] = useState<string | null>(null);

  async function reload() {
    setErr(null);
    try {
      setRows(await listAuditLog(action || null, userId === "" ? null : userId));
    } catch (e) { setErr(String(e)); }
  }
  useEffect(() => { listCashiers().then(setCashiers).catch(console.error); }, []);
  useEffect(() => { const t = setTimeout(reload, 150); return () => clearTimeout(t); }, [action, userId]);

  return (
    <div className="page">
      <div className="page-head">
        <h1>Audit log</h1>
        <span className="pill ok">append-only</span>
      </div>
      <div className="audit-filters">
        <input className="search mini-search" placeholder="Filter by action… (e.g. void, login, drawer, shift)"
          value={action} onChange={(e) => setAction(e.target.value)} />
        <select value={userId} onChange={(e) => setUserId(e.target.value === "" ? "" : +e.target.value)}>
          <option value="">All users</option>
          {cashiers.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
        </select>
      </div>
      {err && <p className="err">{err}</p>}
      <table className="table">
        <thead>
          <tr><th>#</th><th>Time</th><th>User</th><th>Action</th><th>Entity</th><th>Detail</th></tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td className="mono">{r.id}</td>
              <td className="mono">{r.created_at}</td>
              <td>{r.user ?? "—"}</td>
              <td><span className={`pill ${r.action.includes("denied") || r.action.includes("failed") ? "low" : r.action.includes("override") || r.action.includes("void") || r.action.includes("refund") || r.action.includes("no_sale") ? "ok" : "good"}`}>{r.action}</span></td>
              <td>{r.entity ?? "—"}{r.entity_id != null ? ` #${r.entity_id}` : ""}</td>
              <td className="audit-detail">{r.detail ?? ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <div className="empty" style={{ marginTop: 30 }}>No matching audit entries.</div>}
      <p className="hint">Rows are written by the Rust backend on every sensitive action — logins and failed PINs, manager overrides, voids, refunds, drawer events, shift open/close, inventory adjustments, and settings changes. The table is never updated or deleted by the application.</p>
    </div>
  );
}
