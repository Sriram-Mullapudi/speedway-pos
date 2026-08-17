import { useEffect, useState } from "react";
import { listCatalog, listLowStock, adjustStock, listMovements, money } from "../api";
import { useSession } from "../sessionStore";
import type { CatalogRow, Movement } from "../types";

export default function Inventory() {
  const [rows, setRows] = useState<CatalogRow[]>([]);
  const [lowOnly, setLowOnly] = useState(false);
  const [active, setActive] = useState<CatalogRow | null>(null);

  async function reload() {
    setRows(lowOnly ? await listLowStock() : await listCatalog());
  }
  useEffect(() => { reload().catch(console.error); }, [lowOnly]);

  const isLow = (p: CatalogRow) => p.reorder_level > 0 && p.on_hand <= p.reorder_level;

  return (
    <div className="page">
      <div className="page-head">
        <h1>Inventory</h1>
        <label className="toggle">
          <input type="checkbox" checked={lowOnly} onChange={(e) => setLowOnly(e.target.checked)} />
          Low stock only
        </label>
      </div>

      <table className="table">
        <thead>
          <tr>
            <th>Name</th><th>SKU</th>
            <th className="num">On hand</th><th className="num">Reorder at</th>
            <th>Status</th><th className="num">Stock value</th><th></th>
          </tr>
        </thead>
        <tbody>
          {rows.filter((p) => p.active).map((p) => (
            <tr key={p.id}>
              <td>{p.name}</td>
              <td className="mono">{p.sku}</td>
              <td className="num mono">{p.on_hand}</td>
              <td className="num mono">{p.reorder_level || "—"}</td>
              <td>{isLow(p) ? <span className="pill low">LOW</span> : <span className="pill good">OK</span>}</td>
              <td className="num mono">{money(p.on_hand * p.cost)}</td>
              <td><button className="link" onClick={() => setActive(p)}>Receive / Adjust</button></td>
            </tr>
          ))}
        </tbody>
      </table>

      {active && (
        <StockModal
          product={active}
          onClose={() => setActive(null)}
          onDone={() => { setActive(null); reload(); }}
        />
      )}
    </div>
  );
}

function StockModal({
  product, onClose, onDone,
}: {
  product: CatalogRow;
  onClose: () => void;
  onDone: () => void;
}) {
  const [mode, setMode] = useState<"receive" | "adjust">("receive");
  const [qty, setQty] = useState("");
  const [moves, setMoves] = useState<Movement[]>([]);

  useEffect(() => { listMovements(product.id).then(setMoves).catch(console.error); }, [product.id]);

  async function apply() {
    const n = parseInt(qty);
    if (!n) return;
    // Receive is always positive; adjust takes the signed value as typed.
    const delta = mode === "receive" ? Math.abs(n) : n;
    const uid = useSession.getState().session?.cashier_id ?? 1;
    await adjustStock(product.id, delta, mode === "receive" ? "receive" : "adjust", uid);
    onDone();
  }

  return (
    <div className="scrim" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h3>{product.name}</h3>
        <p>On hand: <b>{product.on_hand}</b> · reorder at {product.reorder_level || "—"}</p>

        <div className="tender-tabs">
          <button className={mode === "receive" ? "on" : ""} onClick={() => setMode("receive")}>Receive</button>
          <button className={mode === "adjust" ? "on" : ""} onClick={() => setMode("adjust")}>Adjust</button>
        </div>

        <label className="field">
          {mode === "receive" ? "Units received" : "Change (use − to remove)"}
          <input value={qty} onChange={(e) => setQty(e.target.value)} inputMode="numeric"
            placeholder={mode === "receive" ? "e.g. 24" : "e.g. -3"} autoFocus />
        </label>

        <div className="moves">
          <div className="moves-head">Recent movements</div>
          {moves.length === 0 ? (
            <div className="empty small">No movements yet.</div>
          ) : moves.map((m) => (
            <div className="moverow" key={m.id}>
              <span className={`delta ${m.delta < 0 ? "neg" : "pos"}`}>
                {m.delta > 0 ? "+" : ""}{m.delta}
              </span>
              <span className="reason">{m.reason}</span>
              <span className="when">{m.created_at}</span>
            </div>
          ))}
        </div>

        <div className="modal-actions">
          <button className="btn" onClick={onClose}>Close</button>
          <button className="btn primary" onClick={apply}>Apply</button>
        </div>
      </div>
    </div>
  );
}
