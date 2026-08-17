import { useEffect, useState } from "react";
import { listCustomers, createCustomer, money, formatPhone, normalizePhone } from "../api";
import type { Customer } from "../types";

export default function Customers() {
  const [rows, setRows] = useState<Customer[]>([]);
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [phone, setPhone] = useState("");
  const [err, setErr] = useState<string | null>(null);

  async function reload() { setRows(await listCustomers()); }
  useEffect(() => { reload().catch(console.error); }, []);

  async function add() {
    try {
      if (normalizePhone(phone).length !== 10) { setErr("Enter a valid 10-digit US phone number"); return; }
      await createCustomer(name, normalizePhone(phone));
      setAdding(false); setName(""); setPhone("");
      reload();
    } catch (e) { setErr(String(e)); }
  }

  return (
    <div className="page">
      <div className="page-head">
        <h1>Customers</h1>
        <button className="btn primary slim" onClick={() => setAdding(true)}>+ Add customer</button>
      </div>
      <table className="table">
        <thead><tr><th>Name</th><th>Phone</th><th className="num">Points</th><th className="num">Point value</th><th>Since</th></tr></thead>
        <tbody>
          {rows.map((c) => (
            <tr key={c.id}>
              <td>{c.name}</td>
              <td className="mono">{formatPhone(c.phone)}</td>
              <td className="num mono">{c.loyalty_points}</td>
              <td className="num mono">{money(c.loyalty_points)}</td>
              <td className="mono">{c.created_at}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <div className="empty" style={{ marginTop: 30 }}>No loyalty customers yet — register one at the register or here.</div>}

      {adding && (
        <div className="scrim" onClick={() => setAdding(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>New customer</h3>
            <label className="field">Name
              <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
            </label>
            <label className="field">Phone
              <input value={phone} inputMode="tel" placeholder="+1(813)555-1234"
                onChange={(e) => setPhone(e.target.value)} onBlur={() => setPhone(formatPhone(phone))} />
            </label>
            {err && <p className="err">{err}</p>}
            <div className="modal-actions">
              <button className="btn" onClick={() => setAdding(false)}>Cancel</button>
              <button className="btn primary" onClick={add}>Save</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
