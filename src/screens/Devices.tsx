import { useEffect, useState } from "react";
import { listDevices, printTestReceipt, manualOpenDrawer, type DeviceInfo } from "../api";
import { notify } from "../components/Toast";
import { invoiceHtml, labelHtml, printHtml } from "../documents";
import type { Receipt } from "../types";

const STATUS_LABEL: Record<string, string> = {
  ready: "Ready", simulated: "Simulated", window_available: "Window available",
  not_configured: "Not configured", disconnected: "Disconnected", error: "Error",
};

const MODE_LABEL: Record<string, string> = {
  keyboard_wedge: "Keyboard input", simulated: "Simulated", secondary_window: "Secondary window",
  system_print: "System print", native_escpos: "Native ESC/POS (future)",
};

export default function Devices() {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);

  useEffect(() => { listDevices().then(setDevices).catch(console.error); }, []);

  function openCustomerDisplay() {
    const w = window.open(`${window.location.pathname}?display=customer`, "speedway-customer",
      "width=900,height=600");
    if (!w) notify("Popup blocked — allow popups to open the customer display", "err");
    else notify("Customer display opened");
  }

  async function testReceipt() {
    try {
      const r = await printTestReceipt();
      notify(r.ok ? "Test receipt printed (simulated)" : `Printer error: ${r.message}`, r.ok ? "ok" : "err");
    } catch (e) { notify(String(e), "err"); }
  }

  async function testDrawer() {
    try {
      const r = await manualOpenDrawer("Device test", null);
      notify(r.ok ? "Drawer opened (simulated)" : `Drawer error: ${r.message}`, r.ok ? "ok" : "err");
    } catch (e) { notify(String(e), "err"); }
  }

  function testInvoice(size: "A4" | "A5") {
    const demo: Receipt = {
      id: 0, store_name: "Speedway Market", footer: "", cashier: "Manager",
      created_at: new Date().toISOString().slice(0, 16).replace("T", " "),
      subtotal: 1298, tax: 91, discount: 0, total: 1389, tender_kind: "card",
      tendered: 1389, change: 0, points_earned: 0, points_redeemed: 0, points_balance: null,
      items: [
        { name: "Coca-Cola 20oz", qty: 2, unit_price: 249, line_total: 498 },
        { name: "Chips", qty: 1, unit_price: 800, line_total: 800 },
      ],
    };
    const ok = printHtml(invoiceHtml(demo, {
      store: "Speedway Market", address: "123 Main St, Tampa, FL", phone: "(813) 555-0100",
      taxId: "", invoiceNo: "INV-TEST", customer: "Walk-in customer", cashier: "Manager",
      notes: "Test invoice preview.", size,
    }));
    if (!ok) notify("Popup blocked — allow popups to preview the invoice", "err");
  }

  function testLabel() {
    const ok = printHtml(labelHtml({ name: "Coca-Cola 20oz", barcode: "049000000443", price: 249, count: 6 }));
    if (!ok) notify("Popup blocked — allow popups to preview labels", "err");
  }

  return (
    <div className="page">
      <div className="page-head"><h1>Devices</h1></div>
      <p className="hint">Speedway POS talks to hardware through a device abstraction. Today the receipt printer and cash drawer run in <strong>simulated</strong> mode; the barcode scanner uses ordinary keyboard-wedge input; the customer display and document printing use real window/system-print paths. Native ESC/POS, serial, and HID adapters are a future integration and require real-device validation.</p>

      <div className="device-list">
        {devices.map((d) => (
          <div className="device-card" key={d.kind}>
            <div className="dev-main">
              <div className="dev-label">{d.label}</div>
              <div className="dev-meta">
                <span className="pill sim">{STATUS_LABEL[d.status] ?? d.status}</span>
                <span className="dev-mode">{MODE_LABEL[d.mode] ?? d.mode}</span>
              </div>
            </div>
            <div className="dev-actions">
              {d.kind === "receipt_printer" && <button className="btn slim" onClick={testReceipt}>Print test receipt</button>}
              {d.kind === "cash_drawer" && <button className="btn slim" onClick={testDrawer}>Test drawer</button>}
              {d.kind === "customer_display" && <button className="btn slim" onClick={openCustomerDisplay}>Open display</button>}
              {d.kind === "invoice_printer" && (
                <>
                  <button className="btn slim" onClick={() => testInvoice("A4")}>Preview A4</button>
                  <button className="btn slim" onClick={() => testInvoice("A5")}>Preview A5</button>
                </>
              )}
              {d.kind === "label_printer" && <button className="btn slim" onClick={testLabel}>Preview labels</button>}
              {d.kind === "barcode_scanner" && <span className="dev-note">Scan into the Register — no setup needed</span>}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
