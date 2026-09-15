use std::{fs, path::Path};
use crossterm::event::KeyCode;
use ratatui::{backend::TestBackend, Terminal};

use crate::{
    app::{max_takeout_number, App},
    db::Database,
    models::{
        Area, CartLine, Focus, KotSummary, Offer, Order, OrderStatus, PaymentMode,
        PhysicalTable, Service, TableStatus,
    },
    printer::{PrinterConfig, PrinterMode},
    ui::{bill::render_bill, floor_plan::render_tabs, table_info::render_table_info},
};

#[test]
fn test_app_new_no_database_and_fallback_branches() {
    let mut app = App::new_with_paths(None, None);
    assert!(app.database.is_none());
    assert!(!app.items.is_empty());
    assert!(!app.areas.is_empty());

    // Persistence with no database should not crash
    app.persist_table("Main Hall", 1);
    app.persist_active_order();

    // KDS with no database
    app.recent_kots.push(KotSummary {
        id: 1,
        order_id: 1,
        label: "TK1".into(),
        area: String::new(),
        item_count: 1,
        ticket_text: "KOT".into(),
        is_reprint: false,
        created_at: "12:00:00".into(),
        status: "PENDING".into(),
    });
    app.open_kds();
    assert_eq!(app.kds_kots.len(), 1);

    // Sales analytics and daily report with no database
    app.open_sales_analytics();
    assert!(app.sales_analytics.is_some());
    app.open_daily_report();
    assert!(app.daily_report_summary.is_some());

    // Bill search with no database
    app.open_bill_search();
    assert!(app.bill_search_results.is_empty());
    app.update_bill_search();
    assert!(app.bill_search_results.is_empty());

    // Reprint historical bill with no database
    app.bill_search_results.push(crate::models::HistoricalBill {
        id: 999,
        created_at: "2026-09-15 12:00:00".into(),
        label: "T-1".into(),
        service: Service::DineIn,
        customer_mobile: "9876543210".into(),
        total: 100.0,
        payment_mode: Some(PaymentMode::Cash),
    });
    app.bill_search_index = 0;
    app.reprint_selected_historical_bill();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("Could not load details")));

    // KOT generation and reprint with no database
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.generate_kot();
    assert_eq!(app.recent_kots[0].id, 0);
    app.reprint_full_kot();
    assert_eq!(app.recent_kots[0].id, 0);

    // Billing and payment update with no database
    app.complete_billing("9876543210", None);
    app.update_paid_order_payment_mode(PaymentMode::Cash);
    assert_eq!(app.order().payment_mode, Some(PaymentMode::Cash));

    // CRM update with no database
    app.mobile_buffer = "9876543210".into();
    app.update_customer_crm();
    assert!(app.customer_crm.is_none());
}

#[test]
fn test_app_new_with_invalid_database_path() {
    let app = App::new_with_paths(Some(Path::new("/proc/nonexistent_xyz/test.db")), None);
    assert!(app.database.is_none());
}

