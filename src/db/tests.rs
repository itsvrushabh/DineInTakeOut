use super::Database;
use crate::models::{
    Area, CartLine, MenuItem, Order, OrderStatus, PhysicalTable, Service, TableStatus,
};
use chrono::Local;

fn sample_order(id: u32) -> Order {
    Order {
        id,
        label: "Main-T1".to_string(),
        service: Service::DineIn,
        table_number: Some(1),
        area: Some("Main Hall".to_string()),
        is_ac: false,
        ac_rate: 0.0,
        discount_percent: 0.0,
        cart: vec![
            CartLine::new("Samosa", 20.0, 2),
            CartLine::new("Cutting Chai / Special Tea", 15.0, 1),
        ],
        cart_index: 0,
        status: OrderStatus::Paid,
        customer_mobile: None,
        payment_mode: None,
        kot_sent_count: 0,
    }
}

#[test]
fn menu_roundtrip_and_seed_detection() {
    let db = Database::open_for_tests();
    assert!(db.menu_is_empty());

    db.replace_menu(&[MenuItem::new("Desserts", "Gulab Jamun", "2 pcs", 45.0)])
        .unwrap();
    assert!(!db.menu_is_empty());

    let items = db.load_menu();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Gulab Jamun");
    assert_eq!(items[0].price, 45.0);
    assert!(items[0].is_available);

    db.update_menu_item_availability("Gulab Jamun", false)
        .unwrap();
    let items_after = db.load_menu();
    assert!(!items_after[0].is_available);
}

#[test]
fn paid_orders_persist_with_items_and_drive_bill_numbers() {
    let db = Database::open_for_tests();
    assert_eq!(db.next_bill_number(), 1);

    let mut order7 = sample_order(7);
    order7.discount_percent = 10.0;
    let mut ac = sample_order(8);
    ac.area = Some("AC Rooms".to_string());
    ac.is_ac = true;
    ac.ac_rate = 0.06;
    db.save_paid_order(&order7, "9876543210", &order7.totals())
        .unwrap();
    db.save_paid_order(&ac, "9123456780", &ac.totals()).unwrap();

    assert_eq!(db.next_bill_number(), 9);
    assert_eq!(
        db.scalar_string("SELECT customer_mobile FROM orders WHERE id = 7"),
        "9876543210"
    );
    assert_eq!(
        db.scalar_f64("SELECT discount FROM orders WHERE id = 7"),
        5.5
    );
    assert_eq!(db.scalar_f64("SELECT total FROM orders WHERE id = 7"), 49.5);
    assert_eq!(
        db.scalar_string("SELECT payment_mode FROM orders WHERE id = 7"),
        "",
        "mode is only recorded when the order is closed"
    );

    // Closing records the mode of payment.
    db.update_payment_mode(7, "UPI").unwrap();
    assert_eq!(
        db.scalar_string("SELECT payment_mode FROM orders WHERE id = 7"),
        "UPI"
    );
    // AC-room bill includes the surcharge and GST.
    assert!(
        (db.scalar_f64("SELECT total FROM orders WHERE id = 8") - 55.0 * 1.06 * 1.05).abs()
            < 1e-9
    );
    assert_eq!(
        db.scalar_i64("SELECT COUNT(*) FROM order_items WHERE order_id = 7"),
        2
    );
}

