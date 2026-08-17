import { useEffect, useRef } from "react";

/**
 * Keyboard-wedge barcode scanner support.
 *
 * USB barcode scanners type like a keyboard: a fast burst of characters ending
 * in a suffix (Enter by default). We distinguish that from ordinary human
 * typing by inter-keystroke timing: scanner characters arrive much faster than
 * a person types. The buffer is assembled globally (document-level) so it works
 * regardless of focus, but we never swallow ordinary typing — if keystrokes are
 * slow, the buffer resets and normal input is untouched.
 *
 * The timing/threshold logic is isolated in `feedKey` so it is unit-testable
 * without a DOM.
 */

export interface ScannerState {
  buffer: string;
  lastTime: number;
}

export interface ScannerConfig {
  maxIntervalMs: number; // max gap between scanner chars (human typing is slower)
  minLength: number;     // ignore very short bursts (stray fast keys)
}

export const DEFAULT_SCANNER: ScannerConfig = { maxIntervalMs: 35, minLength: 3 };

/**
 * Pure reducer for one keypress. Returns the completed barcode when the suffix
 * (Enter) arrives after a qualifying fast burst, otherwise null. Exported for
 * unit testing.
 */
export function feedKey(
  state: ScannerState,
  key: string,
  now: number,
  cfg: ScannerConfig = DEFAULT_SCANNER
): { state: ScannerState; barcode: string | null } {
  const gap = now - state.lastTime;

  if (key === "Enter") {
    // Only treat as a scan if the accumulated buffer is long enough AND was
    // assembled as a fast burst. Otherwise it's a normal Enter — pass through.
    if (state.buffer.length >= cfg.minLength) {
      const barcode = state.buffer;
      return { state: { buffer: "", lastTime: now }, barcode };
    }
    return { state: { buffer: "", lastTime: now }, barcode: null };
  }

  // Single printable character only; ignore control keys.
  if (key.length !== 1) {
    return { state, barcode: null };
  }

  // If the gap is too large, this is human typing → start a fresh buffer.
  if (gap > cfg.maxIntervalMs) {
    return { state: { buffer: key, lastTime: now }, barcode: null };
  }
  // Fast burst continues.
  return { state: { buffer: state.buffer + key, lastTime: now }, barcode: null };
}

/**
 * React hook: calls `onScan(barcode)` when a scanner burst completes.
 * Ordinary typing and F-keys are never intercepted (we only act on the
 * completed fast-burst + Enter, and we do not preventDefault on normal keys).
 */
export function useScanner(onScan: (barcode: string) => void, cfg: ScannerConfig = DEFAULT_SCANNER) {
  const ref = useRef<ScannerState>({ buffer: "", lastTime: 0 });
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      // Never interfere with shortcut keys.
      if (e.key.startsWith("F") && e.key.length <= 3) return;
      const { state, barcode } = feedKey(ref.current, e.key, performance.now(), cfg);
      ref.current = state;
      if (barcode) {
        // A completed scan: consume the Enter so it doesn't double-submit a form.
        e.preventDefault();
        onScan(barcode);
      }
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onScan, cfg]);
}