#[test]
fn test_app_new_with_csv_config_and_rate_over_one() {
    let temp_dir = std::env::temp_dir().join(format!("test_csv_cfg_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    fs::write(
        temp_dir.join("config.csv"),
        "Key,Value\nHotelName,MY SPECIAL HOTEL\nHotelAddress,MG Road 42\nPhone,+91 11111 22222\nGSTIN,27AABCS1429B1Z\nACRate,12.5\nUPI,hotel@upi\nbill_printer,BillPrinter1\nkot_printer,KotPrinter1\n",
    ).unwrap();

    fs::write(
        temp_dir.join("table.csv"),
        "Area,Tables,IsAC\nRooftop,4,true\n",
    ).unwrap();

    fs::write(
        temp_dir.join("menu.csv"),
        "Category,Name,Unit,Price,Available\nBreakfast,Special Dosa,Plate,95.0,true\n",
    ).unwrap();

    let app = App::new_with_paths(None, Some(&temp_dir));
    assert_eq!(app.restaurant_name, "MY SPECIAL HOTEL");
    assert_eq!(app.restaurant_address, "MG Road 42");
    assert_eq!(app.restaurant_contact, "+91 11111 22222");
    assert_eq!(app.gst_number, "27AABCS1429B1Z");
    assert!((app.ac_rate - 0.125).abs() < 1e-6);
    assert_eq!(app.upi_id, "hotel@upi");
    assert_eq!(app.areas.len(), 1);
    assert_eq!(app.areas[0].name, "Rooftop");
    assert_eq!(app.items.len(), 1);
    assert_eq!(app.items[0].name, "Special Dosa");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_app_new_with_database_seed_and_crash_reconciliation() {
    let temp_db = std::env::temp_dir().join(format!("test_reconcile_{}.db", std::process::id()));
    let _ = fs::remove_file(&temp_db);

    {
        let db = Database::open(&temp_db).unwrap();
        // Seed areas
        let areas = vec![Area { name: "Main Hall".into(), table_count: 5, is_ac: false }];
        db.replace_areas(&areas).unwrap();

        // Seed an open order on Table 2
        let order = Order {
            id: 10,
            label: "MH-T2".into(),
            service: Service::DineIn,
            table_number: Some(2),
            area: Some("Main Hall".into()),
            is_ac: false,
            ac_rate: 0.0,
            discount_percent: 0.0,
            cart: vec![CartLine::new("Chai", 20.0, 2)],
            cart_index: 0,
            status: OrderStatus::Ordering,
            customer_mobile: None,
            payment_mode: None,
            kot_sent_count: 0,
        };
        db.save_open_order(&order).unwrap();

        // Set physical table in DB as Ready (simulating crash before status was saved)
        let pt = PhysicalTable::ready("Main Hall", 2);
        db.upsert_table(&pt).unwrap();

        // Seed settings
        db.set_setting("restaurant_name", "RECONCILED RESTAURANT").unwrap();
        db.set_setting("restaurant_address", "Reconcile St").unwrap();
        db.set_setting("restaurant_contact", "1234567890").unwrap();
        db.set_setting("gst_number", "GST123").unwrap();
        db.set_setting("ac_rate", "0.08").unwrap();
        db.set_setting("upi_id", "rec@upi").unwrap();
    }

    let app = App::new_with_paths(Some(&temp_db), None);
    assert_eq!(app.restaurant_name, "RECONCILED RESTAURANT");
    assert_eq!(app.orders.len(), 1);

    // Verify reconciliation: table 2 should be Ordering and assigned order_id 10
    let table2 = app.physical_tables.iter().find(|t| t.area == "Main Hall" && t.number == 2).unwrap();
    assert_eq!(table2.status, TableStatus::Ordering);
    assert_eq!(table2.order_id, Some(10));

    let _ = fs::remove_file(&temp_db);
}

#[test]
fn test_cart_edge_cases_and_ensure_editable_order() {
    let mut app = App::new_with_paths(None, None);
    app.orders.clear();

    // Guard functions when orders is empty
    assert!(!app.ensure_editable_order());
    app.remove_selected_line();
    app.adjust_selected_line_quantity(1);
    app.clear_active_cart();
    app.toggle_selected_line_complimentary();
    app.cycle_selected_line_discount();
    app.open_item_note_prompt();

    // Open order with multiple items
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.add_selected_to_cart();
    assert_eq!(app.order().cart.len(), 1);
    // Add distinct item
    if app.items.len() > 1 {
        app.menu_index = 1;
        app.add_selected_to_cart();
    }
    if app.order().cart.len() > 1 {
        // Set cart_index to last item and remove it
        app.order_mut().cart_index = app.order().cart.len() - 1;
        app.remove_selected_line();
        assert_eq!(app.order().cart_index, app.order().cart.len() - 1);
    }

    // Invalid cart index adjustment
    app.order_mut().cart_index = 999;
    app.adjust_selected_line_quantity(1);
    assert!(app.notifications.iter().any(|(n, _)| n.contains("No item selected")));

    // Decrease below 1 guard
    app.order_mut().cart_index = 0;
    app.order_mut().cart[0].qty = 1;
    app.adjust_selected_line_quantity(-1);
    assert!(app.notifications.iter().any(|(n, _)| n.contains("Cannot decrease below 1")));

    // Clear active cart
    app.clear_active_cart();
    assert!(app.order().cart.is_empty());
}

#[test]
fn test_billing_and_kot_guards() {
    let mut app = App::new_with_paths(None, None);
    app.orders.clear();

    // Billing guards when orders is empty
    app.begin_billing();
    app.complete_billing("", None);
    app.perform_close_with_mode(PaymentMode::Cash);
    assert_eq!(app.split_payment_totals(), (0.0, 0.0, 0.0, 0.0));
    app.show_upi_qr();
    assert!(app.upi_qr_uri_and_blocks().is_none());

    // KOT guards when orders is empty
    app.generate_kot();
    app.reprint_full_kot();

    // Open order with empty cart
    app.open_takeout_order();
    app.begin_billing();
    app.complete_billing("", None);
    app.generate_kot();
    app.reprint_full_kot();

    // Zero / complimentary total guard
    app.add_selected_to_cart();
    app.order_mut().cart[0].is_complimentary = true;
    app.complete_billing("", None);
    assert!(app.notifications.iter().any(|(n, _)| n.contains("Total must be greater than zero")));

    // Split payment underpayment guard
    app.order_mut().cart[0].is_complimentary = false;
    app.split_cash = "1.00".into();
    app.confirm_split_payment();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("Remaining balance")));

    // KDS bump status when empty and with fallback
    app.kds_kots.clear();
    app.kds_bump_status();
    app.kds_kots.push(KotSummary {
        id: 5,
        order_id: 1,
        label: "TK1".into(),
        area: String::new(),
        item_count: 1,
        ticket_text: "KOT".into(),
        is_reprint: false,
        created_at: "12:00".into(),
        status: "UNKNOWN_CUSTOM".into(),
    });
    app.kds_index = 0;
    app.kds_bump_status();
    assert_eq!(app.kds_kots[0].status, "PENDING");

    // Reprint recent bill and recent kot when populated
    app.recent_bills.push(crate::models::BillSummary {
        id: 101,
        label: "TK1".into(),
        service: Service::TakeOut,
        total: 50.0,
        receipt: "MOCK RECEIPT".into(),
        payment_mode: Some(PaymentMode::Upi),
    });
    app.recent_bill_index = 0;
    app.reprint_selected_recent_bill();

    app.recent_kots.push(KotSummary {
        id: 202,
        order_id: 1,
        label: "TK1".into(),
        area: String::new(),
        item_count: 1,
        ticket_text: "MOCK KOT".into(),
        is_reprint: false,
        created_at: "12:00".into(),
        status: "PENDING".into(),
    });
    app.recent_kot_index = 0;
    app.reprint_selected_recent_kot();
}