#[test]
fn open_orders_survive_a_roundtrip() {
    let db = Database::open_for_tests();

    let mut dine_in = sample_order(11);
    dine_in.status = OrderStatus::Serving;
    let mut line_with_note = CartLine::new("Veg / Chicken Momos", 65.0, 2);
    line_with_note.note = Some("Extra spicy".to_string());
    let takeout = Order {
        id: 12,
        label: "TK4".to_string(),
        service: Service::TakeOut,
        table_number: None,
        area: None,
        is_ac: false,
        ac_rate: 0.0,
        discount_percent: 0.0,
        cart: vec![line_with_note],
        cart_index: 0,
        status: OrderStatus::Ordering,
        customer_mobile: None,
        payment_mode: None,
        kot_sent_count: 0,
    };

    db.save_open_order(&dine_in).unwrap();
    db.save_open_order(&takeout).unwrap();

    // Check next_bill_number sees open orders too!
    assert_eq!(db.next_bill_number(), 13);

    // Upsert with an extra line must replace the stored cart, not append.
    dine_in.cart.push(CartLine::new("Gulab Jamun", 45.0, 1));
    db.save_open_order(&dine_in).unwrap();

    let restored = db.load_open_orders();
    assert_eq!(restored.len(), 2);

    let back = restored.iter().find(|o| o.id == 11).unwrap();
    assert_eq!(back.label, "Main-T1");
    assert_eq!(back.status, OrderStatus::Serving);
    assert_eq!(back.area, Some("Main Hall".to_string()));
    assert_eq!(back.cart.len(), 3);
    assert_eq!(back.cart[2].name, "Gulab Jamun");

    let tk = restored.iter().find(|o| o.id == 12).unwrap();
    assert_eq!(tk.service, Service::TakeOut);
    assert!(tk.area.is_none());
    assert_eq!(tk.cart[0].note.as_deref(), Some("Extra spicy"));

    db.delete_open_order(11).unwrap();
    let after = db.load_open_orders();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, 12);
}

#[test]
fn tables_persist_and_cleaning_expires() {
    let now = Local::now();
    let db = Database::open_for_tests();

    let mut dirty_old = PhysicalTable::ready("Main Hall", 1);
    dirty_old.status = TableStatus::Dirty;
    let mut dirty_new = PhysicalTable::ready("Garden", 6);
    dirty_new.status = TableStatus::Dirty;

    db.upsert_table(&dirty_old).unwrap();
    db.upsert_table(&dirty_new).unwrap();
    db.rt.block_on(async {
        let conn = db.conn.lock().await;
        conn.execute(
            "UPDATE tables SET updated_at = datetime('now', '-15 minutes')
             WHERE area = 'Main Hall' AND number = 1",
            (),
        )
        .await
        .unwrap();
    });

    let loaded = db.load_tables(&Area::defaults(), now);
    assert_eq!(loaded.len(), 2);
    let mh1 = loaded.iter().find(|t| t.number == 1).unwrap();
    assert_eq!(mh1.status, TableStatus::Ready);
    assert!(mh1.dirty_since.is_none());
    let g6 = loaded.iter().find(|t| t.number == 6).unwrap();
    assert_eq!(g6.status, TableStatus::Dirty);
    assert!(g6.dirty_since.is_some());

    // A table outside the configured layout is ignored.
    db.rt.block_on(async {
        let conn = db.conn.lock().await;
        conn.execute(
            "INSERT INTO tables (area, number, status, updated_at)
             VALUES ('Main Hall', 99, 'READY', datetime('now'))",
            (),
        )
        .await
        .unwrap();
    });
    assert_eq!(db.load_tables(&Area::defaults(), now).len(), 2);
}

#[test]
fn replace_menu_clears_previous_rows() {
    let db = Database::open_for_tests();
    db.replace_menu(&[
        MenuItem::new("A", "Old", "", 10.0),
        MenuItem::new("A", "Newer", "", 12.0),
    ])
    .unwrap();
    db.replace_menu(&[MenuItem::new("B", "Only", "", 99.0)])
        .unwrap();

    let items = db.load_menu();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Only");
}

#[test]
fn optional_columns_survive_takeout_orders() {
    let mut order = sample_order(3);
    order.service = Service::TakeOut;
    order.table_number = None;
    order.area = None;
    order.label = "TK1".to_string();

    let db = Database::open_for_tests();
    db.save_paid_order(&order, "9000000000", &order.totals())
        .unwrap();

    assert_eq!(
        db.scalar_optional_string("SELECT area FROM orders WHERE id = 3"),
        None
    );
}

