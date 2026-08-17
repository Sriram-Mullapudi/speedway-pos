import { useEffect, useState } from "react";
import {
  listCatalog, listCategories, upsertProduct, setProductActive, getSettings,
  money, marginPct,
} from "../api";
import type { CatalogRow, Category, ProductInput, PromoType } from "../types";

const toCents = (dollars: string) => Math.round((parseFloat(dollars) || 0) * 100);
const toDollars = (cents: number) => (cents / 100).toFixed(2);

export default function Catalog() {
  const [rows, setRows] = useState<CatalogRow[]>([]);
  const [cats, setCats] = useState<Category[]>([]);
  const [editing, setEditing] = useState<CatalogRow | "new" | null>(null);

  async function reload() {
    const [r, c] = await Promise.all([listCatalog(), listCategories()]);
    setRows(r);
    setCats(c);
  }
  useEffect(() => { reload().catch(console.error); }, []);

  return (
    <div className="page">
      <div className="page-head">
        <h1>Products</h1>
        <button className="btn primary slim" onClick={() => setEditing("new")}>+ Add product</button>
      </div>

      <table className="table">
        <thead>
          <tr>
            <th>Name</th><th>SKU</th><th>Dept</th>
            <th className="num">Cost</th><th className="num">Price</th>
            <th className="num">Margin</th><th className="num">On hand</th>
            <th></th><th></th>
          </tr>
        </thead>
        <tbody>
          {rows.map((p) => (
            <tr key={p.id} className={p.active ? "" : "muted-row"}>
              <td>
                {p.name}
                {p.age_restricted && <span className="tag">ID</span>}
                {p.promo_type !== "none" && <span className="tag promo-tag">{p.promo_type === "bogo" ? "BOGO" : `2nd ${p.promo_value}%`}</span>}
                {p.bonus_points > 0 && <span className="tag pts-tag">+{p.bonus_points}pt</span>}
                {!p.active && <span className="tag off">inactive</span>}
              </td>
              <td className="mono">{p.sku}</td>
              <td>{p.department ?? "—"}</td>
              <td className="num mono">{money(p.cost)}</td>
              <td className="num mono">{money(p.price)}</td>
              <td className="num"><MarginPill price={p.price} cost={p.cost} /></td>
              <td className="num mono">{p.on_hand}</td>
              <td><button className="link" onClick={() => setEditing(p)}>Edit</button></td>
              <td>
                <button className="link danger"
                  onClick={async () => {
                    if (p.active && !window.confirm(`Deactivate ${p.name}? It will stop appearing on the register.`)) return;
                    await setProductActive(p.id, !p.active); reload();
                  }}>
                  {p.active ? "Deactivate" : "Restore"}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {editing && (
        <ProductForm
          row={editing === "new" ? null : editing}
          cats={cats}
          onClose={() => setEditing(null)}
          onSaved={() => { setEditing(null); reload(); }}
        />
      )}
    </div>
  );
}

function MarginPill({ price, cost }: { price: number; cost: number }) {
  const m = marginPct(price, cost);
  const cls = m >= 30 ? "good" : m >= 15 ? "ok" : "low";
  return <span className={`pill ${cls}`}>{m}%</span>;
}

function ProductForm({
  row, cats, onClose, onSaved,
}: {
  row: CatalogRow | null;
  cats: Category[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const [f, setF] = useState({
    name: row?.name ?? "",
    sku: row?.sku ?? "",
    barcode: row?.barcode ?? "",
    category_id: row?.category_id ?? (cats[0]?.id ?? null),
    price: row ? toDollars(row.price) : "",
    cost: row ? toDollars(row.cost) : "",
    taxPct: row ? String(+(row.tax_rate * 100).toFixed(2)) : "0", // default filled from settings below
    age_restricted: row?.age_restricted ?? false,
    reorder_level: String(row?.reorder_level ?? 0),
    bonus_points: String(row?.bonus_points ?? 0),
    promo_type: (row?.promo_type ?? "none") as PromoType,
    promo_value: String(row?.promo_value ?? 0),
  });
  const [err, setErr] = useState<string | null>(null);
  const set = (k: string, v: unknown) => setF((s) => ({ ...s, [k]: v } as typeof s));

  useEffect(() => {
    if (!row) {
      getSettings().then((m) => {
        if (m.default_tax_pct) set("taxPct", m.default_tax_pct);
        if (m.low_stock_default) set("reorder_level", m.low_stock_default);
      }).catch(() => {});
    }
  }, [row]);

  async function save() {
    const input: ProductInput = {
      id: row?.id,
      sku: f.sku.trim(),
      barcode: f.barcode.trim() || null,
      name: f.name.trim(),
      category_id: f.category_id,
      price: toCents(f.price),
      cost: toCents(f.cost),
      tax_rate: (parseFloat(f.taxPct) || 0) / 100,
      age_restricted: f.age_restricted,
      reorder_level: parseInt(f.reorder_level) || 0,
      bonus_points: parseInt(f.bonus_points) || 0,
      promo_type: f.promo_type,
      promo_value: parseInt(f.promo_value) || 0,
    };
    try {
      await upsertProduct(input);
      onSaved();
    } catch (e) {
      setErr(String(e));
    }
  }

  return (
    <div className="scrim" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h3>{row ? "Edit product" : "New product"}</h3>
        <div className="form">
          <label className="field span2">Name
            <input value={f.name} onChange={(e) => set("name", e.target.value)} autoFocus />
          </label>
          <label className="field">SKU
            <input value={f.sku} onChange={(e) => set("sku", e.target.value)} />
          </label>
          <label className="field">Barcode
            <input value={f.barcode} onChange={(e) => set("barcode", e.target.value)} />
          </label>
          <label className="field">Department
            <select value={f.category_id ?? ""} onChange={(e) => set("category_id", e.target.value ? +e.target.value : null)}>
              <option value="">—</option>
              {cats.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </label>
          <label className="field">Tax %
            <input value={f.taxPct} onChange={(e) => set("taxPct", e.target.value)} inputMode="decimal" />
          </label>
          <label className="field">Cost $
            <input value={f.cost} onChange={(e) => set("cost", e.target.value)} inputMode="decimal" />
          </label>
          <label className="field">Price $
            <input value={f.price} onChange={(e) => set("price", e.target.value)} inputMode="decimal" />
          </label>
          <label className="field">Reorder level
            <input value={f.reorder_level} onChange={(e) => set("reorder_level", e.target.value)} inputMode="numeric" />
          </label>
          <label className="field">Bonus pts / unit
            <input value={f.bonus_points} onChange={(e) => set("bonus_points", e.target.value)} inputMode="numeric" />
          </label>
          <label className="field">Promotion
            <select value={f.promo_type} onChange={(e) => set("promo_type", e.target.value as PromoType)}>
              <option value="none">None</option>
              <option value="bogo">BOGO — buy 1 get 1 free</option>
              <option value="second_pct">2nd item % off</option>
            </select>
          </label>
          {f.promo_type === "second_pct" && (
            <label className="field">2nd item discount %
              <input value={f.promo_value} onChange={(e) => set("promo_value", e.target.value)} inputMode="numeric" placeholder="e.g. 30" />
            </label>
          )}
          <label className="field check">
            <input type="checkbox" checked={f.age_restricted}
              onChange={(e) => set("age_restricted", e.target.checked)} />
            Age-restricted (ID required)
          </label>
        </div>
        {err && <p className="err">{err}</p>}
        <div className="modal-actions">
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn primary" onClick={save}>Save product</button>
        </div>
      </div>
    </div>
  );
}