#[test]
fn test_orders_and_tables_guards() {
    let mut app = App::new_with_paths(None, None);

    // Empty areas guard
    app.areas.clear();
    app.open_table_order();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("No dining areas configured")));

    // Restore areas
    app.areas = Area::defaults();
    app.orders.clear();

    // Close order guards
    app.close_order();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("No orders to close")));

    app.open_takeout_order();
    app.order_mut().cart.push(CartLine::new("Chai", 20.0, 1));
    app.close_order();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("Cannot close order with items")));

    // Payment mode update guard
    app.update_paid_order_payment_mode(PaymentMode::Cash);

    // Advance stage guards
    app.orders.clear();
    app.advance_stage();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("No active order")));

    app.open_takeout_order();
    app.order_mut().status = OrderStatus::Paid;
    app.advance_stage();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("already at final stage")));

    // Table move guards
    app.open_table_move();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("Selected table does not have an active order")));

    app.execute_table_move_or_merge();

    // Transfer to an AC table sets ac_rate
    app.selected_area_index = 0;
    app.selected_table_index = 0;
    app.open_table_order();
    assert_eq!(app.order().is_ac, false);

    // Find an AC area destination
    let ac_dest_idx = app.table_move_targets().iter().position(|t| t.is_ac).unwrap();
    app.table_move_target_index = ac_dest_idx;
    app.execute_table_move_or_merge();
    assert_eq!(app.order().is_ac, true);
    assert!((app.order().ac_rate - app.ac_rate).abs() < 1e-6);
}

