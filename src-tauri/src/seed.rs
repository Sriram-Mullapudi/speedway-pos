use sqlx::SqlitePool;

/// Seed a realistic convenience/liquor-store catalog on first run so the app
/// looks like a real store in screenshots. No-op if products already exist.
pub async fn seed_if_empty(pool: &SqlitePool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }

    // Cashiers are seeded by seed_cashiers() with real argon2-hashed PINs.

    sqlx::query(
        "INSERT INTO categories (name) VALUES \
         ('Beverages'),('Beer & Wine'),('Spirits'),('Tobacco'),('Snacks'),('Grocery')",
    )
    .execute(pool)
    .await?;

    // sku, name, category_id, price¢, cost¢, tax_rate, age_restricted
    let products: Vec<(&str, &str, i64, i64, i64, f64, i64)> = vec![
        ("0001", "Coca-Cola 20oz",            1, 249, 120, 0.07, 0),
        ("0002", "Red Bull 8.4oz",            1, 329, 165, 0.07, 0),
        ("0003", "Spring Water 1L",           1,  159,  55, 0.07, 0),
        ("0004", "Gatorade Cool Blue 28oz",   1,  219, 110, 0.07, 0),
        ("0005", "Budweiser 6-pack",          2,  999, 620, 0.09, 1),
        ("0006", "Modelo Especial 12-pack",   2, 1899,1180, 0.09, 1),
        ("0007", "Barefoot Pinot Grigio 750ml",2, 899, 520, 0.09, 1),
        ("0008", "Tito's Vodka 750ml",        3, 2199,1450, 0.09, 1),
        ("0009", "Jack Daniel's 750ml",       3, 2899,1980, 0.09, 1),
        ("0010", "Marlboro Red Pack",         4,  899, 640, 0.12, 1),
        ("0011", "Camel Blue Pack",           4,  879, 625, 0.12, 1),
        ("0012", "Doritos Nacho 9.25oz",      5,  549, 280, 0.00, 0),
        ("0013", "Snickers Bar",              5,  189,  85, 0.00, 0),
        ("0014", "Lay's Classic 8oz",         5,  499, 250, 0.00, 0),
        ("0015", "Wonder Bread Loaf",         6,  329, 150, 0.00, 0),
        ("0016", "Large Eggs Dozen",          6,  399, 210, 0.00, 0),
    ];

    for (sku, name, cat, price, cost, tax_rate, age) in products {
        let res = sqlx::query(
            "INSERT INTO products \
             (sku, name, category_id, price, cost, tax_rate, age_restricted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(sku)
        .bind(name)
        .bind(cat)
        .bind(price)
        .bind(cost)
        .bind(tax_rate)
        .bind(age)
        .execute(pool)
        .await?;

        sqlx::query(
            "INSERT INTO inventory (product_id, quantity_on_hand, reorder_level) \
             VALUES (?1, ?2, ?3)",
        )
        .bind(res.last_insert_rowid())
        .bind(48)
        .bind(12)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Ensure the three default accounts exist with real argon2-hashed PINs.
/// Idempotent: only runs while no admin exists.
pub async fn seed_cashiers(pool: &SqlitePool) -> anyhow::Result<()> {
    let admins: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cashiers WHERE role = 'admin'")
        .fetch_one(pool).await?;
    if admins > 0 {
        return Ok(());
    }
    let defaults = [("admin", "Admin", "1234"), ("manager", "Manager", "2222"), ("cashier", "Cashier", "1111")];
    for (role, name, pin) in defaults {
        let hash = crate::security::hash_pin(pin).map_err(|e| anyhow::anyhow!(e))?;
        let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM cashiers WHERE role = ?1 LIMIT 1")
            .bind(role).fetch_optional(pool).await?;
        match existing {
            Some(id) => {
                sqlx::query("UPDATE cashiers SET name = ?1, pin_hash = ?2, active = 1, updated_at = datetime('now') WHERE id = ?3")
                    .bind(name).bind(&hash).bind(id).execute(pool).await?;
            }
            None => {
                sqlx::query("INSERT INTO cashiers (name, role, pin_hash) VALUES (?1, ?2, ?3)")
                    .bind(name).bind(role).bind(&hash).execute(pool).await?;
            }
        }
    }
    Ok(())
}

/// Expand the catalog with real convenience-store departments and items:
/// lottery, fountain, coffee, food, propane, and the water department —
/// plus one open-price item per department for manual price entry.
/// Idempotent: INSERT OR IGNORE keyed on unique names/SKUs, safe on every boot.
pub async fn seed_convenience_catalog(pool: &SqlitePool) -> anyhow::Result<()> {
    for name in ["Lottery", "Fountain", "Coffee", "Food", "Propane", "Water", "Ice", "Household", "Automotive"] {
        sqlx::query("INSERT OR IGNORE INTO categories (name) VALUES (?1)")
            .bind(name)
            .execute(pool)
            .await?;
    }

    async fn cat_id(pool: &SqlitePool, name: &str) -> anyhow::Result<i64> {
        Ok(sqlx::query_scalar("SELECT id FROM categories WHERE name = ?1")
            .bind(name)
            .fetch_one(pool)
            .await?)
    }
    let lot = cat_id(pool, "Lottery").await?;
    let fon = cat_id(pool, "Fountain").await?;
    let cof = cat_id(pool, "Coffee").await?;
    let food = cat_id(pool, "Food").await?;
    let pro = cat_id(pool, "Propane").await?;
    let wat = cat_id(pool, "Water").await?;
    let ice = cat_id(pool, "Ice").await?;
    let hh = cat_id(pool, "Household").await?;
    let auto = cat_id(pool, "Automotive").await?;

    // (sku, name, cat, price¢, cost¢, tax, age, open)
    let items: Vec<(&str, String, i64, i64, i64, f64, i64, i64)> = vec![
        // Lottery — tax-free, sold at face value
        ("L001", "Powerball $2".into(), lot, 200, 200, 0.0, 1, 0),
        ("L002", "Mega Millions $2".into(), lot, 200, 200, 0.0, 1, 0),
        ("L003", "FL Lotto $2".into(), lot, 200, 200, 0.0, 1, 0),
        ("L004", "Scratch-Off $1".into(), lot, 100, 100, 0.0, 1, 0),
        ("L005", "Scratch-Off $2".into(), lot, 200, 200, 0.0, 1, 0),
        ("L006", "Scratch-Off $5".into(), lot, 500, 500, 0.0, 1, 0),
        ("L007", "Scratch-Off $10".into(), lot, 1000, 1000, 0.0, 1, 0),
        ("L008", "Scratch-Off $20".into(), lot, 2000, 2000, 0.0, 1, 0),
        ("L009", "Scratch-Off $30".into(), lot, 3000, 3000, 0.0, 1, 0),
        // Fountain
        ("F001", "Fountain Small".into(), fon, 119, 22, 0.07, 0, 0),
        ("F002", "Fountain Medium".into(), fon, 149, 28, 0.07, 0, 0),
        ("F003", "Fountain Large".into(), fon, 179, 34, 0.07, 0, 0),
        // Coffee
        ("C001", "Coffee Small".into(), cof, 129, 24, 0.07, 0, 0),
        ("C002", "Coffee Medium".into(), cof, 159, 30, 0.07, 0, 0),
        ("C003", "Coffee Large".into(), cof, 189, 36, 0.07, 0, 0),
        // Food
        ("FD01", "Hot Dog".into(), food, 199, 70, 0.07, 0, 0),
        ("FD02", "Pizza Slice".into(), food, 249, 95, 0.07, 0, 0),
        ("FD03", "Taquito".into(), food, 179, 62, 0.07, 0, 0),
        ("FD04", "Nachos".into(), food, 349, 120, 0.07, 0, 0),
        ("FD05", "Breakfast Sandwich".into(), food, 399, 160, 0.07, 0, 0),
        // Propane
        ("P001", "Propane Exchange".into(), pro, 2199, 1450, 0.07, 0, 0),
        ("P002", "Propane New Tank".into(), pro, 5499, 3900, 0.07, 0, 0),
        // Water department
        ("W001", "PureLife 40-Pack".into(), wat, 549, 380, 0.07, 0, 0),
        ("W002", "Zephyrhills 40-Pack".into(), wat, 699, 490, 0.07, 0, 0),
        ("W003", "Dasani 35-Pack".into(), wat, 649, 460, 0.07, 0, 0),
        ("W004", "Zephyrhills 1 Gallon".into(), wat, 199, 110, 0.07, 0, 0),
        ("W005", "PureLife 1 Gallon".into(), wat, 189, 105, 0.07, 0, 0),
        ("W006", "Distilled Water 1 Gal".into(), wat, 159, 85, 0.07, 0, 0),
        ("W007", "Sparkling Water".into(), wat, 149, 70, 0.07, 0, 0),
        // Fountain XL + hot drinks
        ("F004", "Fountain XL".into(), fon, 199, 40, 0.07, 0, 0),
        ("C004", "Hot Chocolate".into(), cof, 179, 38, 0.07, 0, 0),
        ("C005", "Hot Tea".into(), cof, 149, 25, 0.07, 0, 0),
        // Ice
        ("I001", "Ice 10 lb".into(), ice, 249, 120, 0.07, 0, 0),
        ("I002", "Ice 20 lb".into(), ice, 399, 200, 0.07, 0, 0),
        // More food
        ("FD06", "Chicken Tenders".into(), food, 499, 210, 0.07, 0, 0),
        ("FD07", "Deli Sandwich".into(), food, 449, 190, 0.07, 0, 0),
        ("FD08", "Donut".into(), food, 129, 40, 0.07, 0, 0),
        ("FD09", "Ice Cream Bar".into(), food, 229, 90, 0.07, 0, 0),
        // Household
        ("H001", "Paper Towels".into(), hh, 299, 170, 0.07, 0, 0),
        ("H002", "Toilet Paper 4-Pack".into(), hh, 349, 200, 0.07, 0, 0),
        ("H003", "Cleaning Spray".into(), hh, 399, 220, 0.07, 0, 0),
        // Automotive
        ("A001", "Motor Oil 1 Qt".into(), auto, 699, 420, 0.07, 0, 0),
        ("A002", "Phone Charger".into(), auto, 999, 380, 0.07, 0, 0),
    ];

    // one open-price item per department + generic misc
    let cats: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM categories ORDER BY id")
        .fetch_all(pool)
        .await?;
    let mut all = items;
    for (cid, cname) in &cats {
        all.push((
            Box::leak(format!("OPEN-{}", cid).into_boxed_str()),
            format!("{} — Open Price", cname),
            *cid, 0, 0, 0.07, 0, 1,
        ));
    }

    for (sku, name, cat, price, cost, tax, age, open) in all {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO products \
             (sku, name, category_id, price, cost, tax_rate, age_restricted, open_price) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(sku).bind(&name).bind(cat).bind(price).bind(cost).bind(tax).bind(age).bind(open)
        .execute(pool)
        .await?;
        if res.rows_affected() > 0 {
            sqlx::query(
                "INSERT OR IGNORE INTO inventory (product_id, quantity_on_hand, reorder_level) \
                 VALUES (?1, 500, 0)",
            )
            .bind(res.last_insert_rowid())
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}
