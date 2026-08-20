import { useEffect, useState } from "react";
import Register from "./screens/Register";
import Catalog from "./screens/Catalog";
import Inventory from "./screens/Inventory";
import Cashiers from "./screens/Cashiers";
import ShiftView from "./screens/ShiftView";
import Reports from "./screens/Reports";
import TouchscreenConfig from "./screens/TouchscreenConfig";
import Transactions from "./screens/Transactions";
import Customers from "./screens/Customers";
import Settings from "./screens/Settings";
import AuditLog from "./screens/AuditLog";
import Devices from "./screens/Devices";
import Purchasing from "./screens/Purchasing";
import SystemHealth from "./screens/SystemHealth";
import BackupRecovery from "./screens/BackupRecovery";
import HelpSupport from "./screens/HelpSupport";
import Registers from "./screens/Registers";
import { Toasts, notify } from "./components/Toast";
import SelfCheckout from "./screens/SelfCheckout";
import { applyTheme } from "./theme";
import { getSettings } from "./api";
import LockScreen from "./screens/LockScreen";
import { DrawerModal, OpenShiftModal } from "./components/ShiftModals";
import { useSession } from "./sessionStore";

type View = "register" | "catalog" | "inventory" | "shift" | "reports" | "transactions" | "customers" | "touchscreen" | "cashiers" | "settings" | "audit" | "devices" | "purchasing" | "health" | "backup" | "help" | "registers";

export default function App() {
  const { session, activeShift, ready, refresh, logout, isManager } = useSession();
  const [view, setView] = useState<View>("register");
  const [drawer, setDrawer] = useState(false);
  const [openShift, setOpenShift] = useState(false);
  const [kiosk, setKiosk] = useState(false);

  useEffect(() => { refresh().catch(console.error); }, [refresh]);
  useEffect(() => { getSettings().then((m) => applyTheme(m.theme)).catch(() => applyTheme(undefined)); }, []);

  // Prompt to open a shift right after login when none is active.
  useEffect(() => {
    if (session && !activeShift) setOpenShift(true);
  }, [session, activeShift]);

  if (!ready) return <div className="lock"><div className="lock-card">Loading…</div></div>;
  if (!session) return <LockScreen />;
  if (kiosk) return (<><SelfCheckout onExit={() => setKiosk(false)} /><Toasts /></>);

  const tabs: { id: View; label: string }[] = [
    { id: "register", label: "Register" },
    { id: "catalog", label: "Products" },
    { id: "inventory", label: "Inventory" },
    { id: "shift", label: "Shift" },
    { id: "reports", label: "Reports" },
    { id: "transactions", label: "Transactions" },
    { id: "customers", label: "Customers" },
    ...(isManager() ? [{ id: "touchscreen" as View, label: "Touchscreen" }, { id: "cashiers" as View, label: "Cashiers" }, { id: "audit" as View, label: "Audit" }, { id: "purchasing" as View, label: "Purchasing" }, { id: "registers" as View, label: "Registers" }, { id: "devices" as View, label: "Devices" }, { id: "health" as View, label: "System Health" }, { id: "backup" as View, label: "Backup" }, { id: "help" as View, label: "Help" }, { id: "settings" as View, label: "Settings" }] : []),
  ];

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <b>Speedway Market</b>
          <span>Register 1</span>
        </div>
        <nav className="nav">
          {tabs.map((t) => (
            <button key={t.id} className={`navbtn ${view === t.id ? "on" : ""}`} onClick={() => setView(t.id)}>
              {t.label}
            </button>
          ))}
        </nav>
        <div className="session-actions">
          <span className="chip">
            {session.name}<span className="chip-role">{session.role}</span>
          </span>
          {!activeShift && <button className="navbtn warn" onClick={() => setOpenShift(true)}>Open shift</button>}
          <button className="navbtn" onClick={() => { if (!activeShift) { notify("Open a shift before starting the kiosk", "err"); return; } setKiosk(true); }}>Kiosk</button>
          <button className="navbtn" onClick={() => setDrawer(true)}>Drawer</button>
          <button className="navbtn" onClick={() => { logout(); setView("register"); }}>Lock</button>
        </div>
      </header>

      <main className="content">
        {view === "register" && <Register onNav={(v) => setView(v as View)} />}
        {view === "catalog" && <Catalog />}
        {view === "inventory" && <Inventory />}
        {view === "shift" && <ShiftView />}
        {view === "reports" && <Reports />}
        {view === "touchscreen" && <TouchscreenConfig />}
        {view === "transactions" && <Transactions />}
        {view === "customers" && <Customers />}
        {view === "settings" && <Settings />}
        {view === "audit" && <AuditLog />}
        {view === "devices" && <Devices />}
        {view === "purchasing" && <Purchasing />}
        {view === "health" && <SystemHealth />}
        {view === "backup" && <BackupRecovery />}
        {view === "help" && <HelpSupport />}
        {view === "registers" && <Registers />}
        {view === "cashiers" && <Cashiers />}
      </main>

      {drawer && <DrawerModal onClose={() => setDrawer(false)} />}
      {openShift && <OpenShiftModal onClose={() => setOpenShift(false)} />}
      <Toasts />
    </div>
  );
}