#[test]
fn test_events_all_focus_and_keys() {
    let mut app = App::new_with_paths(None, None);

    // General navigation
    app.open_takeout_order();
    app.open_takeout_order();
    app.handle_key(KeyCode::Char(']'));
    app.handle_key(KeyCode::Char('['));
    app.handle_key(KeyCode::Char('?'));
    assert!(app.show_help);
    app.handle_key(KeyCode::Char('?'));
    assert!(!app.show_help);
    app.handle_key(KeyCode::F(7));
    assert_eq!(app.focus, Focus::KitchenDisplay);
    app.handle_key(KeyCode::Esc);
    app.handle_key(KeyCode::F(8));
    assert_eq!(app.focus, Focus::Analytics);
    app.handle_key(KeyCode::Esc);
    app.handle_key(KeyCode::Char('z'));
    assert_eq!(app.focus, Focus::DailyReport);
    app.handle_key(KeyCode::Esc);

    // Box numbers 1-7
    for d in 1..=7 {
        app.handle_key(KeyCode::Char(char::from_digit(d, 10).unwrap()));
    }

    // KitchenDisplay keys
    app.open_kds();
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Char('w'));
    app.handle_key(KeyCode::Char('s'));
    app.handle_key(KeyCode::Char(' '));
    app.handle_key(KeyCode::Char('r'));
    app.handle_key(KeyCode::F(5));
    app.handle_key(KeyCode::Esc);

    // SplitPayment keys
    app.open_split_payment();
    app.handle_key(KeyCode::BackTab);
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Tab);
    app.handle_key(KeyCode::Down);

    // Field 1
    app.split_field = 1;
    app.handle_key(KeyCode::Char('a'));
    app.handle_key(KeyCode::Char('5'));
    app.handle_key(KeyCode::Backspace);

    // Field 2
    app.split_field = 2;
    app.handle_key(KeyCode::Char('a'));
    app.handle_key(KeyCode::Char('8'));
    app.handle_key(KeyCode::Backspace);
    app.handle_key(KeyCode::Esc);

    // TableJump keys
    app.focus = Focus::TableJump;
    app.handle_key(KeyCode::Char('m'));
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::BackTab);
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Tab);
    app.handle_key(KeyCode::Esc);

    // BillSearch keys
    app.focus = Focus::BillSearch;
    app.handle_key(KeyCode::Char('1'));
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::BackTab);
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Tab);
    app.handle_key(KeyCode::Esc);

    // OfferSelect keys
    app.focus = Focus::OfferSelect;
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Char('k'));
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Char('j'));
    app.handle_key(KeyCode::Char('0'));

    // PaymentMode keys
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Char('k'));
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Char('j'));
    app.handle_key(KeyCode::Char('u'));
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('d'));
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('c'));
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('s'));
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('q'));

    // Tables keys
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Left);
    app.handle_key(KeyCode::Char('h'));
    app.handle_key(KeyCode::Right);
    app.handle_key(KeyCode::Char('l'));
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Char('k'));
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Char('j'));
    app.handle_key(KeyCode::Char('m'));
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Char('t'));
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Char('c'));
    app.handle_key(KeyCode::Char('s'));
    app.handle_key(KeyCode::Char('r'));
    app.handle_key(KeyCode::Char('K'));
    app.handle_key(KeyCode::Char('g'));
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Char('b'));
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::BackTab);
    app.handle_key(KeyCode::Tab);

    // Menu keys
    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Left);
    app.handle_key(KeyCode::Char('h'));
    app.handle_key(KeyCode::Right);
    app.handle_key(KeyCode::Char('l'));
    app.handle_key(KeyCode::Char('/'));
    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Char('g'));
    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Char('K'));
    app.handle_key(KeyCode::Char('p'));
    let temp_export_dir = std::env::temp_dir().join(format!("test_keys_export_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_export_dir);
    app.data_file = temp_export_dir.join("menu.csv");
    app.handle_key(KeyCode::Char('e'));
    app.handle_key(KeyCode::Char('i'));
    let _ = fs::remove_dir_all(&temp_export_dir);

    // Cart keys
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('w'));
    app.handle_key(KeyCode::Char('s'));
    app.handle_key(KeyCode::Char('k'));
    app.handle_key(KeyCode::Char('K'));
    app.handle_key(KeyCode::Char('c'));
    app.handle_key(KeyCode::Char('d'));
    app.handle_key(KeyCode::Char('C'));
    app.handle_key(KeyCode::Char('n'));
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('+'));
    app.handle_key(KeyCode::Char('-'));
    app.handle_key(KeyCode::Char('x'));
    app.handle_key(KeyCode::Char('g'));
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('p'));
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Tab);
    app.handle_key(KeyCode::BackTab);
}

