import { useEffect, useState } from "react";
import {
  listVendors, upsertVendor, setVendorActive, listPurchaseOrders, setPoStatus,
  reorderSuggestions, adjustInventory, money,
  type Vendor, type PoRow, type ReorderRow,
} from "../api";
import { notify } from "../components/Toast";

type Tab = "vendors" | "orders" | "reorder" | "adjust";

export default function Purchasing() {
  const [tab, setTab] = useState<Tab>("orders");
  return (
    <div className="page">
      <div className="page-head">
        <h1>Purchasing & Inventory</h1>
        <div className="seg">
          {(["orders", "vendors", "reorder", "adjust"] as Tab[]).map((t) => (
            <button key={t} className={tab === t ? "on" : ""} onClick={() => setTab(t)}>
              {t === "orders" ? "Purchase Orders" : t[0].toUpperCase() + t.slice(1)}
            </button>
          ))}
        </div>
      </div>
      {tab === "vendors" && <Vendors />}
      {tab === "orders" && <Orders />}
      {tab === "reorder" && <Reorder />}
      {tab === "adjust" && <Adjust />}
    </div>
  );
}

function Vendors() {
  const [rows, setRows] = useState<Vendor[]>([]);
  const [editing, setEditing] = useState<Partial<Vendor> | null>(null);
  const reload = () => listVendors().then(setRows).catch(console.error);
  useEffect(() => { reload(); }, []);

  async function save() {
    if (!editing?.name?.trim()) { notify("Vendor name required", "err"); return; }
    try { await upsertVendor(editing as Vendor); setEditing(null); reload(); notify("Vendor saved"); }
    catch (e) { notify(String(e), "err"); }
  }

  return (
    <>
      <div className="page-head"><span /><button className="btn primary slim" onClick={() => setEditing({ name: "" })}>+ Add vendor</button></div>
      <table className="table">
        <thead><tr><th>Name</th><th>Contact</th><th>Phone</th><th>Email</th><th>Status</th><th></th></tr></thead>
        <tbody>
          {rows.map((v) => (
            <tr key={v.id} className={v.active ? "" : "muted-row"}>
              <td>{v.name}</td><td>{v.contact ?? "—"}</td><td className="mono">{v.phone ?? "—"}</td>
              <td>{v.email ?? "—"}</td>
              <td><span className={`pill ${v.active ? "good" : "low"}`}>{v.active ? "active" : "inactive"}</span></td>
              <td>
                <button className="link" onClick={() => setEditing(v)}>Edit</button>
                <button className="link danger" onClick={async () => { await setVendorActive(v.id, !v.active); reload(); }}>
                  {v.active ? "Deactivate" : "Reactivate"}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <div className="empty" style={{ marginTop: 30 }}>No vendors yet.</div>}
      {editing && (
        <div className="scrim" onClick={() => setEditing(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>{editing.id ? "Edit vendor" : "New vendor"}</h3>
            {(["name", "contact", "phone", "email", "account_no", "notes"] as const).map((f) => (
              <label className="field" key={f}>{f.replace("_", " ")}
                <input value={(editing[f] as string) ?? ""} onChange={(e) => setEditing({ ...editing, [f]: e.target.value })} />
              </label>
            ))}
            <div className="modal-actions">
              <button className="btn" onClick={() => setEditing(null)}>Cancel</button>
              <button className="btn primary" onClick={save}>Save</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

function Orders() {
  const [rows, setRows] = useState<PoRow[]>([]);
  const reload = () => listPurchaseOrders().then(setRows).catch(console.error);
  useEffect(() => { reload(); }, []);

  async function advance(po: PoRow, status: string) {
    try { await setPoStatus(po.id, status); reload(); notify(`PO #${po.id} ${status}`); }
    catch (e) { notify(String(e), "err"); }
  }

  return (
    <>
      <p className="hint">Purchase orders flow Draft → Ordered → Partial → Received. Receiving converts cases to selling units, records an inventory movement, and updates product cost with history — never rewriting historical sale cost. (PO creation and receiving UI use the backend commands; this view manages status.)</p>
      <table className="table">
        <thead><tr><th>#</th><th>Vendor</th><th>Ref</th><th>Lines</th><th className="num">Cost</th><th>Status</th><th>Created</th><th></th></tr></thead>
        <tbody>
          {rows.map((po) => (
            <tr key={po.id}>
              <td className="mono">{po.id}</td><td>{po.vendor}</td><td>{po.reference ?? "—"}</td>
              <td className="num">{po.line_count}</td><td className="num mono">{money(po.total_cost)}</td>
              <td><span className={`pill ${po.status === "received" ? "good" : po.status === "cancelled" ? "low" : "ok"}`}>{po.status}</span></td>
              <td className="mono">{po.created_at}</td>
              <td>
                {po.status === "draft" && <button className="link" onClick={() => advance(po, "ordered")}>Mark ordered</button>}
                {(po.status === "draft" || po.status === "ordered") && <button className="link danger" onClick={() => advance(po, "cancelled")}>Cancel</button>}
                {po.status === "received" && <button className="link" onClick={() => advance(po, "closed")}>Close</button>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <div className="empty" style={{ marginTop: 30 }}>No purchase orders yet.</div>}
    </>
  );
}

function Reorder() {
  const [rows, setRows] = useState<ReorderRow[]>([]);
  useEffect(() => { reorderSuggestions().then(setRows).catch(console.error); }, []);
  return (
    <>
      <p className="hint">Products at or below reorder level. Suggestions only — Speedway never auto-orders.</p>
      <table className="table">
        <thead><tr><th>Product</th><th className="num">On hand</th><th className="num">Reorder at</th><th className="num">Pack</th><th className="num">Suggested cases</th><th>Preferred vendor</th></tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.product_id}>
              <td>{r.name}</td><td className="num mono">{r.on_hand}</td><td className="num mono">{r.reorder_level}</td>
              <td className="num mono">{r.pack_size}</td><td className="num mono">{r.suggested_cases}</td><td>{r.vendor ?? "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <div className="empty" style={{ marginTop: 30 }}>Nothing needs reordering.</div>}
    </>
  );
}

function Adjust() {
  const [productId, setProductId] = useState("");
  const [delta, setDelta] = useState("");
  const [reason, setReason] = useState("shrink");
  async function submit() {
    const pid = parseInt(productId), d = parseInt(delta);
    if (!pid || !d) { notify("Enter a product id and a non-zero delta", "err"); return; }
    try { await adjustInventory(pid, d, reason); notify("Adjustment recorded"); setDelta(""); }
    catch (e) { notify(String(e), "err"); }
  }
  return (
    <div className="panel" style={{ maxWidth: 460 }}>
      <div className="panel-title">Inventory adjustment</div>
      <p className="hint">Records an append-only movement with a reason code and audits the change. Use negative deltas for damage/spoilage/shrink.</p>
      <label className="field">Product ID<input value={productId} inputMode="numeric" onChange={(e) => setProductId(e.target.value)} /></label>
      <label className="field">Delta (units, +/−)<input value={delta} inputMode="numeric" onChange={(e) => setDelta(e.target.value)} /></label>
      <label className="field">Reason
        <select value={reason} onChange={(e) => setReason(e.target.value)}>
          <option value="shrink">Shrink</option><option value="damage">Damage</option>
          <option value="spoilage">Spoilage</option><option value="correction">Correction</option>
        </select>
      </label>
      <button className="btn primary" onClick={submit}>Record adjustment</button>
    </div>
  );
}
