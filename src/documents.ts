import type { Receipt } from "./types";

/** Money helper mirroring api.money for standalone document strings. */
function m(cents: number): string {
  const s = cents < 0 ? "-" : "";
  const a = Math.abs(cents);
  return `${s}$${Math.floor(a / 100)}.${String(a % 100).padStart(2, "0")}`;
}

export interface InvoiceMeta {
  store: string; address: string; phone: string; taxId: string;
  invoiceNo: string; customer: string; cashier: string; notes: string;
  size: "A4" | "A5";
}

/** Professional A4/A5 business-document invoice (distinct from receipt style).
 *  Rendered as a standalone HTML string for the browser/OS print path. */
export function invoiceHtml(receipt: Receipt, meta: InvoiceMeta): string {
  const rows = receipt.items.map((it) => `
    <tr>
      <td>${escapeHtml(it.name)}</td>
      <td class="num">${it.qty}</td>
      <td class="num">${m(it.unit_price)}</td>
      <td class="num">${m(it.line_total)}</td>
    </tr>`).join("");
  const pageCss = meta.size === "A4" ? "size: A4;" : "size: A5;";
  return `<!doctype html><html><head><meta charset="utf-8"><title>Invoice ${escapeHtml(meta.invoiceNo)}</title>
  <style>
    @page { ${pageCss} margin: 16mm; }
    body { font-family: system-ui, Arial, sans-serif; color: #1a1a1a; font-size: 12px; }
    .head { display: flex; justify-content: space-between; border-bottom: 2px solid #222; padding-bottom: 12px; }
    .biz h1 { margin: 0 0 4px; font-size: 20px; }
    .muted { color: #666; }
    .meta { margin: 16px 0; display: flex; justify-content: space-between; }
    table { width: 100%; border-collapse: collapse; margin-top: 12px; }
    th, td { text-align: left; padding: 7px 8px; border-bottom: 1px solid #ddd; }
    th { background: #f4f4f4; font-size: 11px; text-transform: uppercase; letter-spacing: .04em; }
    .num { text-align: right; }
    .totals { margin-top: 14px; margin-left: auto; width: 240px; }
    .totals .r { display: flex; justify-content: space-between; padding: 4px 0; }
    .totals .grand { font-size: 16px; font-weight: 700; border-top: 2px solid #222; margin-top: 6px; padding-top: 8px; }
    .notes { margin-top: 24px; color: #555; }
  </style></head><body>
    <div class="head">
      <div class="biz">
        <h1>${escapeHtml(meta.store)}</h1>
        <div class="muted">${escapeHtml(meta.address)}</div>
        <div class="muted">${escapeHtml(meta.phone)}</div>
        ${meta.taxId ? `<div class="muted">Tax ID: ${escapeHtml(meta.taxId)}</div>` : ""}
      </div>
      <div class="inv">
        <h1>INVOICE</h1>
        <div class="muted">No: ${escapeHtml(meta.invoiceNo)}</div>
        <div class="muted">Ref sale: #${receipt.id}</div>
        <div class="muted">${escapeHtml(receipt.created_at)}</div>
      </div>
    </div>
    <div class="meta">
      <div><strong>Bill to:</strong> ${escapeHtml(meta.customer || "Walk-in customer")}</div>
      <div><strong>Cashier:</strong> ${escapeHtml(meta.cashier || receipt.cashier)}</div>
    </div>
    <table>
      <thead><tr><th>Description</th><th class="num">Qty</th><th class="num">Unit</th><th class="num">Amount</th></tr></thead>
      <tbody>${rows}</tbody>
    </table>
    <div class="totals">
      <div class="r"><span>Subtotal</span><span>${m(receipt.subtotal)}</span></div>
      ${receipt.discount > 0 ? `<div class="r"><span>Discount</span><span>-${m(receipt.discount)}</span></div>` : ""}
      <div class="r"><span>Tax</span><span>${m(receipt.tax)}</span></div>
      <div class="r grand"><span>Total</span><span>${m(receipt.total)}</span></div>
      <div class="r"><span>Payment</span><span>${escapeHtml(receipt.tender_kind)}</span></div>
    </div>
    ${meta.notes ? `<div class="notes"><strong>Notes:</strong> ${escapeHtml(meta.notes)}</div>` : ""}
  </body></html>`;
}

export interface LabelSpec {
  name: string; barcode: string; price: number; count: number;
}

/** Product price/barcode label sheet (repeated tiles) for the system print path. */
export function labelHtml(label: LabelSpec): string {
  const tile = `
    <div class="label">
      <div class="l-name">${escapeHtml(label.name)}</div>
      <div class="l-price">${m(label.price)}</div>
      <div class="l-code">*${escapeHtml(label.barcode)}*</div>
      <div class="l-num">${escapeHtml(label.barcode)}</div>
    </div>`;
  const tiles = Array.from({ length: Math.max(1, Math.min(60, label.count)) }, () => tile).join("");
  return `<!doctype html><html><head><meta charset="utf-8"><title>Labels</title>
  <style>
    @page { margin: 8mm; }
    body { font-family: system-ui, Arial, sans-serif; }
    .sheet { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6mm; }
    .label { border: 1px solid #bbb; border-radius: 4px; padding: 8px; text-align: center; }
    .l-name { font-size: 11px; font-weight: 600; }
    .l-price { font-size: 20px; font-weight: 800; margin: 4px 0; }
    .l-code { font-family: "Libre Barcode 39", monospace; font-size: 22px; letter-spacing: 1px; }
    .l-num { font-size: 10px; letter-spacing: 2px; }
  </style></head><body><div class="sheet">${tiles}</div></body></html>`;
}

/** Open an HTML document in a print window (browser/OS print path → also "Save as PDF"). */
export function printHtml(html: string) {
  const w = window.open("", "_blank", "width=800,height=1000");
  if (!w) return false;
  w.document.write(html);
  w.document.close();
  w.focus();
  setTimeout(() => w.print(), 300);
  return true;
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
}