#[test]
fn test_ui_render_all_edge_cases() {
    let mut app = App::new_with_paths(None, None);
    app.open_takeout_order();

    let mut line1 = CartLine::new("Special Dosa", 100.0, 2);
    line1.is_complimentary = true;
    line1.note = Some("Crispy".into());
    line1.kot_sent_qty = 1;

    let mut line2 = CartLine::new("Filter Coffee", 40.0, 2);
    line2.discount_percent = 10.0;
    line2.kot_sent_qty = 2;

    app.order_mut().cart = vec![line1, line2];
    app.order_mut().discount_percent = 10.0;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    // Render bill in various statuses
    terminal.draw(|f| {
        let area = f.area();
        app.order_mut().status = OrderStatus::Ordering;
        app.focus = Focus::Tables;
        render_bill(f, &app, area);

        app.order_mut().status = OrderStatus::Serving;
        render_bill(f, &app, area);

        app.order_mut().status = OrderStatus::BillRequested;
        render_bill(f, &app, area);

        app.order_mut().status = OrderStatus::Paid;
        app.order_mut().payment_mode = None;
        render_bill(f, &app, area);
    }).unwrap();

    // Render floor plan with serving & bill requested tables
    if app.physical_tables.len() > 2 {
        app.physical_tables[0].status = TableStatus::Serving;
        app.physical_tables[1].status = TableStatus::BillRequested;
    }
    app.focus = Focus::Tables;
    terminal.draw(|f| {
        render_tabs(f, &app, f.area());
    }).unwrap();

    // Render table info with no areas
    let saved_areas = app.areas.clone();
    app.areas.clear();
    terminal.draw(|f| {
        render_table_info(f, &app, f.area());
    }).unwrap();
    app.areas = saved_areas;

    // Render table search scrolled
    app.focus = Focus::TableJump;
    app.table_input = "".into();
    app.table_search_index = 5;
    terminal.draw(|f| {
        render_table_info(f, &app, ratatui::layout::Rect::new(0, 0, 80, 5));
    }).unwrap();

    // Render KDS empty
    app.kds_kots.clear();
    terminal.draw(|f| {
        crate::ui::kds::render_kds(f, &app);
    }).unwrap();

    // Render KDS with various ticket statuses and elapsed tiers
    let now = chrono::Local::now();
    let t_warn = (now - chrono::Duration::minutes(15)).format("%H:%M:%S").to_string();
    let t_urgent = (now - chrono::Duration::minutes(35)).format("%H:%M:%S").to_string();
    app.kds_kots = vec![
        KotSummary {
            id: 1,
            order_id: 10,
            label: "MH-T1".into(),
            area: "Main Hall".into(),
            item_count: 2,
            ticket_text: "ITEM        QTY\n----------------\nPaneer Tikka   1\n  ↳ Extra spicy\nSend to Kitchen\n".into(),
            is_reprint: false,
            created_at: t_warn,
            status: "PREPARING".into(),
        },
        KotSummary {
            id: 2,
            order_id: 11,
            label: "TK1".into(),
            area: "".into(),
            item_count: 1,
            ticket_text: "ITEM        QTY\n----------------\nMasala Dosa   1\n".into(),
            is_reprint: false,
            created_at: t_urgent,
            status: "READY".into(),
        },
        KotSummary {
            id: 3,
            order_id: 12,
            label: "MH-T2".into(),
            area: "Main Hall".into(),
            item_count: 3,
            ticket_text: "".into(),
            is_reprint: true,
            created_at: "not-a-time".into(),
            status: "SERVED".into(),
        },
    ];
    terminal.draw(|f| {
        crate::ui::kds::render_kds(f, &app);
    }).unwrap();

    // Render menu with many categories and unavailable item
    app.categories = vec!["ALL", "C1", "C2", "C3", "C4", "C5", "C6"].into_iter().map(String::from).collect();
    app.selected_category_index = 3;
    if !app.items.is_empty() {
        app.items[0].is_available = false;
    }
    app.focus = Focus::Menu;
    terminal.draw(|f| {
        crate::ui::menu::render_menu(f, &app, f.area());
    }).unwrap();
}

