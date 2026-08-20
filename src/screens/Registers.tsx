import { useEffect, useState } from "react";
import {
  listRegisters, upsertRegister, setRegisterActive, getActiveRegister, setActiveRegister,
  type Register,
} from "../api";
import { notify } from "../components/Toast";

export default function Registers() {
  const [rows, setRows] = useState<Register[]>([]);
  const [active, setActive] = useState<Register | null>(null);
  const [editing, setEditing] = useState<{ id?: number; name: string } | null>(null);

  const reload = () => {
    listRegisters().then(setRows).catch((e) => notify(String(e), "err"));
    getActiveRegister().then(setActive).catch(() => {});
  };
  useEffect(() => { reload(); }, []);

  async function save() {
    if (!editing?.name.trim()) { notify("Register name required", "err"); return; }
    try { await upsertRegister(editing); setEditing(null); reload(); notify("Register saved"); }
    catch (e) { notify(String(e), "err"); }
  }

  async function selectThis(id: number) {
    try { await setActiveRegister(id); reload(); notify("This terminal is now that register"); }
    catch (e) { notify(String(e), "err"); }
  }

  return (
    <div className="page">
      <div className="page-head"><h1>Registers</h1>
        <button className="btn primary slim" onClick={() => setEditing({ name: "" })}>+ Add register</button>
      </div>

      <p className="hint">A register is a checkout terminal. This screen manages the store's registers and sets which one <strong>this machine</strong> is. New sales and shifts are stamped with the active register's identity, so reports can be filtered per terminal. This is single-store, multi-lane — no branch syncing.</p>

      {active && (
        <div className="panel" style={{ maxWidth: 480, marginBottom: 16 }}>
          <div className="panel-title">This terminal</div>
          <div className="hc-row"><span>Active register</span><span className="mono">{active.name}</span></div>
          <div className="hc-row"><span>Global ID</span><span className="mono" style={{ fontSize: 11 }}>{active.global_id}</span></div>
        </div>
      )}

      <table className="table">
        <thead><tr><th>ID</th><th>Name</th><th>Global ID</th><th>Status</th><th>Actions</th></tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id} className={r.active ? "" : "muted-row"}>
              <td className="mono">{r.id}</td>
              <td>{r.name}{active?.id === r.id && <span className="pill good" style={{ marginLeft: 8 }}>this terminal</span>}</td>
              <td className="mono" style={{ fontSize: 11 }}>{r.global_id}</td>
              <td><span className={`pill ${r.active ? "good" : "low"}`}>{r.active ? "active" : "inactive"}</span></td>
              <td>
                <button className="link" onClick={() => setEditing({ id: r.id, name: r.name })}>Rename</button>
                {active?.id !== r.id && r.active && <button className="link" onClick={() => selectThis(r.id)}>Use here</button>}
                <button className="link danger" onClick={async () => {
                  try { await setRegisterActive(r.id, !r.active); reload(); }
                  catch (e) { notify(String(e), "err"); }
                }}>{r.active ? "Deactivate" : "Reactivate"}</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {editing && (
        <div className="scrim" onClick={() => setEditing(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>{editing.id ? "Rename register" : "New register"}</h3>
            <label className="field">Name
              <input autoFocus value={editing.name} onChange={(e) => setEditing({ ...editing, name: e.target.value })} />
            </label>
            <div className="modal-actions">
              <button className="btn" onClick={() => setEditing(null)}>Cancel</button>
              <button className="btn primary" onClick={save}>Save</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
