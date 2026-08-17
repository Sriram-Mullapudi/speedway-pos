import { useEffect, useState } from "react";
import { getSettings, saveSettings, resetDemoData } from "../api";
import { notify } from "../components/Toast";
import { useSession } from "../sessionStore";
import type { SettingsMap } from "../types";
import { THEMES, applyTheme } from "../theme";

const DEFAULTS: SettingsMap = {
  theme: "midnight",
  store_name: "Speedway Market",
  receipt_footer: "Thank you — see you soon!",
  default_tax_pct: "7",
  loyalty_threshold: "500",
  loyalty_reward: "1000",
  low_stock_default: "12",
};

export default function Settings() {
  const [s, setS] = useState<SettingsMap>(DEFAULTS);
  const [confirming, setConfirming] = useState(false);
  const set = (k: string, v: string) => setS((m) => ({ ...m, [k]: v }));

  useEffect(() => {
    getSettings().then((m) => setS({ ...DEFAULTS, ...m })).catch(console.error);
  }, []);

  async function save() {
    for (const k of ["default_tax_pct", "loyalty_threshold", "loyalty_reward", "low_stock_default"]) {
      if (s[k] !== undefined && Number.isNaN(Number(s[k]))) {
        notify(`"${k.replace(/_/g, " ")}" must be a number`, "err");
        return;
      }
    }
    try {
      await saveSettings(s);
      notify("Settings saved");
    } catch (e) { notify(String(e), "err"); }
  }

  async function doReset() {
    setConfirming(false);
    try {
      const msg = await resetDemoData();
      // The reset wipes shifts — refresh the session so the UI doesn't hold a stale one.
      await useSession.getState().refresh();
      notify(msg);
    } catch (e) { notify(String(e), "err"); }
  }

  return (
    <div className="page">
      <div className="page-head">
        <h1>Settings</h1>
        <button className="btn primary slim" onClick={save}>Save settings</button>
      </div>

      <div className="settings-grid">
        <div className="panel">
          <div className="panel-title">Theme</div>
          <div className="theme-row">
            {THEMES.map((t) => (
              <button key={t.id}
                className={`theme-swatch ${t.id} ${s.theme === t.id ? "on" : ""}`}
                onClick={() => { set("theme", t.id); applyTheme(t.id); }}>
                {t.label}
              </button>
            ))}
          </div>
          <p className="hint">Applied instantly; press Save settings to keep it.</p>
        </div>

        <div className="panel">
          <div className="panel-title">Store</div>
          <label className="field">Store name
            <input value={s.store_name} onChange={(e) => set("store_name", e.target.value)} />
          </label>
          <label className="field">Receipt footer message
            <input value={s.receipt_footer} onChange={(e) => set("receipt_footer", e.target.value)} />
          </label>
          <label className="field">Default tax % for new products
            <input value={s.default_tax_pct} inputMode="decimal" onChange={(e) => set("default_tax_pct", e.target.value)} />
          </label>
        </div>

        <div className="panel">
          <div className="panel-title">Loyalty</div>
          <label className="field">Points required for a reward
            <input value={s.loyalty_threshold} inputMode="numeric" onChange={(e) => set("loyalty_threshold", e.target.value)} />
          </label>
          <label className="field">Reward value (cents — 1000 = $10)
            <input value={s.loyalty_reward} inputMode="numeric" onChange={(e) => set("loyalty_reward", e.target.value)} />
          </label>
          <p className="hint">Earning stays 1 point per $1 plus per-product bonus points. These rules are enforced in the Rust backend, not the UI.</p>
        </div>

        <div className="panel">
          <div className="panel-title">Inventory</div>
          <label className="field">Default reorder level for new products
            <input value={s.low_stock_default} inputMode="numeric" onChange={(e) => set("low_stock_default", e.target.value)} />
          </label>
        </div>

        <div className="panel danger-panel">
          <div className="panel-title">Demo mode</div>
          <p className="hint">Wipes all transactions, shifts, customers, movements, and audit history, then seeds a realistic week of sales, five loyalty customers, and product promotions. Products and cashier accounts are kept.</p>
          <button className="btn slim danger-btn" onClick={() => setConfirming(true)}>Reset demo data</button>
        </div>
      </div>

      {confirming && (
        <div className="scrim" onClick={() => setConfirming(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Reset demo data?</h3>
            <p>This permanently deletes all sales, shifts, customers, and audit history and replaces them with seeded demo data. This cannot be undone.</p>
            <div className="modal-actions">
              <button className="btn" onClick={() => setConfirming(false)}>Cancel</button>
              <button className="btn primary" onClick={doReset}>Yes, reset</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
