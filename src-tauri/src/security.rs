//! Pure security primitives: PIN hashing/verification (argon2) and the
//! role/permission matrix. No DB, no I/O — fully unit-testable.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;

pub fn hash_pin(pin: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub fn verify_pin(pin: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(pin.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

pub const MANAGER_ONLY: &[&str] = &[
    "void", "refund", "no_sale", "paid_out", "price_override",
    "shift_close_override", "product_delete", "settings", "manage_cashiers",
    "open_drawer", "configure_devices",
    "backup", "restore", "diagnostics",
];

pub fn role_can(role: &str, action: &str) -> bool {
    if role == "admin" {
        return true;
    }
    if MANAGER_ONLY.contains(&action) {
        return role == "manager";
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_roundtrips() {
        let h = hash_pin("1234").unwrap();
        assert!(verify_pin("1234", &h));
        assert!(!verify_pin("0000", &h));
    }
    #[test]
    fn pin_is_never_stored_in_plaintext() {
        let h = hash_pin("1234").unwrap();
        assert!(!h.contains("1234"));
    }
    #[test]
    fn admin_can_do_anything() {
        assert!(role_can("admin", "void"));
        assert!(role_can("admin", "settings"));
    }
    #[test]
    fn cashier_blocked_from_manager_only_actions() {
        assert!(!role_can("cashier", "void"));
        assert!(!role_can("cashier", "manage_cashiers"));
    }
    #[test]
    fn cashier_allowed_base_actions() {
        assert!(role_can("cashier", "ring_sale"));
    }
    #[test]
    fn manager_allowed_manager_actions() {
        assert!(role_can("manager", "void"));
        assert!(role_can("manager", "refund"));
    }
}
