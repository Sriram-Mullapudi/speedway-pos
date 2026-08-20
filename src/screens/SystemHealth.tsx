import { useEffect, useState } from "react";
import { systemHealth, listDevices, fmtBytes, fmtTs, type HealthReport, type DeviceInfo } from "../api";
import { notify } from "../components/Toast";

function statusPill(s: string) {
  const good = ["Healthy", "ok", "Valid", "Compatible"].includes(s);
  const bad = ["Error", "Failed", "failed", "Invalid"].includes(s);
  const cls = good ? "good" : bad ? "low" : "ok";
  return <span className={`pill ${cls}`}>{s}</span>;
}

export default function SystemHealth() {
  const [h, setH] = useState<HealthReport | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [checking, setChecking] = useState(false);

  const load = (integrity: boolean) => systemHealth(integrity).then(setH).catch((e) => notify(String(e), "err"));
  useEffect(() => { load(false); listDevices().then(setDevices).catch(() => {}); }, []);

  async function runCheck() {
    setChecking(true);
    try { await load(true); notify("Database check complete"); }
    finally { setChecking(false); }
  }

  if (!h) return <div className="page"><div className="empty">Loading…</div></div>;

  return (
    <div className="page">
      <div className="page-head"><h1>System Health</h1>
        <button className={`btn slim ${checking ? "busy" : ""}`} disabled={checking} onClick={runCheck}>
          {checking ? "Checking…" : "Run database check"}
        </button>
      </div>

      <div className="health-grid">
        <div className="health-card">
          <div className="hc-title">Database</div>
          <div className="hc-row"><span>Status</span>{statusPill(h.db_status)}</div>
          <div className="hc-row"><span>Schema version</span><span className="mono">v{h.schema_version}</span></div>
          <div className="hc-row"><span>Size</span><span className="mono">{fmtBytes(h.db_size)}</span></div>
          <div className="hc-row"><span>Journal</span><span className="mono">{h.wal_mode}</span></div>
          <div className="hc-row"><span>WAL size</span><span className="mono">{fmtBytes(h.wal_size)}</span></div>
          <div className="hc-row"><span>Integrity</span>{h.integrity === "not_run" ? <span className="muted">Run check</span> : statusPill(h.integrity)}</div>
        </div>

        <div className="health-card">
          <div className="hc-title">Backup</div>
          <div className="hc-row"><span>Status</span>{statusPill(h.backup_status)}</div>
          <div className="hc-row"><span>Last backup</span><span className="mono">{fmtTs(h.last_backup)}</span></div>
          <div className="hc-row"><span>Last type</span><span className="mono">{h.last_backup_kind ?? "—"}</span></div>
          <div className="hc-row"><span>Auto frequency</span><span className="mono">{h.auto_frequency}</span></div>
        </div>

        <div className="health-card">
          <div className="hc-title">Application</div>
          <div className="hc-row"><span>Version</span><span className="mono">{h.app_version}</span></div>
          <div className="hc-row"><span>Schema</span><span className="mono">v{h.schema_version}</span></div>
          <div className="hc-row"><span>Platform</span><span className="mono">{h.platform}</span></div>
        </div>

        <div className="health-card">
          <div className="hc-title">Devices</div>
          {devices.map((d) => (
            <div className="hc-row" key={d.kind}><span>{d.label}</span>
              <span className="pill ok">{d.status.replace(/_/g, " ")}</span></div>
          ))}
        </div>

        <div className="health-card">
          <div className="hc-title">Sync</div>
          <div className="hc-row"><span>Status</span><span className="pill ok">Disabled</span></div>
          <div className="hc-row"><span>Last successful sync</span><span className="muted">Not applicable</span></div>
        </div>
      </div>
    </div>
  );
}
