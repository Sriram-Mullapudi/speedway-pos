import { useEffect, useMemo, useState } from "react";
import { listCatalog, listCategories, getLayout, saveLayout, money } from "../api";
import type { CatalogRow, Category, TouchButton, TouchLayout } from "../types";

const FUNCTIONS: { code: string; label: string; color: string }[] = [
  { code: "clear", label: "Clear Sale", color: "#e5534b" },
  { code: "void_last", label: "Void Last", color: "#e5534b" },
  { code: "no_sale", label: "No Sale", color: "#a06cff" },
  { code: "cancel", label: "Cancel", color: "#8a98a8" },
];
const PALETTE_COLORS = ["#2fbf71", "#3aa0ff", "#f5a623", "#e5534b", "#a06cff", "#20c9b0"];

function emptyLayout(rows: number, cols: number): TouchLayout {
  return { rows, cols, showNumpad: true, cells: Array(rows * cols).fill(null) };
}

export default function TouchscreenConfig() {
  const [layout, setLayout] = useState<TouchLayout>(emptyLayout(4, 5));
  const [products, setProducts] = useState<CatalogRow[]>([]);
  const [cats, setCats] = useState<Category[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [tab, setTab] = useState<"products" | "departments" | "functions">("products");
  const [pquery, setPquery] = useState("");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    (async () => {
      const [loaded, p, c] = await Promise.all([getLayout(), listCatalog(), listCategories()]);
      if (loaded) setLayout(loaded);
      setProducts(p.filter((x) => x.active));
      setCats(c);
    })().catch(console.error);
  }, []);

  const filteredProducts = useMemo(
    () => products.filter((p) => p.name.toLowerCase().includes(pquery.toLowerCase())),
    [products, pquery]
  );

  function resize(rows: number, cols: number) {
    const next = emptyLayout(rows, cols);
    // preserve overlapping cells by (r,c)
    for (let r = 0; r < Math.min(rows, layout.rows); r++)
      for (let c = 0; c < Math.min(cols, layout.cols); c++)
        next.cells[r * cols + c] = layout.cells[r * layout.cols + c];
    next.showNumpad = layout.showNumpad;
    setLayout(next);
    setSelected(null);
  }

  function assign(index: number, btn: TouchButton | null) {
    setLayout((l) => {
      const cells = [...l.cells];
      cells[index] = btn;
      return { ...l, cells };
    });
    setSaved(false);
  }

  // palette item -> button factory
  function makeProductBtn(p: CatalogRow, color: string): TouchButton {
    return { kind: "product", label: p.name, color, productId: p.id };
  }
  function makeDeptBtn(c: Category, color: string): TouchButton {
    return { kind: "department", label: c.name, color, departmentId: c.id };
  }
  function makeFnBtn(f: { code: string; label: string; color: string }): TouchButton {
    return { kind: "function", label: f.label, color: f.color, functionCode: f.code };
  }

  function placeToSelected(btn: TouchButton) {
    if (selected == null) return;
    assign(selected, btn);
  }

  // drag-drop
  const [drag, setDrag] = useState<TouchButton | null>(null);

  async function persist() {
    await saveLayout(layout);
    setSaved(true);
  }

  return (
    <div className="page tsc">
      <div className="page-head">
        <h1>Touchscreen layout</h1>
        <div className="tsc-controls">
          <label className="mini">Rows
            <select value={layout.rows} onChange={(e) => resize(+e.target.value, layout.cols)}>
              {[2,3,4,5,6,7,8].map((n) => <option key={n} value={n}>{n}</option>)}
            </select>
          </label>
          <label className="mini">Cols
            <select value={layout.cols} onChange={(e) => resize(layout.rows, +e.target.value)}>
              {[3,4,5,6,7,8,9].map((n) => <option key={n} value={n}>{n}</option>)}
            </select>
          </label>
          <label className="toggle mini">
            <input type="checkbox" checked={layout.showNumpad}
              onChange={(e) => { setLayout((l) => ({ ...l, showNumpad: e.target.checked })); setSaved(false); }} />
            Number pad
          </label>
          <button className="btn slim" onClick={() => { setLayout(emptyLayout(layout.rows, layout.cols)); setSaved(false); }}>Reset grid</button>
          <button className="btn primary slim" onClick={persist}>{saved ? "Saved ✓" : "Save layout"}</button>
        </div>
      </div>

      <div className="tsc-body">
        {/* the grid */}
        <div className="tsc-grid" style={{ gridTemplateColumns: `repeat(${layout.cols}, 1fr)` }}>
          {layout.cells.map((cell, i) => (
            <button
              key={i}
              className={`tsc-cell ${selected === i ? "sel" : ""} ${cell ? "" : "unused"}`}
              style={cell ? { background: cell.color, color: "#04130a" } : undefined}
              onClick={() => setSelected(i)}
              onDoubleClick={() => assign(i, null)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={() => { if (drag) assign(i, drag); }}
            >
              {cell ? <span className="tsc-cell-label">{cell.label}</span> : <span className="plus">+</span>}
            </button>
          ))}
        </div>

        {/* palette */}
        <div className="tsc-palette">
          <div className="tsc-hint">
            {selected == null ? "Pick a cell, then a tile — or drag a tile onto the grid." : `Cell ${selected + 1} selected. Double-click a cell to clear it.`}
          </div>
          <div className="tender-tabs">
            <button className={tab === "products" ? "on" : ""} onClick={() => setTab("products")}>Products</button>
            <button className={tab === "departments" ? "on" : ""} onClick={() => setTab("departments")}>Departments</button>
            <button className={tab === "functions" ? "on" : ""} onClick={() => setTab("functions")}>Functions</button>
          </div>

          {tab === "products" && (
            <>
              <input className="search mini-search" placeholder="Filter products…" value={pquery} onChange={(e) => setPquery(e.target.value)} />
              <div className="tsc-tiles">
                {filteredProducts.map((p, i) => {
                  const color = PALETTE_COLORS[i % PALETTE_COLORS.length];
                  const btn = makeProductBtn(p, color);
                  return (
                    <div key={p.id} className="tile" style={{ borderColor: color }}
                      draggable onDragStart={() => setDrag(btn)} onDragEnd={() => setDrag(null)}
                      onClick={() => placeToSelected(btn)}>
                      <span className="tile-l">{p.name}</span>
                      <span className="tile-s">{money(p.price)}</span>
                    </div>
                  );
                })}
              </div>
            </>
          )}

          {tab === "departments" && (
            <div className="tsc-tiles">
              {cats.map((c, i) => {
                const color = PALETTE_COLORS[i % PALETTE_COLORS.length];
                const btn = makeDeptBtn(c, color);
                return (
                  <div key={c.id} className="tile" style={{ borderColor: color }}
                    draggable onDragStart={() => setDrag(btn)} onDragEnd={() => setDrag(null)}
                    onClick={() => placeToSelected(btn)}>
                    <span className="tile-l">{c.name}</span>
                  </div>
                );
              })}
            </div>
          )}

          {tab === "functions" && (
            <div className="tsc-tiles">
              {FUNCTIONS.map((f) => {
                const btn = makeFnBtn(f);
                return (
                  <div key={f.code} className="tile" style={{ borderColor: f.color }}
                    draggable onDragStart={() => setDrag(btn)} onDragEnd={() => setDrag(null)}
                    onClick={() => placeToSelected(btn)}>
                    <span className="tile-l">{f.label}</span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
