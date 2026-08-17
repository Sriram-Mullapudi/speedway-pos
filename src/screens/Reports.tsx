import { useEffect, useState } from "react";
import { getReport, getProfitReport, getLossPrevention, exportCsv, money } from "../api";
import type { ReportData, ProfitReport, LossPreventionRow } from "../types";

type Period = "today" | "week" | "month" | "all";
type ReportView = "sales" | "profit" | "loss";
const PERIODS: { id: Period; label: string }[] = [
  { id: "today", label: "Today" },
  { id: "week", label: "Week" },
  { id: "month", label: "Month" },
  { id: "all", label: "All time" },
];

// A calm categorical palette for the charts.
const COLORS = ["#2fbf71", "#3aa0ff", "#f5a623", "#e5534b", "#a06cff", "#20c9b0", "#f06fb0", "#8a98a8"];

export default function Reports() {
  const [view, setView] = useState<ReportView>("sales");
  const [period, setPeriod] = useState<Period>("today");
  const [data, setData] = useState<ReportData | null>(null);

  useEffect(() => { getReport(period).then(setData).catch(console.error); }, [period]);

  return (
    <div className="page">
      <div className="page-head">
        <h1>Reports</h1>
        <div className="seg">
          {PERIODS.map((p) => (
            <button key={p.id} className={period === p.id ? "on" : ""} onClick={() => setPeriod(p.id)}>{p.label}</button>
          ))}
        </div>
      </div>
      <div className="tender-tabs" style={{ marginBottom: 16 }}>
        <button className={view === "sales" ? "on" : ""} onClick={() => setView("sales")}>Sales</button>
        <button className={view === "profit" ? "on" : ""} onClick={() => setView("profit")}>Profit &amp; Margin</button>
        <button className={view === "loss" ? "on" : ""} onClick={() => setView("loss")}>Loss Prevention</button>
      </div>

      {view === "profit" && <ProfitView period={period} />}
      {view === "loss" && <LossView period={period} />}
      {view === "sales" && (!data ? <div className="empty">Loading…</div> : data.txn_count === 0 ? (
        <div className="empty" style={{ marginTop: 40 }}>No sales in this period yet.</div>
      ) : (
        <>
          <div className="kpis">
            <Kpi label="Gross sales" value={money(data.gross)} />
            <Kpi label="Net (ex-tax)" value={money(data.net)} />
            <Kpi label="Tax collected" value={money(data.tax)} />
            <Kpi label="Transactions" value={String(data.txn_count)} />
            <Kpi label="Avg basket" value={money(data.avg_basket)} />
          </div>

          <div className="report-grid">
            <Panel title="Department breakdown">
              <Pie data={data.by_department.map((d) => ({ label: d.department, value: d.sales }))} />
            </Panel>
            <Panel title="Payment methods">
              <Bars data={data.by_payment.map((p) => ({ label: p.kind.replace("_", " "), value: p.amount }))} money />
            </Panel>
            <Panel title="Top products" wide>
              <Bars data={data.top_products.map((p) => ({ label: p.name, value: p.qty }))} />
            </Panel>
          </div>
        </>
      ))}
    </div>
  );
}

