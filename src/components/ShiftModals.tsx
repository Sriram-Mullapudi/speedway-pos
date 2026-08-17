import { useEffect, useState } from "react";
import {
  openShift, closeShift, getShiftSummary, createCashDrawerEvent, managerOverride, money,
} from "../api";
import { useSession } from "../sessionStore";
import PinPad from "./PinPad";
import type { Shift, ShiftSummary, DrawerEventType } from "../types";

const toCents = (d: string) => Math.round((parseFloat(d) || 0) * 100);

export function OpenShiftModal({ onClose }: { onClose: () => void }) {
  const setShift = useSession((s) => s.setShift);
  const [float, setFloat] = useState("100.00");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  async function go() {
    if (busy) return;
    setBusy(true);
    try { const shift = await openShift(toCents(float)); setShift(shift); onClose(); }
    catch (e) { setErr(String(e)); }
    finally { setBusy(false); }
  }
  return (
    <Scrim onClose={onClose}>
      <h3>Open shift</h3>
      <p>Count the cash already in the drawer and enter it as your starting float.</p>
      <label className="field">Starting float $
        <input value={float} onChange={(e) => setFloat(e.target.value)} inputMode="decimal" autoFocus />
      </label>
      {err && <p className="err">{err}</p>}
      <Actions onClose={onClose} onConfirm={go} confirmLabel="Open shift" />
    </Scrim>
  );
}

export function CloseShiftModal({ shift, onClose }: { shift: Shift; onClose: () => void }) {
  const setShift = useSession((s) => s.setShift);
  const [sum, setSum] = useState<ShiftSummary | null>(null);
  const [counted, setCounted] = useState("");
  const [result, setResult] = useState<ShiftSummary | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => { getShiftSummary(shift.id).then(setSum).catch((e) => setErr(String(e))); }, [shift.id]);

  const [busy, setBusy] = useState(false);
  async function go() {
    if (busy) return;
    if (!counted.trim()) { setErr("Count the drawer and enter the amount before closing."); return; }
    setBusy(true);
    try { const r = await closeShift(shift.id, toCents(counted)); setResult(r); setShift(null); }
    catch (e) { setErr(String(e)); }
    finally { setBusy(false); }
  }

  if (result) {
    const os = result.over_short ?? 0;
    return (
      <Scrim onClose={onClose}>
        <h3>Shift closed</h3>
        <Row label="Expected cash" value={money(result.expected_cash)} />
        <Row label="Counted cash" value={money(result.counted_cash ?? 0)} />
        <div className={`overshort ${os === 0 ? "ok" : os > 0 ? "over" : "short"}`}>
          {os === 0 ? "Balanced" : os > 0 ? `Over ${money(os)}` : `Short ${money(-os)}`}
        </div>
        <Actions onClose={onClose} onConfirm={onClose} confirmLabel="Done" hideCancel />
      </Scrim>
    );
  }

  return (
    <Scrim onClose={onClose}>
      <h3>Close shift</h3>
      {sum ? (
        <div className="summary">
          <Row label="Opening float" value={money(sum.opening_float)} />
          <Row label="Cash sales" value={money(sum.cash_sales)} />
          <Row label="Cash paid in" value={money(sum.cash_in)} />
          <Row label="Cash paid out" value={`-${money(sum.cash_out)}`} />
          <Row label="Cash refunds" value={`-${money(sum.cash_refunds)}`} />
          <div className="summary-rule" />
          <Row label="Expected in drawer" value={money(sum.expected_cash)} strong />
          <Row label="Card sales" value={money(sum.card_sales)} muted />
          <Row label="Transactions" value={String(sum.txn_count)} muted />
        </div>
      ) : <p>Loading…</p>}
      <label className="field">Counted cash $
        <input value={counted} onChange={(e) => setCounted(e.target.value)} inputMode="decimal" placeholder="e.g. 245.50" autoFocus />
      </label>
      {err && <p className="err">{err}</p>}
      <Actions onClose={onClose} onConfirm={go} confirmLabel="Close shift" />
    </Scrim>
  );
}