#[test]
fn daily_sales_summary_and_historical_search() {
    let db = Database::open_for_tests();
    let mut order1 = sample_order(101);
    order1.payment_mode = Some(crate::models::PaymentMode::Upi);
    db.save_paid_order(&order1, "9998887770", &order1.totals())
        .unwrap();

    let today = Local::now().format("%Y-%m-%d").to_string();
    let summary = db.get_daily_sales_summary(&today);
    assert_eq!(summary.total_orders, 1);
    assert_eq!(summary.dine_in_orders, 1);
    assert_eq!(summary.upi_count, 1);
    assert!(summary.total_sales > 0.0);

    let results = db.search_bills("999888");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 101);
    assert_eq!(results[0].customer_mobile, "9998887770");

    let loaded = db.load_historical_order(101);
    assert!(loaded.is_some());
    let ord = loaded.unwrap();
    assert_eq!(ord.id, 101);
    assert_eq!(ord.cart.len(), 2);

    // Test customer CRM profile
    let crm = db.get_customer_crm_profile("9998887770");
    assert_eq!(crm.visit_count, 1);
    assert!(crm.total_spent > 0.0);
    assert!(!crm.favorite_items.is_empty());

    // Test Sales Analytics
    let analytics = db.get_sales_analytics(&today);
    assert_eq!(analytics.total_orders, 1);
    assert!(analytics.net_sales > 0.0);
    assert_eq!(analytics.payment_breakdown.len(), 1);
    assert_eq!(analytics.payment_breakdown[0].0, "UPI");

    // Test KOT status update and active KOTs loading
    let kot_id = db
        .save_kot(101, "T1", "Main Hall", 2, "Test Ticket", false)
        .unwrap();
    assert!(kot_id > 0);
    let active_kots = db.load_active_kots();
    assert_eq!(active_kots.len(), 1);
    assert_eq!(active_kots[0].status, "PENDING");

    db.update_kot_status(kot_id, "SERVED").unwrap();
    let active_after = db.load_active_kots();
    assert!(active_after.is_empty());

    // Test backup purging
    let temp_dir = std::env::temp_dir().join(format!("test_backup_purge_{}", kot_id));
    let _ = std::fs::create_dir_all(&temp_dir);
    let old_file = temp_dir.join("billing_2020-01-01.db");
    let new_file = temp_dir.join(format!("billing_{today}.db"));
    let _ = std::fs::write(&old_file, b"test");
    let _ = std::fs::write(&new_file, b"test");
    let purged = Database::purge_old_backups(&temp_dir, 30);
    assert_eq!(purged, 1);
    assert!(!old_file.exists());
    assert!(new_file.exists());
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn areas_settings_offers_and_z_report_persistence() {
    let db = Database::open_for_tests();

    // Areas
    let areas = vec![
        Area {
            name: "Garden".into(),
            is_ac: false,
            table_count: 10,
        },
        Area {
            name: "Banquet".into(),
            is_ac: true,
            table_count: 5,
        },
    ];
    db.replace_areas(&areas).unwrap();
    let loaded_areas = db.load_areas();
    assert_eq!(loaded_areas.len(), 2);
    assert_eq!(loaded_areas[0].name, "Garden");
    assert_eq!(loaded_areas[1].name, "Banquet");

    // Settings
    db.set_setting("RestaurantName", "SHREE GANESH").unwrap();
    assert_eq!(db.get_setting("RestaurantName"), "SHREE GANESH");
    assert_eq!(db.get_setting("NonexistentKey"), "");

    // Offers
    let offers = vec![crate::models::Offer {
        id: 1,
        name: "Weekend Special".into(),
        discount_percent: 15.0,
    }];
    db.replace_offers(&offers).unwrap();
    let loaded_offers = db.load_offers();
    assert_eq!(loaded_offers.len(), 1);
    assert_eq!(loaded_offers[0].name, "Weekend Special");
    assert_eq!(loaded_offers[0].discount_percent, 15.0);

    // Delete open order
    let mut open_ord = sample_order(505);
    open_ord.status = OrderStatus::Ordering;
    db.save_open_order(&open_ord).unwrap();
    assert_eq!(db.load_open_orders().len(), 1);
    db.delete_open_order(505).unwrap();
    assert_eq!(db.load_open_orders().len(), 0);

    // Z-Report persistence
    let summary = db.get_daily_sales_summary("2026-09-14");
    let report_id = db
        .save_z_report(
            "2026-09-14",
            summary.subtotal,
            summary.total_sales,
            summary.total_orders as u32,
            "RAW REPORT TEXT",
        )
        .unwrap();
    assert!(report_id > 0);

    // Load recent KOTs
    let kot_id = db.save_kot(505, "T5", "Garden", 1, "Ticket", true).unwrap();
    assert!(kot_id > 0);
    let recent_kots = db.load_recent_kots();
    assert!(!recent_kots.is_empty());
}
