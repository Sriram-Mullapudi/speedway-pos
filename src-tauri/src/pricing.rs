//! Pure pricing helpers — no database, no I/O, fully unit-testable.
//! All money is integer cents. This is the kind of logic an interviewer
//! will ask you to walk through, so it lives on its own and is tested.

pub fn line_tax(line_total: i64, tax_rate: f64) -> i64 {
    (line_total as f64 * tax_rate).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_tax_to_nearest_cent() {
        // 999c @ 9% = 89.91c -> 90c
        assert_eq!(line_tax(999, 0.09), 90);
    }

    #[test]
    fn zero_tax_is_zero() {
        assert_eq!(line_tax(1000, 0.0), 0);
    }
}

/// Promotion-aware line total.
/// - "bogo": every 2nd unit is free (buy one, get one).
/// - "second_pct": every 2nd unit is discounted by `promo_value` percent.
/// - anything else: plain price × qty.
pub fn promo_line_total(unit_price: i64, qty: i64, promo_type: &str, promo_value: i64) -> i64 {
    match promo_type {
        "bogo" => {
            let free = qty / 2;
            unit_price * (qty - free)
        }
        "second_pct" => {
            let discounted_units = qty / 2;
            let full_units = qty - discounted_units;
            let discounted_price = unit_price * (100 - promo_value.clamp(0, 100)) / 100;
            unit_price * full_units + discounted_price * discounted_units
        }
        _ => unit_price * qty,
    }
}

#[cfg(test)]
mod promo_tests {
    use super::*;

    #[test]
    fn bogo_second_is_free() {
        assert_eq!(promo_line_total(500, 2, "bogo", 0), 500);   // pay 1 of 2
        assert_eq!(promo_line_total(500, 3, "bogo", 0), 1000);  // pay 2 of 3
        assert_eq!(promo_line_total(500, 1, "bogo", 0), 500);   // no pair yet
    }

    #[test]
    fn second_unit_percent_off() {
        // $5.00 + $3.50 (30% off the 2nd)
        assert_eq!(promo_line_total(500, 2, "second_pct", 30), 850);
        // 3 units: two full + one discounted
        assert_eq!(promo_line_total(500, 3, "second_pct", 30), 1350);
    }

    #[test]
    fn none_is_plain_multiplication() {
        assert_eq!(promo_line_total(249, 3, "none", 0), 747);
    }
}

/// Loyalty redemption: if the customer has at least `threshold` points and
/// asked to redeem, consume `threshold` points for a `reward`-cent discount,
/// capped at the sale total. Returns (discount, points_consumed).
pub fn loyalty_redemption(points: i64, redeem: bool, threshold: i64, reward: i64, gross: i64) -> Result<(i64, i64), String> {
    if !redeem {
        return Ok((0, 0));
    }
    if points < threshold {
        return Err(format!("Not enough points to redeem — {} required", threshold));
    }
    Ok((reward.min(gross), threshold))
}

/// Points earned: 1 point per whole dollar of the (post-discount) total,
/// plus any per-product bonus points accumulated on the sale.
pub fn loyalty_earned(total: i64, bonus_points: i64) -> i64 {
    total / 100 + bonus_points
}

#[cfg(test)]
mod loyalty_tests {
    use super::*;

    #[test]
    fn no_redeem_no_discount() {
        assert_eq!(loyalty_redemption(9999, false, 500, 1000, 5000).unwrap(), (0, 0));
    }

    #[test]
    fn redeem_at_threshold_gives_reward() {
        assert_eq!(loyalty_redemption(500, true, 500, 1000, 5000).unwrap(), (1000, 500));
    }

    #[test]
    fn reward_capped_at_sale_total() {
        assert_eq!(loyalty_redemption(700, true, 500, 1000, 600).unwrap(), (600, 500));
    }

    #[test]
    fn under_threshold_is_rejected() {
        assert!(loyalty_redemption(499, true, 500, 1000, 5000).is_err());
    }

    #[test]
    fn earn_one_point_per_dollar_plus_bonus() {
        assert_eq!(loyalty_earned(2599, 0), 25);
        assert_eq!(loyalty_earned(2599, 10), 35);
        assert_eq!(loyalty_earned(99, 0), 0);
    }
}
