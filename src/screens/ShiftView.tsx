import { useEffect, useState } from "react";
import { getShiftSummary, money } from "../api";
import { useSession } from "../sessionStore";
import { OpenShiftModal, CloseShiftModal } from "../components/ShiftModals";
import type { ShiftSummary } from "../types";

export default function ShiftView() {
  const activeShift = useSession((s) => s.activeShift);
  const [sum, setSum] = useState<ShiftSummary | null>(null);
  const [modal, setModal] = useState<"open" | "close" | null>(null);

  useEffect(() => {
    if (activeShift) getShiftSummary(activeShift.id).then(setSum).catch(console.error);
    else setSum(null);
  }, [activeShift]);

  if (!activeShift) {
    return (
      <div className="page">
        <div className="page-head"><h1>Shift</h1></div>
        <div className="empty" style={{ marginTop: 40 }}>No open shift.</div>
        <div style={{ textAlign: "center", marginTop: 16 }}>
          <button className="btn primary slim" onClick={() => setModal("open")}>Open a shift</button>
        </div>
        {modal === "open" && <OpenShiftModal onClose={() => setModal(null)} />}
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-head">
        <h1>Shift #{activeShift.id}</h1>
        <button className="btn primary slim" onClick={() => setModal("close")}>Close shift</button>
      </div>
      {sum && (
        <div className="summary card-summary">
          <Row label="Opening float" value={money(sum.opening_float)} />
          <Row label="Cash sales" value={money(sum.cash_sales)} />
          <Row label="Card sales" value={money(sum.card_sales)} />
          <Row label="Cash paid in" value={money(sum.cash_in)} />
          <Row label="Cash paid out" value={`-${money(sum.cash_out)}`} />
          <div className="summary-rule" />
          <Row label="Expected in drawer" value={money(sum.expected_cash)} strong />
          <Row label="Transactions" value={String(sum.txn_count)} muted />
          <Row label="Gross sales" value={money(sum.gross_sales)} muted />
        </div>
      )}
      {modal === "close" && <CloseShiftModal shift={activeShift} onClose={() => setModal(null)} />}
    </div>
  );
}

function Row({ label, value, strong, muted }: { label: string; value: string; strong?: boolean; muted?: boolean }) {
  return (
    <div className={`srow ${strong ? "strong" : ""} ${muted ? "muted" : ""}`}>
      <span>{label}</span><span className="mono">{value}</span>
    </div>
  );
}
