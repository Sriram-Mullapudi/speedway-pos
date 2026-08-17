import { useEffect, useState } from "react";
import { listCashiers, createCashier, updateCashier, deactivateCashier } from "../api";
import type { Cashier } from "../types";

export default function Cashiers() {
  const [rows, setRows] = useState<Cashier[]>([]);
  const [editing, setEditing] = useState<Cashier | "new" | null>(null);
  async function reload() { setRows(await listCashiers()); }
  useEffect(() => { reload().catch(console.error); }, []);

  return (
    <div className="page">
      <div className="page-head">
        <h1>Cashiers</h1>
        <button className="btn primary slim" onClick={() => setEditing("new")}>+ Add cashier</button>
      </div>
      <table className="table">
        <thead><tr><th>Name</th><th>Role</th><th>Status</th><th></th><th></th></tr></thead>
        <tbody>
          {rows.map((c) => (
            <tr key={c.id} className={c.active ? "" : "muted-row"}>
              <td>{c.name}</td>
              <td><span className={`pill ${c.role === "admin" ? "good" : c.role === "manager" ? "ok" : "low"}`}>{c.role}</span></td>
              <td>{c.active ? "Active" : "Inactive"}</td>
              <td><button className="link" onClick={() => setEditing(c)}>Edit</button></td>
              <td>{c.active && <button className="link danger" onClick={async () => {
                if (!window.confirm(`Deactivate ${c.name}? They will no longer be able to sign in.`)) return;
                await deactivateCashier(c.id); reload();
              }}>Deactivate</button>}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {editing && (
        <CashierForm row={editing === "new" ? null : editing}
          onClose={() => setEditing(null)} onSaved={() => { setEditing(null); reload(); }} />
      )}
    </div>
  );
}

function CashierForm({ row, onClose, onSaved }: { row: Cashier | null; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState(row?.name ?? "");
  const [role, setRole] = useState<Cashier["role"]>(row?.role ?? "cashier");
  const [active, setActive] = useState(row?.active ?? true);
  const [pin, setPin] = useState("");
  const [err, setErr] = useState<string | null>(null);

  async function save() {
    try {
      if (row) await updateCashier(row.id, name.trim(), role, active, pin || null);
      else {
        if (!pin) { setErr("A PIN is required for a new cashier"); return; }
        await createCashier(name.trim(), role, pin);
      }
      onSaved();
    } catch (e) { setErr(String(e)); }
  }

  return (
    <div className="scrim" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h3>{row ? "Edit cashier" : "New cashier"}</h3>
        <div className="form">
          <label className="field span2">Name
            <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </label>
          <label className="field">Role
            <select value={role} onChange={(e) => setRole(e.target.value as Cashier["role"])}>
              <option value="cashier">Cashier</option>
              <option value="manager">Manager</option>
              <option value="admin">Admin</option>
            </select>
          </label>
          <label className="field">{row ? "New PIN (blank = keep)" : "PIN"}
            <input value={pin} onChange={(e) => setPin(e.target.value)} inputMode="numeric" placeholder="4 digits" />
          </label>
          {row && (
            <label className="field check">
              <input type="checkbox" checked={active} onChange={(e) => setActive(e.target.checked)} /> Active
            </label>
          )}
        </div>
        {err && <p className="err">{err}</p>}
        <div className="modal-actions">
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn primary" onClick={save}>Save</button>
        </div>
      </div>
    </div>
  );
}