export function DrawerModal({ onClose, initialType, initialReason }: {
  onClose: () => void; initialType?: DrawerEventType; initialReason?: string;
}) {
  const isManager = useSession((s) => s.isManager());
  const [type, setType] = useState<DrawerEventType>(initialType ?? "no_sale");
  const [amount, setAmount] = useState("");
  const [reason, setReason] = useState(initialReason ?? "");
  const [override, setOverride] = useState<{ action: string } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  const sensitive = type === "no_sale" || type === "paid_out";
  const needsOverride = sensitive && !isManager;
  const amountCents = toCents(amount);
  // A no-sale must carry a reason; paid in/out must carry a positive amount.
  const invalid =
    (type === "no_sale" && !reason.trim()) ||
    (type !== "no_sale" && amountCents <= 0);

  const [busy, setBusy] = useState(false);
  async function submit(managerId: number | null) {
    if (busy) return;
    setBusy(true);
    try {
      await createCashDrawerEvent(type, type === "no_sale" ? 0 : toCents(amount), reason || null, managerId);
      setDone(true);
    } catch (e) { setErr(String(e)); }
    finally { setBusy(false); }
  }
  function go() { if (needsOverride) setOverride({ action: type }); else submit(null); }

  if (done) {
    return (
      <Scrim onClose={onClose}>
        <h3>Drawer event recorded</h3>
        <p>{type.replace("_", " ")} logged to the audit trail.</p>
        <Actions onClose={onClose} onConfirm={onClose} confirmLabel="Done" hideCancel />
      </Scrim>
    );
  }
  if (override) {
    return (
      <ManagerOverrideModal action={override.action}
        onApprove={(id) => { setOverride(null); submit(id); }}
        onCancel={() => setOverride(null)} />
    );
  }
  return (
    <Scrim onClose={onClose}>
      <h3>Cash drawer</h3>
      <div className="tender-tabs">
        {(["no_sale","paid_in","paid_out","safe_drop"] as DrawerEventType[]).map((t) => (
          <button key={t} className={type === t ? "on" : ""} onClick={() => setType(t)}>{t.replace("_", " ")}</button>
        ))}
      </div>
      {type !== "no_sale" && (
        <label className="field">Amount $
          <input value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" autoFocus />
        </label>
      )}
      <label className="field">Reason{type === "no_sale" ? " (required)" : ""}
        <input value={reason} onChange={(e) => setReason(e.target.value)}
          placeholder={type === "no_sale" ? "Why is the drawer being opened?" : "Optional note"} />
      </label>
      {needsOverride && <p className="note">A manager PIN is required for {type.replace("_", " ")}.</p>}
      {err && <p className="err">{err}</p>}
      <Actions onClose={onClose} onConfirm={go} confirmLabel={needsOverride ? "Manager approve" : "Record"} disabled={invalid} />
    </Scrim>
  );
}

export function ManagerOverrideModal({
  action, onApprove, onCancel,
}: { action: string; onApprove: (managerId: number) => void; onCancel: () => void }) {
  const [pin, setPin] = useState("");
  const [err, setErr] = useState<string | null>(null);
  async function go() {
    try { const id = await managerOverride(action, pin); onApprove(id); }
    catch (e) { setErr(String(e)); setPin(""); }
  }
  return (
    <Scrim onClose={onCancel}>
      <h3>Manager approval</h3>
      <p>Authorize <b>{action.replace("_", " ")}</b> with a manager PIN.</p>
      <PinPad value={pin} onChange={(v) => { setPin(v); setErr(null); }} onSubmit={go} maxLen={4} />
      {err && <p className="err">{err}</p>}
      <Actions onClose={onCancel} onConfirm={go} confirmLabel="Approve" />
    </Scrim>
  );
}

function Scrim({ children, onClose }: { children: React.ReactNode; onClose: () => void }) {
  return (
    <div className="scrim" onClick={onClose}>
      <div className="modal wide" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>{children}</div>
    </div>
  );
}
function Actions({ onClose, onConfirm, confirmLabel, hideCancel, disabled }:
  { onClose: () => void; onConfirm: () => void; confirmLabel: string; hideCancel?: boolean; disabled?: boolean }) {
  return (
    <div className="modal-actions">
      {!hideCancel && <button className="btn" onClick={onClose}>Cancel</button>}
      <button className="btn primary" onClick={onConfirm} disabled={disabled}>{confirmLabel}</button>
    </div>
  );
}
function Row({ label, value, strong, muted }:
  { label: string; value: string; strong?: boolean; muted?: boolean }) {
  return (
    <div className={`srow ${strong ? "strong" : ""} ${muted ? "muted" : ""}`}>
      <span>{label}</span><span className="mono">{value}</span>
    </div>
  );
}