function ProfitView({ period }: { period: Period }) {
  const [rep, setRep] = useState<ProfitReport | null>(null);
  useEffect(() => { getProfitReport(period).then(setRep).catch(console.error); }, [period]);
  if (!rep) return <div className="empty">Loading…</div>;
  if (rep.total_revenue === 0) return <div className="empty" style={{ marginTop: 40 }}>No sales in this period yet.</div>;
  const uncosted = rep.total_revenue - rep.costed_revenue;
  return (
    <>
      <div className="kpis">
        <Kpi label="Revenue" value={money(rep.total_revenue)} />
        <Kpi label="Cost (known)" value={money(rep.total_cost)} />
        <Kpi label="Gross profit" value={money(rep.gross_profit)} />
        <Kpi label="Margin" value={`${rep.margin_pct.toFixed(1)}%`} />
      </div>
      {uncosted > 0 && (
        <p className="hint">{money(uncosted)} of revenue came from items with no recorded historical cost (sold before cost tracking began) and is excluded from profit — never estimated.</p>
      )}
      <div className="page-head"><span /><button className="btn slim" onClick={() => exportCsv(`profit-${period}.csv`, rep.by_department as unknown as Record<string, unknown>[])}>Export CSV</button></div>
      <table className="table">
        <thead><tr><th>Department</th><th className="num">Revenue</th><th className="num">Cost</th><th className="num">Profit</th><th className="num">Margin</th><th className="num">% costed</th></tr></thead>
        <tbody>
          {rep.by_department.map((r) => (
            <tr key={r.department}>
              <td>{r.department}</td>
              <td className="num mono">{money(r.revenue)}</td>
              <td className="num mono">{money(r.cost)}</td>
              <td className="num mono">{money(r.profit)}</td>
              <td className="num mono">{r.margin_pct.toFixed(1)}%</td>
              <td className="num mono">{r.costed_pct.toFixed(0)}%</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

function LossView({ period }: { period: Period }) {
  const [rows, setRows] = useState<LossPreventionRow[] | null>(null);
  useEffect(() => { getLossPrevention(period).then(setRows).catch(console.error); }, [period]);
  if (!rows) return <div className="empty">Loading…</div>;
  return (
    <>
      <p className="hint">Shrink signals by cashier: voids, refunds, no-sales, and cumulative drawer over/short. High values aren't proof of wrongdoing — they're where to look.</p>
      <div className="page-head"><span /><button className="btn slim" onClick={() => exportCsv(`loss-prevention-${period}.csv`, rows as unknown as Record<string, unknown>[])}>Export CSV</button></div>
      <table className="table">
        <thead><tr><th>Cashier</th><th className="num">Voids</th><th className="num">Void $</th><th className="num">Refunds</th><th className="num">Refund $</th><th className="num">No-sales</th><th className="num">Over/Short</th></tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.cashier}>
              <td>{r.cashier}</td>
              <td className="num mono">{r.void_count}</td>
              <td className="num mono">{money(r.void_amount)}</td>
              <td className="num mono">{r.refund_count}</td>
              <td className="num mono">{money(r.refund_amount)}</td>
              <td className="num mono">{r.no_sale_count}</td>
              <td className="num mono" style={{ color: r.over_short < 0 ? "var(--danger)" : undefined }}>{money(r.over_short)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

function Kpi({ label, value }: { label: string; value: string }) {
  return <div className="kpi"><div className="kpi-v">{value}</div><div className="kpi-l">{label}</div></div>;
}

function Panel({ title, wide, children }: { title: string; wide?: boolean; children: React.ReactNode }) {
  return (
    <div className={`panel ${wide ? "wide" : ""}`}>
      <div className="panel-title">{title}</div>
      {children}
    </div>
  );
}

// ---- charts (plain SVG, no deps) ----
function Pie({ data }: { data: { label: string; value: number }[] }) {
  const total = data.reduce((s, d) => s + d.value, 0) || 1;
  let angle = -Math.PI / 2;
  const R = 70, C = 90;
  const slices = data.map((d, i) => {
    const frac = d.value / total;
    const a0 = angle;
    const a1 = angle + frac * Math.PI * 2;
    angle = a1;
    const large = a1 - a0 > Math.PI ? 1 : 0;
    const x0 = C + R * Math.cos(a0), y0 = C + R * Math.sin(a0);
    const x1 = C + R * Math.cos(a1), y1 = C + R * Math.sin(a1);
    const path = `M ${C} ${C} L ${x0} ${y0} A ${R} ${R} 0 ${large} 1 ${x1} ${y1} Z`;
    return { path, color: COLORS[i % COLORS.length], label: d.label, pct: Math.round(frac * 100) };
  });
  return (
    <div className="pie-wrap">
      <svg viewBox="0 0 180 180" width="180" height="180">
        {slices.map((s, i) => <path key={i} d={s.path} fill={s.color} stroke="var(--panel)" strokeWidth="1" />)}
      </svg>
      <div className="legend">
        {slices.map((s, i) => (
          <div className="leg" key={i}>
            <span className="dot" style={{ background: s.color }} />
            <span className="leg-l">{s.label}</span>
            <span className="leg-v">{s.pct}%</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function Bars({ data, money: asMoney }: { data: { label: string; value: number }[]; money?: boolean }) {
  const max = Math.max(...data.map((d) => d.value), 1);
  return (
    <div className="bars">
      {data.map((d, i) => (
        <div className="bar-row" key={i}>
          <span className="bar-label" title={d.label}>{d.label}</span>
          <div className="bar-track">
            <div className="bar-fill" style={{ width: `${(d.value / max) * 100}%`, background: COLORS[i % COLORS.length] }} />
          </div>
          <span className="bar-val mono">{asMoney ? money(d.value) : d.value}</span>
        </div>
      ))}
    </div>
  );
}