#[test]
fn test_printer_and_db_edge_cases() {
    // PrinterMode::Lpr dispatch
    let lpr_cfg = PrinterConfig {
        mode: PrinterMode::Lpr,
        bill_printer: "mock_test_printer_xyz".into(),
        ..Default::default()
    };
    let _ = crate::printer::send_bytes(&lpr_cfg, None, b"TEST BYTES");

    // Database empty string queries
    let temp_db = std::env::temp_dir().join(format!("test_edge_db_{}.db", std::process::id()));
    let _ = fs::remove_file(&temp_db);
    let db = Database::open(&temp_db).unwrap();

    let crm = db.get_customer_crm_profile("");
    assert_eq!(crm.phone, "");

    let analytics = db.get_sales_analytics("");
    assert!(!analytics.date.is_empty());

    let _ = fs::remove_file(&temp_db);
}

#[test]
fn test_max_takeout_number_edge_cases() {
    let orders = vec![
        Order {
            id: 1,
            label: "TK5".into(),
            service: Service::TakeOut,
            table_number: None,
            area: None,
            is_ac: false,
            ac_rate: 0.0,
            discount_percent: 0.0,
            cart: Vec::new(),
            cart_index: 0,
            status: OrderStatus::Ordering,
            customer_mobile: None,
            payment_mode: None,
            kot_sent_count: 0,
        },
        Order {
            id: 2,
            label: "MH-T1".into(),
            service: Service::DineIn,
            table_number: Some(1),
            area: Some("Main Hall".into()),
            is_ac: false,
            ac_rate: 0.0,
            discount_percent: 0.0,
            cart: Vec::new(),
            cart_index: 0,
            status: OrderStatus::Ordering,
            customer_mobile: None,
            payment_mode: None,
            kot_sent_count: 0,
        },
    ];
    assert_eq!(max_takeout_number(&orders), 5);
}

