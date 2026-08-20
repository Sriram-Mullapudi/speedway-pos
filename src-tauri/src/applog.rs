//! Minimal append-only application logger (Phase 15). No external framework —
//! a plain file with size-based rotation. Logging never panics and never blocks
//! checkout: every failure is swallowed. Never logs secrets.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

const MAX_BYTES: u64 = 512 * 1024; // rotate at ~512KB

fn log_path(dir: &PathBuf) -> PathBuf { dir.join("logs").join("speedway.log") }

/// Append a line. Best-effort: any error is ignored so logging can never crash
/// the register.
pub fn log(app_data_dir: &PathBuf, level: &str, msg: &str) {
    let _ = try_log(app_data_dir, level, msg);
}

fn try_log(app_data_dir: &PathBuf, level: &str, msg: &str) -> std::io::Result<()> {
    let logs = app_data_dir.join("logs");
    fs::create_dir_all(&logs)?;
    let path = log_path(app_data_dir);

    // Rotate if oversized: speedway.log -> speedway.log.1 (single generation).
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_BYTES {
            let _ = fs::rename(&path, logs.join("speedway.log.1"));
        }
    }

    let ts = now_iso();
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    // Sanitize: strip anything that looks like it could be a secret key=value.
    writeln!(f, "{} [{}] {}", ts, level, sanitize(msg))?;
    Ok(())
}

/// Return recent log lines (sanitized already at write time), newest last.
pub fn tail(app_data_dir: &PathBuf, max_lines: usize) -> Vec<String> {
    let path = log_path(app_data_dir);
    match fs::read_to_string(&path) {
        Ok(s) => {
            let lines: Vec<&str> = s.lines().collect();
            let start = lines.len().saturating_sub(max_lines);
            lines[start..].iter().map(|l| l.to_string()).collect()
        }
        Err(_) => vec![],
    }
}

fn sanitize(msg: &str) -> String {
    // Defense in depth: redact obvious secret-looking tokens.
    let lower = msg.to_lowercase();
    if lower.contains("pin") || lower.contains("hash") || lower.contains("secret")
        || lower.contains("password") || lower.contains("token") {
        return "[redacted: potential sensitive content]".to_string();
    }
    msg.to_string()
}

fn now_iso() -> String {
    // Avoid a chrono dependency: use SQLite-free UTC seconds formatting.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("t={}", secs)
}
