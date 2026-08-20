import { useState } from "react";
import { diagnosticInfo, exportDiagnosticBundle } from "../api";
import { notify } from "../components/Toast";

const TROUBLESHOOTING = [
  { title: "Device issues", items: [
    "Receipt printer or cash drawer run in simulated mode until real hardware is configured.",
    "The barcode scanner uses keyboard-wedge input — no setup needed; scan into the Register.",
    "Customer display opens as a separate window from Settings → Devices.",
  ]},
  { title: "Backup & recovery", items: [
    "Create manual backups any time from Backup & Recovery. Automatic backups run at startup when eligible.",
    "Restore requires a manager, creates a safety backup first, and applies on the next restart.",
    "If a backup fails, checkout is never affected; System Health will show Attention.",
  ]},
];

export default function HelpSupport() {
  const [diag, setDiag] = useState<string | null>(null);

  async function copyDiag() {
    try {
      const text = await diagnosticInfo();
      setDiag(text);
      await navigator.clipboard.writeText(text).catch(() => {});
      notify("Diagnostic information copied");
    } catch (e) { notify(String(e), "err"); }
  }

  async function exportBundle() {
    try { const path = await exportDiagnosticBundle(); notify(`Diagnostic bundle exported to ${path}`); }
    catch (e) { notify(String(e), "err"); }
  }

  return (
    <div className="page">
      <div className="page-head"><h1>Help &amp; Support</h1></div>

      <div className="panel" style={{ maxWidth: 640, marginBottom: 16 }}>
        <div className="panel-title">About Speedway POS</div>
        <p className="hint">Offline-first point-of-sale for convenience and liquor stores. Version 0.1.0. Your data stays on this machine; no internet connection is required for checkout, backup, restore, or diagnostics.</p>
      </div>

      <div className="panel" style={{ maxWidth: 640, marginBottom: 16 }}>
        <div className="panel-title">Diagnostics</div>
        <p className="hint">Share safe technical details with support. Diagnostics never include customer information, PINs, hashes, payment data, or transaction history.</p>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn slim" onClick={copyDiag}>Copy diagnostic information</button>
          <button className="btn slim" onClick={exportBundle}>Export diagnostic bundle</button>
        </div>
        {diag && <pre className="diag-box">{diag}</pre>}
      </div>

      {TROUBLESHOOTING.map((cat) => (
        <div className="panel" style={{ maxWidth: 640, marginBottom: 16 }} key={cat.title}>
          <div className="panel-title">{cat.title}</div>
          <ul className="help-list">{cat.items.map((it, i) => <li key={i}>{it}</li>)}</ul>
        </div>
      ))}

      <div className="panel" style={{ maxWidth: 640 }}>
        <div className="panel-title">Documentation &amp; contact</div>
        <p className="hint">Documentation and support contact can be configured for your deployment. (Placeholder — a future support-ticket integration can attach here without changing this screen.)</p>
      </div>
    </div>
  );
}