#[test]
fn test_advanced_events_and_table_operations() {
    let mut app = App::new_with_paths(None, None);

    // 1. Focus::PaymentMode keys
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.complete_billing("9876543210", None);
    app.begin_billing();
    assert_eq!(app.focus, Focus::PaymentMode);
    app.handle_key(KeyCode::Char('k'));
    app.handle_key(KeyCode::Char('j'));
    app.payment_mode_index = 3; // Split
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.focus, Focus::SplitPayment);
    app.handle_key(KeyCode::Esc);

    // 2. Focus::OfferSelect keys with multiple offers
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.offers = vec![
        Offer { id: 1, name: "Offer 1".into(), discount_percent: 10.0 },
        Offer { id: 2, name: "Offer 2".into(), discount_percent: 20.0 },
    ];
    app.focus = Focus::OfferSelect;
    app.handle_key(KeyCode::Down);
    assert_eq!(app.offer_index, 1);
    app.handle_key(KeyCode::Down);
    assert_eq!(app.offer_index, 2);
    app.handle_key(KeyCode::Up);
    assert_eq!(app.offer_index, 1);
    app.handle_key(KeyCode::Char('2'));
    assert_eq!(app.order().discount_percent, 20.0);

    // 3. Focus::TableJump navigation
    app.selected_area_index = 0;
    app.selected_table_index = 0;
    app.open_table_order();
    let t1_order_id = app.order().id;
    app.focus = Focus::TableJump;
    app.table_input = "Main".into();
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.focus, Focus::Tables);
    assert_eq!(app.order().id, t1_order_id);

    // 4. Focus::TableMove navigation and keyboard execution
    app.selected_area_index = 0;
    app.selected_table_index = 0;
    app.open_table_move();
    assert_eq!(app.focus, Focus::TableMove);
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Right);
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.focus, Focus::Tables);

    // 5. Focus::KitchenDisplay navigation
    app.open_kds();
    app.kds_kots = vec![
        KotSummary {
            id: 1,
            order_id: 1,
            label: "TK1".into(),
            area: "".into(),
            item_count: 1,
            ticket_text: "".into(),
            is_reprint: false,
            created_at: "12:00".into(),
            status: "PENDING".into(),
        },
        KotSummary {
            id: 2,
            order_id: 2,
            label: "TK2".into(),
            area: "".into(),
            item_count: 1,
            ticket_text: "".into(),
            is_reprint: false,
            created_at: "12:01".into(),
            status: "PENDING".into(),
        },
    ];
    app.handle_key(KeyCode::Down);
    assert_eq!(app.kds_index, 1);
    app.handle_key(KeyCode::Char('s'));
    assert_eq!(app.kds_index, 1);
    app.handle_key(KeyCode::Up);
    assert_eq!(app.kds_index, 0);
    app.handle_key(KeyCode::Esc);

    // 6. Focus::BillSearch navigation and backspace
    app.focus = Focus::BillSearch;
    app.handle_key(KeyCode::Char('x'));
    app.handle_key(KeyCode::Backspace);
    app.bill_search_results = vec![
        crate::models::HistoricalBill {
            id: 1,
            created_at: "2026-09-15 12:00".into(),
            label: "T-1".into(),
            service: Service::DineIn,
            customer_mobile: "".into(),
            total: 50.0,
            payment_mode: None,
        },
        crate::models::HistoricalBill {
            id: 2,
            created_at: "2026-09-15 12:05".into(),
            label: "T-2".into(),
            service: Service::DineIn,
            customer_mobile: "".into(),
            total: 75.0,
            payment_mode: None,
        },
    ];
    app.handle_key(KeyCode::Down);
    assert_eq!(app.bill_search_index, 1);
    app.handle_key(KeyCode::Tab);
    app.handle_key(KeyCode::Up);
    assert_eq!(app.bill_search_index, 0);
    app.handle_key(KeyCode::Esc);

    // 7. Focus::SplitPayment field 0 backspace, field 2 autofill and typing
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.open_split_payment();
    app.split_field = 0;
    app.handle_key(KeyCode::Char('5'));
    app.handle_key(KeyCode::Backspace);
    assert!(app.split_cash.is_empty());

    app.split_field = 2;
    app.handle_key(KeyCode::Char('a')); // auto-fills card
    assert!(!app.split_card.is_empty());
    app.handle_key(KeyCode::Backspace);
    app.handle_key(KeyCode::Char('9'));
    app.handle_key(KeyCode::Esc);
}

