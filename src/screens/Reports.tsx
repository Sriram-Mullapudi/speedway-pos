import { useEffect, useState } from "react";
import { getReport, money } from "../api";
import type { ReportData } from "../types";

type Period = "today" | "week" | "month" | "all";
const PERIODS: { id: Period; label: string }[] = [
  { id: "today", label: "Today" },
  { id: "week", label: "Week" },
  { id: "month", label: "Month" },
  { id: "all", label: "All time" },
];

// A calm categorical palette for the charts.
const COLORS = ["#2fbf71", "#3aa0ff", "#f5a623", "#e5534b", "#a06cff", "#20c9b0", "#f06fb0", "#8a98a8"];

export default function Reports() {
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

      {!data ? <div className="empty">Loading…</div> : data.txn_count === 0 ? (
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
      )}
    </div>
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
