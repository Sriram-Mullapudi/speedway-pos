//! Device abstraction layer (Phase 12).
//!
//! The register/business logic depends on these traits, never on a concrete
//! printer/drawer implementation. Today only the Simulated adapters exist and
//! are honestly labelled as such; native ESC/POS / serial / HID adapters are a
//! documented future integration point (see `NativeAdapterPlan` doc below) and
//! are intentionally NOT compiled here, since they cannot be built or validated
//! in this environment.
//!
//! Failure isolation: none of these are ever called inside the sale DB
//! transaction. `create_sale` commits first and returns; hardware actions are
//! attempted afterward and their failures are reported separately, never
//! rolling back a committed sale.

use serde::{Deserialize, Serialize};

/// Honest device status — never reports "Connected" for something simulated.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Ready,          // usable now (e.g. keyboard-wedge scanner, simulated printer)
    Simulated,      // works, but is a software stand-in for real hardware
    WindowAvailable,// customer display window can be opened
    NotConfigured,
    Disconnected,
    Testing,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    BarcodeScanner,
    ReceiptPrinter,
    CashDrawer,
    CustomerDisplay,
    InvoicePrinter,
    LabelPrinter,
}

/// A device's configured implementation. Only `Simulated` and input/window
/// modes are real today; `Native*` variants are placeholders describing the
/// future adapter and always resolve to NotConfigured until implemented.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceMode {
    Simulated,
    KeyboardWedge,   // barcode scanner via keyboard input (real, frontend)
    SecondaryWindow, // customer display via Tauri window (real)
    SystemPrint,     // invoice/label via browser/OS print path (real)
    NativeEscpos,    // future: requires native adapter + real-device validation
    Disabled,
}

// ---- Result & error --------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct DeviceResult {
    pub ok: bool,
    pub mode: String,
    pub message: String,
}
impl DeviceResult {
    fn ok(mode: DeviceMode, message: impl Into<String>) -> Self {
        Self { ok: true, mode: format!("{:?}", mode), message: message.into() }
    }
    fn err(mode: DeviceMode, message: impl Into<String>) -> Self {
        Self { ok: false, mode: format!("{:?}", mode), message: message.into() }
    }
}

// ---- Capability traits -----------------------------------------------------

pub trait ReceiptPrinter {
    fn print_receipt(&self, doc: &str) -> DeviceResult;
    fn status(&self) -> DeviceStatus;
}
pub trait CashDrawer {
    fn open(&self, reason: &str) -> DeviceResult;
    fn status(&self) -> DeviceStatus;
}
pub trait DocumentPrinter {
    fn print_document(&self, doc: &str) -> DeviceResult;
    fn status(&self) -> DeviceStatus;
}
pub trait LabelPrinter {
    fn print_labels(&self, doc: &str, count: u32) -> DeviceResult;
    fn status(&self) -> DeviceStatus;
}

// ---- Simulated adapters (genuinely working) --------------------------------

/// Simulated receipt printer. Returns success and echoes the rendered content;
/// a forced-failure flag exists purely so failure isolation can be tested.
pub struct SimulatedReceiptPrinter {
    pub fail: bool,
}
impl ReceiptPrinter for SimulatedReceiptPrinter {
    fn print_receipt(&self, doc: &str) -> DeviceResult {
        if self.fail {
            return DeviceResult::err(DeviceMode::Simulated, "Simulated printer failure");
        }
        DeviceResult::ok(DeviceMode::Simulated, format!("Rendered {} chars", doc.len()))
    }
    fn status(&self) -> DeviceStatus { DeviceStatus::Simulated }
}

pub struct SimulatedCashDrawer {
    pub fail: bool,
}
impl CashDrawer for SimulatedCashDrawer {
    fn open(&self, reason: &str) -> DeviceResult {
        if self.fail {
            return DeviceResult::err(DeviceMode::Simulated, "Simulated drawer failure");
        }
        DeviceResult::ok(DeviceMode::Simulated, format!("Drawer opened: {}", reason))
    }
    fn status(&self) -> DeviceStatus { DeviceStatus::Simulated }
}

// ---- Cash-drawer decision logic (pure, unit-tested) ------------------------

/// The single authoritative rule for whether an event should auto-open the
/// drawer. A future native adapter plugs in *after* this decision; the rule
/// itself never changes with hardware.
///
/// `allow_card_drawer` reflects the (default false) configuration to open the
/// drawer on card-only sales.
pub fn should_auto_open_drawer(event: &str, tender_kind: Option<&str>, allow_card_drawer: bool) -> bool {
    match event {
        "no_sale" | "paid_in" | "paid_out" | "safe_drop" => true,
        "sale" | "refund" => match tender_kind {
            Some("cash") => true,
            Some("card") => allow_card_drawer,
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod drawer_rule_tests {
    use super::*;

    #[test]
    fn cash_sale_opens_drawer() {
        assert!(should_auto_open_drawer("sale", Some("cash"), false));
    }
    #[test]
    fn card_only_sale_does_not_open_by_default() {
        assert!(!should_auto_open_drawer("sale", Some("card"), false));
    }
    #[test]
    fn card_sale_opens_when_configured() {
        assert!(should_auto_open_drawer("sale", Some("card"), true));
    }
    #[test]
    fn cash_refund_opens_drawer() {
        assert!(should_auto_open_drawer("refund", Some("cash"), false));
    }
    #[test]
    fn card_refund_does_not_open_by_default() {
        assert!(!should_auto_open_drawer("refund", Some("card"), false));
    }
    #[test]
    fn drawer_events_always_open() {
        for e in ["no_sale", "paid_in", "paid_out", "safe_drop"] {
            assert!(should_auto_open_drawer(e, None, false), "{} should open", e);
        }
    }
    #[test]
    fn unrelated_events_do_not_open() {
        assert!(!should_auto_open_drawer("void", None, false));
        assert!(!should_auto_open_drawer("login", None, false));
    }
}

#[cfg(test)]
mod simulated_adapter_tests {
    use super::*;

    #[test]
    fn simulated_printer_success() {
        let p = SimulatedReceiptPrinter { fail: false };
        assert!(p.print_receipt("SPEEDWAY MARKET\nTotal $5.00").ok);
    }
    #[test]
    fn simulated_printer_failure_is_reported_not_panicked() {
        let p = SimulatedReceiptPrinter { fail: true };
        let r = p.print_receipt("x");
        assert!(!r.ok);
        assert!(r.message.contains("failure"));
    }
    #[test]
    fn simulated_drawer_open_ok() {
        let d = SimulatedCashDrawer { fail: false };
        assert!(d.open("cash sale").ok);
    }
}