#[test]
fn test_tick_cleaning_and_table_merging_with_matching_notes() {
    let mut app = App::new_with_paths(None, None);

    // Dirty table with dirty_since = None
    app.physical_tables[0].status = TableStatus::Dirty;
    app.physical_tables[0].dirty_since = None;
    let cleaned = app.tick_cleaning();
    assert!(cleaned >= 1);
    assert_eq!(app.physical_tables[0].status, TableStatus::Ready);

    // Clean selected table when database is None
    app.selected_area_index = 0;
    app.selected_table_index = 0;
    app.physical_tables[0].status = TableStatus::Dirty;
    app.clean_selected_table();
    assert_eq!(app.physical_tables[0].status, TableStatus::Ready);

    // Table merging with matching item name and note
    app.selected_area_index = 0;
    app.selected_table_index = 0;
    app.open_table_order();
    let mut item1 = CartLine::new("Chai", 20.0, 1);
    item1.note = Some("Ginger".into());
    app.order_mut().cart.push(item1.clone());

    app.selected_table_index = 1;
    app.open_table_order();
    let mut item2 = CartLine::new("Chai", 20.0, 2);
    item2.note = Some("Ginger".into());
    let item3 = CartLine::new("Samosa", 30.0, 1);
    app.order_mut().cart.push(item2);
    app.order_mut().cart.push(item3);

    // Merge Table 2 into Table 1
    app.selected_table_index = 1;
    let target_idx = app.table_move_targets().iter().position(|t| t.table_number == 1).unwrap();
    app.table_move_target_index = target_idx;
    app.execute_table_move_or_merge();

    // Verify Chai with note "Ginger" was combined: 1 + 2 = 3
    let merged_order = app.order();
    let chai_line = merged_order.cart.iter().find(|l| l.name == "Chai" && l.note.as_deref() == Some("Ginger")).unwrap();
    assert_eq!(chai_line.qty, 3);
}

#[test]
fn test_import_config_edge_cases() {
    let mut app = App::new_with_paths(None, None);
    let temp_dir = std::env::temp_dir().join(format!("test_import_cases_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    app.data_file = temp_dir.join("menu.csv");

    // 1. menu.csv empty
    fs::write(temp_dir.join("menu.csv"), "").unwrap();
    app.import_config();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("menu.csv empty")));

    // 2. menu.csv valid, table.csv empty
    fs::write(
        temp_dir.join("menu.csv"),
        "Category,Name,Unit,Price,Available\nBreakfast,Idli,Plate,40.0,true\n",
    ).unwrap();
    fs::write(temp_dir.join("table.csv"), "").unwrap();
    fs::write(temp_dir.join("areas.csv"), "").unwrap();
    app.import_config();
    assert!(app.notifications.iter().any(|(n, _)| n.contains("table.csv / areas.csv empty")));

    // 3. config.csv invalid
    fs::write(
        temp_dir.join("table.csv"),
        "Area,Tables,IsAC\nGarden,5,false\n",
    ).unwrap();
    fs::write(temp_dir.join("offers.csv"), "Name,DiscountPercent\nOffer1,10.0\n").unwrap();
    fs::write(temp_dir.join("config.csv"), "corrupt_data_without_comma\n").unwrap();
    app.import_config();

    // 4. Valid files with database = None covers line 782
    fs::write(
        temp_dir.join("config.csv"),
        "Key,Value\nRestaurantName,IMPORT RESTAURANT\nAddress,Import Road\nPhone,123456\nGSTNumber,GST99\nAcRate,5.0\nUpiId,imp@upi\n",
    ).unwrap();
    app.database = None;
    app.import_config();
    assert_eq!(app.restaurant_name, "IMPORT RESTAURANT");
    assert_eq!(app.offers.len(), 1);

    let _ = fs::remove_dir_all(&temp_dir);
}
