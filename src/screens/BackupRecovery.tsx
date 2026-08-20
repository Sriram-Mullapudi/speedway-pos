import { useEffect, useState } from "react";
import {
  createManualBackup, listBackupsCmd, validateBackupCmd, restoreBackupCmd,
  getSettings, saveSettings, fmtBytes, fmtTs,
  type BackupMeta, type ValidationResult,
} from "../api";
import { notify } from "../components/Toast";

const FREQ = [
  { id: "disabled", label: "Disabled" },
  { id: "daily", label: "Daily" },
  { id: "every3", label: "Every 3 days" },
  { id: "weekly", label: "Weekly" },
];

export default function BackupRecovery() {
  const [backups, setBackups] = useState<BackupMeta[]>([]);
  const [freq, setFreq] = useState("disabled");
  const [busy, setBusy] = useState(false);
  const [validation, setValidation] = useState<{ file: string; result: ValidationResult } | null>(null);
  const [confirmRestore, setConfirmRestore] = useState<BackupMeta | null>(null);

  const reload = () => listBackupsCmd().then(setBackups).catch((e) => notify(String(e), "err"));
  useEffect(() => {
    reload();
    getSettings().then((m) => setFreq(m.backup_auto_freq ?? "disabled")).catch(() => {});
  }, []);

  async function backupNow() {
    setBusy(true);
    try { const m = await createManualBackup(); notify(`Backup created: ${m.filename}`); reload(); }
    catch (e) { notify(String(e), "err"); }
    finally { setBusy(false); }
  }

  async function saveFreq(v: string) {
    setFreq(v);
    try { await saveSettings({ backup_auto_freq: v }); notify("Automatic backup schedule saved"); }
    catch (e) { notify(String(e), "err"); }
  }

  async function validate(file: string) {
    try { setValidation({ file, result: await validateBackupCmd(file) }); }
    catch (e) { notify(String(e), "err"); }
  }

  async function doRestore(m: BackupMeta) {
    setBusy(true);
    try {
      const msg = await restoreBackupCmd(m.filename);
      setConfirmRestore(null);
      notify(msg);
    } catch (e) { notify(String(e), "err"); }
    finally { setBusy(false); }
  }

  return (
    <div className="page">
      <div className="page-head"><h1>Backup &amp; Recovery</h1>
        <button className={`btn primary slim ${busy ? "busy" : ""}`} disabled={busy} onClick={backupNow}>
          {busy ? "Working…" : "Create backup now"}
        </button>
      </div>

      <div className="panel" style={{ maxWidth: 520, marginBottom: 16 }}>
        <div className="panel-title">Automatic backups</div>
        <label className="field">Schedule
          <select value={freq} onChange={(e) => saveFreq(e.target.value)}>
            {FREQ.map((f) => <option key={f.id} value={f.id}>{f.label}</option>)}
          </select>
        </label>
        <p className="hint">Automatic backups run at startup when eligible and never interrupt checkout. Backups are stored in the application data folder under <code>backups/</code>. Retention keeps recent manual and automatic backups; the newest is never deleted.</p>
      </div>

      <table className="table">
        <thead><tr><th>Created</th><th>Type</th><th className="num">Size</th><th className="num">Schema</th><th>Actions</th></tr></thead>
        <tbody>
          {backups.map((m) => (
            <tr key={m.filename}>
              <td className="mono">{fmtTs(m.created_at)}</td>
              <td><span className={`pill ${m.kind === "safety" ? "ok" : "good"}`}>{m.kind}</span></td>
              <td className="num mono">{fmtBytes(m.backup_size)}</td>
              <td className="num mono">v{m.schema_version}</td>
              <td>
                <button className="link" onClick={() => validate(m.filename)}>Validate</button>
                <button className="link danger" onClick={() => setConfirmRestore(m)}>Restore</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {backups.length === 0 && <div className="empty" style={{ marginTop: 30 }}>No backups yet — create your first backup above.</div>}

      {validation && (
        <div className="scrim" onClick={() => setValidation(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Validation — {validation.file}</h3>
            <div className={`pill ${validation.result.valid ? "good" : "low"}`} style={{ marginBottom: 10 }}>
              {validation.result.valid ? "Valid" : "Invalid"} · {validation.result.compatibility}
            </div>
            <table className="table"><tbody>
              {validation.result.checks.map(([name, ok]) => (
                <tr key={name}><td>{name.replace(/_/g, " ")}</td>
                  <td><span className={`pill ${ok ? "good" : "low"}`}>{ok ? "pass" : "fail"}</span></td></tr>
              ))}
            </tbody></table>
            <p className="hint">{validation.result.message}</p>
            <div className="modal-actions"><button className="btn" onClick={() => setValidation(null)}>Close</button></div>
          </div>
        </div>
      )}

      {confirmRestore && (
        <div className="scrim" onClick={() => setConfirmRestore(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Restore this backup?</h3>
            <div className="restore-warn">
              <strong>This replaces the current database.</strong> Before restoring, Speedway automatically creates and validates a <em>safety backup</em> of the current data. The restore is applied on the next app restart. Any changes made after the selected backup will be lost.
            </div>
            <table className="table"><tbody>
              <tr><td>File</td><td className="mono">{confirmRestore.filename}</td></tr>
              <tr><td>Created</td><td className="mono">{fmtTs(confirmRestore.created_at)}</td></tr>
              <tr><td>Schema</td><td className="mono">v{confirmRestore.schema_version}</td></tr>
              <tr><td>Size</td><td className="mono">{fmtBytes(confirmRestore.backup_size)}</td></tr>
            </tbody></table>
            <div className="modal-actions">
              <button className="btn" onClick={() => setConfirmRestore(null)}>Cancel</button>
              <button className={`btn danger-btn ${busy ? "busy" : ""}`} disabled={busy} onClick={() => doRestore(confirmRestore)}>
                {busy ? "Staging…" : "Create safety backup & restore"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
