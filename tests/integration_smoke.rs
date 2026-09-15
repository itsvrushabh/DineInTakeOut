//! Deterministic workflow, persistence, receipt, and rendering coverage.

use std::{fs, path::PathBuf};

use crossterm::event::KeyCode;
use dinein_takeout_billing::{
    app::{init_physical_tables, App},
    config::{export_menu_csv, load_areas_csv, load_menu, load_offers_csv},
    db::Database,
    models::{Area, Focus, MenuItem, Offer, OrderStatus, PaymentMode, RecentTab, TableStatus},
    receipts::{money, render_receipt},
    ui::ui,
};
use ratatui::{backend::TestBackend, Terminal};

use std::sync::atomic::{AtomicU64, Ordering};
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_path(name: &str) -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "dinein_takeout_{name}_{}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        count
    ));
    let _ = fs::remove_file(&path);
    path
}

fn fixture() -> (App, PathBuf) {
    let database_path = test_path("db");
    let database = Database::open(&database_path).unwrap();
    let areas = vec![Area {
        name: "Main Hall".into(),
        is_ac: false,
        table_count: 2,
    }];
    let app = App {
        items: vec![
            MenuItem::new("Food", "Samosa", "1 pc", 20.0),
            MenuItem::new("Food", "Paneer Curry", "plate", 100.0),
        ],
        orders: Vec::new(),
        active_order: 0,
        next_order_id: 1,
        next_takeout_id: 1,
        physical_tables: init_physical_tables(&areas),
        areas,
        restaurant_name: "SHREE KRISHNA RESTAURANT".into(),
        restaurant_address: "Station Road, Near Main Market".into(),
        restaurant_contact: "+91 98765 43210".into(),
        gst_number: "27TESTGST".into(),
        ac_rate: 0.06,
        offers: vec![Offer {
            id: 1,
            name: "Student".into(),
            discount_percent: 10.0,
        }],
        upi_id: "test@upi".into(),
        menu_index: 0,
        search: String::new(),
        focus: Focus::Menu,
        data_file: test_path("menu.csv"),
        matcher: fuzzy_matcher::skim::SkimMatcherV2::default().ignore_case(),
        selected_area_index: 0,
        selected_table_index: 0,
        notifications: Vec::new(),
        recent_bills: Vec::new(),
        recent_bill_index: 0,
        recent_kots: Vec::new(),
        recent_kot_index: 0,
        recent_tab: RecentTab::Bills,
        focus_return: Focus::Cart,
        database: Some(database),
        mobile_buffer: String::new(),
        payment_mode_index: 0,
        close_on_payment: false,
        offer_index: 0,
        pending_mobile: String::new(),
        show_help: false,
        table_input: String::new(),
        table_search_index: 0,
        bill_search_query: String::new(),
        bill_search_results: Vec::new(),
        bill_search_index: 0,
        item_note_buffer: String::new(),
        table_move_target_index: 0,
        daily_report_summary: None,
        categories: vec!["ALL".to_string(), "Food".to_string()],
        selected_category_index: 0,
        customer_crm: None,
        split_cash: String::new(),
        split_upi: String::new(),
        split_card: String::new(),
        split_field: 0,
        kds_kots: Vec::new(),
        kds_index: 0,
        sales_analytics: None,
        printer_config: dinein_takeout_billing::printer::PrinterConfig::default(),
        pending_effects: std::collections::VecDeque::new(),
    };
    (app, database_path)
}

#[test]
fn can_boot_app_and_quit_cleanly() {
    let (mut app, database_path) = fixture();
    assert!(!app.items.is_empty());
    assert!(app.handle_key(KeyCode::Char('q')));
    drop(app);
    let _ = fs::remove_file(database_path);
}

#[test]
fn e2e_order_flow_via_test_db() {
    let (mut app, database_path) = fixture();
    app.open_table_order();
    app.add_selected_to_cart();
    assert_eq!(app.order().cart[0].qty, 1);
    app.advance_stage();
    app.advance_stage();
    assert_eq!(app.order().status, OrderStatus::BillRequested);
    app.complete_billing("9876543210", Some(1));
    assert_eq!(app.order().status, OrderStatus::Paid);
    assert_eq!(app.recent_bills.len(), 1);
    app.close_order();
    assert_eq!(app.focus, Focus::PaymentMode);
    app.perform_close_with_mode(PaymentMode::Upi);
    assert!(app.orders.is_empty());
    assert_eq!(app.physical_tables[0].status, TableStatus::Dirty);
    let db = Database::open(&database_path).unwrap();
    assert_eq!(db.load_recent_bills()[0].id, 1);
    let _ = fs::remove_file(database_path);
}

#[test]
fn takeout_search_merge_and_quantity_guards_work() {
    let (mut app, database_path) = fixture();
    app.open_takeout_order();
    app.search = "paneer".into();
    assert_eq!(app.visible_items(), vec![1]);
    app.add_selected_to_cart();
    app.add_selected_to_cart();
    assert_eq!(app.order().cart[0].qty, 2);
    app.adjust_selected_line_quantity(-1);
    assert_eq!(app.order().cart[0].qty, 1);
    app.remove_selected_line();
    assert!(app.order().cart.is_empty());
    let _ = fs::remove_file(database_path);
}

#[test]
fn billing_guards_mobile_input_and_offer_flow_work() {
    let (mut app, database_path) = fixture();
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.begin_billing();
    assert_eq!(app.focus, Focus::MobileEntry);
    for digit in "12345678901".chars() {
        app.handle_key(KeyCode::Char(digit));
    }
    assert_eq!(app.mobile_buffer, "1234567890");
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.focus, Focus::OfferSelect);
    app.handle_key(KeyCode::Char('1'));
    assert_eq!(app.order().status, OrderStatus::Paid);
    assert!((app.order().totals().discount - 2.0).abs() < 1e-9);
    let _ = fs::remove_file(database_path);
}

#[test]
fn csv_loaders_skip_invalid_rows_and_preserve_quoted_values() {
    let dir = test_path("csv");
    fs::create_dir_all(&dir).unwrap();
    let menu = dir.join("menu.csv");
    fs::write(&menu, "Category,Item Name,Unit,Price\n\"Breakfast, Snacks\",\"Tea, special\",cup,12.5\nBad,,x,10\nBad,Free,x,0\n").unwrap();
    let items = load_menu(&menu).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Tea, special");
    assert_eq!(items[0].category, "Breakfast, Snacks");
    let areas = dir.join("areas.csv");
    fs::write(&areas, "Area,IsAC,TableCount\nAC,yes,0\nMain,no,3\n").unwrap();
    assert_eq!(load_areas_csv(&areas).unwrap()[0].table_count, 1);
    let offers = dir.join("offers.csv");
    fs::write(&offers, "Name,DiscountPercent\nBig,150\nSkip,0\n").unwrap();
    assert_eq!(load_offers_csv(&offers).unwrap()[0].discount_percent, 100.0);
    let exported = dir.join("out.csv");
    assert_eq!(export_menu_csv(&exported, &items).unwrap(), 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn receipts_render_totals_and_unicode_currency() {
    let (mut app, database_path) = fixture();
    app.open_takeout_order();
    app.add_selected_to_cart();
    let receipt = render_receipt(
        app.order(),
        Some("1234567890"),
        "GST-1",
        &app.restaurant_name,
        &app.restaurant_address,
        &app.restaurant_contact,
    );
    assert!(receipt.contains("SHREE KRISHNA RESTAURANT"));
    assert!(receipt.contains("Station Road, Near Main Market"));
    assert!(receipt.contains("Contact: +91 98765 43210"));
    assert!(receipt.contains("GST: GST-1"));
    assert!(receipt.contains("Mobile: 1234567890"));
    assert!(receipt.contains("GST (8.0%)"));
    assert_eq!(money(-12.5), "-₹12.50");
    let _ = fs::remove_file(database_path);
}

#[test]
fn all_primary_ui_states_render_on_compact_backend() {
    let (mut app, database_path) = fixture();
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.show_help = true;
    app.notify("Visible notification");
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Menu") || text.contains("Samosa"));
    app.show_help = false;
    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Notice"));
    assert!(text.contains("Table Details"));
    let _ = fs::remove_file(database_path);
}

#[test]
fn table_details_and_order_tracking_work() {
    let (mut app, database_path) = fixture();
    // Default table: Main Hall, Table 1
    assert_eq!(app.selected_area_name(), "Main Hall");
    assert_eq!(app.selected_table_index, 0);

    let pt = app
        .selected_physical_table()
        .expect("physical table exists");
    assert_eq!(pt.area, "Main Hall");
    assert_eq!(pt.number, 1);
    assert_eq!(pt.status, TableStatus::Ready);
    assert!(app.selected_table_order().is_none());

    // Open an order on Main Hall Table 1
    app.open_table_order();
    let order = app.selected_table_order().expect("order active on table");
    assert_eq!(order.label, "Main-T1");
    assert_eq!(order.status, OrderStatus::Ordering);

    // Selected physical table now reflects taking order
    let pt2 = app.selected_physical_table().unwrap();
    assert_eq!(pt2.status, TableStatus::Ordering);

    let _ = fs::remove_file(database_path);
}

#[test]
fn table_search_and_jump_workflow_works() {
    let (mut app, database_path) = fixture();
    // Start in Focus::Tables
    app.focus = Focus::Tables;

    // Press 'g' to trigger table jump search
    assert!(!app.handle_key(KeyCode::Char('g')));
    assert_eq!(app.focus, Focus::TableJump);

    // Initial search lists all tables
    let all = app.matching_tables();
    assert_eq!(all.len(), 2); // 2 tables in Main Hall fixture

    // Type '2' to search for Table 2
    app.handle_key(KeyCode::Char('2'));
    assert_eq!(app.table_input, "2");
    let matches = app.matching_tables();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].table_number, 2);

    // Press Enter to jump to Table 2
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.focus, Focus::Tables);
    assert_eq!(app.selected_table_index, 1); // 0-indexed: index 1 is Table 2
    assert_eq!(app.selected_physical_table().unwrap().number, 2);

    // Re-enter search and test Esc cancellation
    app.handle_key(KeyCode::Char('g'));
    app.handle_key(KeyCode::Char('9'));
    assert_eq!(app.table_input, "9");
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.focus, Focus::Tables);
    assert!(app.table_input.is_empty());

    let _ = fs::remove_file(database_path);
}

#[test]
fn table_jump_ui_renders_search_and_details() {
    let (mut app, database_path) = fixture();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    // Normal mode: should render "Table Details"
    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Table Details"));
    assert!(text.contains("Main Hall"));

    // Jump mode: should render "Search Table"
    app.focus = Focus::TableJump;
    app.table_input = "2".into();
    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Search Table"));
    assert!(text.contains("Search: 2|"));

    let _ = fs::remove_file(database_path);
}

#[test]
fn paid_bill_updates_payment_mode_across_bill_receipt_recent_and_db() {
    let (mut app, database_path) = fixture();
    let backend = TestBackend::new(100, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    app.open_table_order();
    app.add_selected_to_cart();
    app.complete_billing("9876543210", None);

    assert_eq!(app.order().status, OrderStatus::Paid);
    assert_eq!(app.order().payment_mode, None);
    assert_eq!(app.recent_bills[0].payment_mode, None);

    // Initial paid bill rendering (without payment mode)
    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("PAID ✔"));
    assert!(!text.contains("PAID via"));

    // Press 'p' on paid bill to update payment type
    app.handle_key(KeyCode::Char('p'));
    assert_eq!(app.focus, Focus::PaymentMode);
    assert!(!app.close_on_payment);

    // Press 'u' for UPI
    app.handle_key(KeyCode::Char('u'));
    assert_ne!(app.focus, Focus::PaymentMode);
    assert_eq!(app.order().payment_mode, Some(PaymentMode::Upi));
    assert_eq!(app.recent_bills[0].payment_mode, Some(PaymentMode::Upi));
    assert!(app.recent_bills[0].receipt.contains("Payment: UPI"));
    assert!(app.recent_bills[0].receipt.contains("Paid via"));

    // Verify database record updated
    let db = Database::open(&database_path).unwrap();
    assert_eq!(
        db.load_recent_bills()[0].payment_mode,
        Some(PaymentMode::Upi)
    );

    // Draw UI and check that bill title, totals, recent bills, and table info reflect UPI
    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("PAID via UPI ✔"));
    assert!(text.contains("Payment Type"));
    assert!(text.contains("UPI"));
    assert!(text.contains("[Paid: UPI]"));
    assert!(text.contains("[UPI]"));

    // Change payment type from UPI to CASH via hotkey 'c'
    app.handle_key(KeyCode::Char('p'));
    assert_eq!(app.focus, Focus::PaymentMode);
    app.handle_key(KeyCode::Char('c'));
    assert_eq!(app.order().payment_mode, Some(PaymentMode::Cash));
    assert_eq!(app.recent_bills[0].payment_mode, Some(PaymentMode::Cash));
    assert!(app.recent_bills[0].receipt.contains("Payment: CASH"));
    assert_eq!(
        db.load_recent_bills()[0].payment_mode,
        Some(PaymentMode::Cash)
    );

    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("PAID via CASH ✔"));
    assert!(text.contains("CASH"));
    assert!(text.contains("[Paid: CASH]"));
    assert!(text.contains("[CASH]"));

    // Change payment type to CARD via hotkey 'd'
    app.handle_key(KeyCode::Char('p'));
    assert_eq!(app.focus, Focus::PaymentMode);
    app.handle_key(KeyCode::Char('d'));
    assert_eq!(app.order().payment_mode, Some(PaymentMode::Card));
    assert_eq!(app.recent_bills[0].payment_mode, Some(PaymentMode::Card));
    assert!(app.recent_bills[0].receipt.contains("Payment: CARD"));
    assert_eq!(
        db.load_recent_bills()[0].payment_mode,
        Some(PaymentMode::Card)
    );

    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("PAID via CARD ✔"));
    assert!(text.contains("CARD"));
    assert!(text.contains("[Paid: CARD]"));
    assert!(text.contains("[CARD]"));

    // Close order via 'c' and Enter
    app.close_order();
    assert_eq!(app.focus, Focus::PaymentMode);
    assert!(app.close_on_payment);
    app.handle_key(KeyCode::Enter);
    assert!(app.orders.is_empty());
    assert_eq!(app.physical_tables[0].status, TableStatus::Dirty);

    // Verify recent bills still retains CARD receipt
    assert_eq!(app.recent_bills[0].payment_mode, Some(PaymentMode::Card));
    assert!(app.recent_bills[0].receipt.contains("Payment: CARD"));

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_features_kot_and_item_notes() {
    let (mut app, database_path) = fixture();
    app.open_table_order();
    app.add_selected_to_cart();
    assert_eq!(app.order().cart.len(), 1);

    // Add note to item
    app.open_item_note_prompt();
    assert_eq!(app.focus, Focus::ItemNote);
    app.item_note_buffer = "Less spicy, extra crisp".to_string();
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.focus, Focus::Menu);
    assert_eq!(
        app.order().cart[0].note.as_deref(),
        Some("Less spicy, extra crisp")
    );

    // Generate KOT
    app.generate_kot();
    assert_eq!(app.order().kot_sent_count, 1);
    assert_eq!(app.recent_kots.len(), 1);
    assert_eq!(app.recent_kots[0].item_count, 1);
    assert!(app.recent_kots[0]
        .ticket_text
        .contains("SHREE KRISHNA RESTAURANT"));
    let kot_text =
        dinein_takeout_billing::receipts::render_kot(app.order(), false, &app.restaurant_name);
    assert!(kot_text.contains("SHREE KRISHNA RESTAURANT"));
    assert!(kot_text.contains("KITCHEN ORDER TICKET"));
    assert!(kot_text.contains("↳ Less spicy, extra crisp"));
    assert!(!kot_text.contains("REPRINT"));

    // Second KOT should mark as REPRINT
    let kot_reprint =
        dinein_takeout_billing::receipts::render_kot(app.order(), true, &app.restaurant_name);
    assert!(kot_reprint.contains("[REPRINT]"));

    // Receipt should also contain note
    let receipt = render_receipt(
        app.order(),
        None,
        &app.gst_number,
        &app.restaurant_name,
        &app.restaurant_address,
        &app.restaurant_contact,
    );
    assert!(receipt.contains("↳ Less spicy, extra crisp"));

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_features_86_stock_toggle() {
    let (mut app, database_path) = fixture();
    assert!(app.items[0].is_available);

    // Toggle out of stock
    app.focus = Focus::Menu;
    app.menu_index = 0;
    app.handle_key(KeyCode::Char('o'));
    assert!(!app.items[0].is_available);

    // Try to add out-of-stock item
    app.open_table_order();
    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Enter);
    assert!(app.order().cart.is_empty(), "Item was out of stock!");

    // Render menu UI and verify 86 OUT badge
    let backend = TestBackend::new(80, 25);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("[86 OUT]"));

    // Toggle back to available and add to cart
    app.handle_key(KeyCode::Char('o'));
    assert!(app.items[0].is_available);
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.order().cart.len(), 1);

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_features_table_move_and_merge() {
    let (mut app, database_path) = fixture();
    app.areas[0].table_count = 3;
    app.rebuild_physical_tables();

    // Table 1: Samosa
    app.selected_table_index = 0;
    app.open_table_order();
    app.add_selected_to_cart();
    let t1_id = app.order().id;
    assert_eq!(app.order().table_number, Some(1));

    // Table 2: Paneer Curry
    app.selected_table_index = 1;
    app.open_table_order();
    app.menu_index = 1;
    app.add_selected_to_cart();
    let t2_id = app.order().id;
    assert_eq!(app.order().table_number, Some(2));
    assert_eq!(app.orders.len(), 2);

    // Select Table 1 and Move to Table 3 (empty -> transfer)
    app.selected_table_index = 0;
    app.open_table_move();
    assert_eq!(app.focus, Focus::TableMove);
    // Target index: Table 3 is index 1 among targets (targets exclude Table 1)
    let targets = app.table_move_targets();
    let t3_idx = targets.iter().position(|t| t.table_number == 3).unwrap();
    app.table_move_target_index = t3_idx;
    app.execute_table_move_or_merge();

    let moved_order = app.orders.iter().find(|o| o.id == t1_id).unwrap();
    assert_eq!(moved_order.table_number, Some(3));
    assert_eq!(app.physical_tables[0].status, TableStatus::Ready);
    assert_eq!(app.physical_tables[2].status, TableStatus::Ordering);

    // Now merge Table 3 into Table 2
    app.selected_table_index = 2; // Table 3
    app.open_table_move();
    let targets2 = app.table_move_targets();
    let t2_idx = targets2.iter().position(|t| t.table_number == 2).unwrap();
    app.table_move_target_index = t2_idx;
    app.execute_table_move_or_merge();

    assert_eq!(app.orders.len(), 1);
    let merged_order = &app.orders[0];
    assert_eq!(merged_order.id, t2_id);
    assert_eq!(merged_order.cart.len(), 2); // Samosa + Paneer Curry
    assert_eq!(app.physical_tables[2].status, TableStatus::Ready);

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_features_upi_qr_z_report_and_bill_search() {
    let (mut app, database_path) = fixture();
    app.open_table_order();
    app.add_selected_to_cart();
    let order_id = app.order().id;

    // Test Dynamic UPI QR code generation
    let (uri, blocks, amount) = app.upi_qr_uri_and_blocks().expect("UPI QR generation");
    assert!(uri.starts_with("upi://pay?pa=test@upi"));
    assert!(!blocks.is_empty());
    assert!(amount > 0.0);

    // Complete billing via UPI
    app.complete_billing("9876543210", None);
    app.select_payment_mode(PaymentMode::Upi);
    app.close_order();
    app.handle_key(KeyCode::Enter);

    // Test Daily Sales Summary (Z-Report)
    app.open_daily_report();
    assert_eq!(app.focus, Focus::DailyReport);
    let summary = app.daily_report_summary.as_ref().unwrap();
    assert_eq!(summary.total_orders, 1);
    assert_eq!(summary.upi_count, 1);
    assert!(summary.total_sales > 0.0);
    app.print_daily_report();

    // Test Bill Search & Reprint
    app.open_bill_search();
    assert_eq!(app.focus, Focus::BillSearch);
    assert_eq!(app.bill_search_results.len(), 1);
    assert_eq!(app.bill_search_results[0].id, order_id);

    app.bill_search_query = "98765".to_string();
    app.update_bill_search();
    assert_eq!(app.bill_search_results.len(), 1);

    app.reprint_selected_historical_bill();

    // UI render of modals
    let backend = TestBackend::new(80, 25);
    let mut terminal = Terminal::new(backend).unwrap();

    app.focus = Focus::DailyReport;
    terminal.draw(|frame| ui(frame, &app)).unwrap();

    app.focus = Focus::BillSearch;
    terminal.draw(|frame| ui(frame, &app)).unwrap();

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_features_daily_backup_creation() {
    let db_path = test_path("backup_test.db");
    let _ = fs::write(&db_path, b"mock sqlite database file");
    Database::create_daily_backup(&db_path);

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let backup_file = db_path
        .parent()
        .unwrap()
        .join("backups")
        .join(format!("billing_{today}.db"));
    assert!(backup_file.exists());

    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(backup_file);
}

#[test]
fn csv_sources_and_hotel_bill_details() {
    use dinein_takeout_billing::config::{
        default_menu, export_config_csv, export_table_csv, load_config_csv, load_table_csv,
    };

    // 1. default_menu must be completely empty
    let empty_menu = default_menu();
    assert!(empty_menu.is_empty(), "default_menu should have no items");

    // 2. table.csv roundtrip and table type & counts
    let table_path = test_path("test_table.csv");
    let test_areas = vec![
        Area {
            name: "Main Hall".into(),
            is_ac: false,
            table_count: 8,
        },
        Area {
            name: "AC Dining".into(),
            is_ac: true,
            table_count: 6,
        },
        Area {
            name: "Family Section".into(),
            is_ac: true,
            table_count: 4,
        },
        Area {
            name: "Garden".into(),
            is_ac: false,
            table_count: 6,
        },
    ];
    let written = export_table_csv(&table_path, &test_areas).unwrap();
    assert_eq!(written, 4);
    let loaded_areas = load_table_csv(&table_path).unwrap();
    assert_eq!(loaded_areas.len(), 4);
    assert_eq!(loaded_areas[0].name, "Main Hall");
    assert_eq!(loaded_areas[0].table_count, 8);
    assert!(!loaded_areas[0].is_ac);
    assert_eq!(loaded_areas[1].name, "AC Dining");
    assert_eq!(loaded_areas[1].table_count, 6);
    assert!(loaded_areas[1].is_ac);
    assert_eq!(loaded_areas[2].name, "Family Section");
    assert_eq!(loaded_areas[2].table_count, 4);
    assert!(loaded_areas[2].is_ac);
    assert_eq!(loaded_areas[3].name, "Garden");
    assert_eq!(loaded_areas[3].table_count, 6);
    assert!(!loaded_areas[3].is_ac);
    let _ = fs::remove_file(&table_path);

    // 3. config.csv with hotel name, address, contact, GST
    let config_path = test_path("test_config.csv");
    export_config_csv(
        &config_path,
        "SHREE KRISHNA RESTAURANT",
        "Station Road, Near Main Market",
        "+91 98765 43210",
        "27AAPFU0939F1ZV",
        0.06,
        "shreekrishna@upi",
    )
    .unwrap();
    let cfg = load_config_csv(&config_path).unwrap();
    assert_eq!(
        cfg.get("RestaurantName").unwrap(),
        "SHREE KRISHNA RESTAURANT"
    );
    assert_eq!(
        cfg.get("Address").unwrap(),
        "Station Road, Near Main Market"
    );
    assert_eq!(cfg.get("Contact").unwrap(), "+91 98765 43210");
    assert_eq!(cfg.get("GSTNumber").unwrap(), "27AAPFU0939F1ZV");
    let _ = fs::remove_file(&config_path);

    // 4. Verify receipt printing contains all hotel details
    let (mut app, database_path) = fixture();
    app.open_takeout_order();
    app.add_selected_to_cart();
    let receipt = render_receipt(
        app.order(),
        Some("9998887776"),
        &app.gst_number,
        &app.restaurant_name,
        &app.restaurant_address,
        &app.restaurant_contact,
    );
    assert!(receipt.contains("SHREE KRISHNA RESTAURANT"));
    assert!(receipt.contains("Station Road, Near Main Market"));
    assert!(receipt.contains("Contact: +91 98765 43210"));
    assert!(receipt.contains("GST: 27TESTGST"));
    assert!(receipt.contains("Mobile: 9998887776"));

    let _ = fs::remove_file(database_path);
}

#[test]
fn two_box_recent_bills_and_kots_in_db() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let (mut app, database_path) = fixture();

    // 1. Open order, add items, and generate KOT
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.generate_kot();

    // Verify KOT is stored in recent_kots and in database
    assert_eq!(app.recent_kots.len(), 1);
    assert_eq!(app.recent_kots[0].item_count, 1);
    let db = app.database.as_ref().unwrap();
    let loaded_kots = db.load_recent_kots();
    assert_eq!(loaded_kots.len(), 1);
    assert_eq!(loaded_kots[0].item_count, 1);
    assert_eq!(loaded_kots[0].label, "TK1");

    // 2. Complete the bill
    app.complete_billing("9991112222", None);
    app.select_payment_mode(PaymentMode::Cash);
    app.close_order();
    assert_eq!(app.recent_bills.len(), 1);

    // 3. Test Navigation in Focus::RecentBills between Bills box and KOTs box
    app.focus = Focus::RecentBills;
    assert_eq!(app.recent_tab, RecentTab::Bills);

    // Switch to KOTs box with right arrow / l
    app.handle_key(KeyCode::Right);
    assert_eq!(app.recent_tab, RecentTab::Kots);

    // Switch back to Bills box with left arrow / h
    app.handle_key(KeyCode::Left);
    assert_eq!(app.recent_tab, RecentTab::Bills);

    // 4. Test rendering of both boxes in UI
    let backend = TestBackend::new(100, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    // In Bills tab
    app.recent_tab = RecentTab::Bills;
    terminal.draw(|frame| ui(frame, &app)).unwrap();

    // In KOTs tab
    app.recent_tab = RecentTab::Kots;
    terminal.draw(|frame| ui(frame, &app)).unwrap();

    // 5. Test Z-Report saved to database (no disk files)
    app.open_daily_report();
    app.print_daily_report();

    // Verify that bills/ directory has NO .txt files
    if let Ok(entries) = std::fs::read_dir("bills") {
        let txt_count = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
            .count();
        assert_eq!(txt_count, 0, "No .txt files should be written to bills/");
    }

    let _ = fs::remove_file(database_path);
}

#[test]
fn generalized_box_switching_by_number() {
    let (mut app, database_path) = fixture();

    // App starts in Focus::Menu (Box 4)
    assert_eq!(app.focus, Focus::Menu);

    // Press '1' -> Switch to Box 1: Tables
    app.handle_key(KeyCode::Char('1'));
    assert_eq!(app.focus, Focus::Tables);

    // Press '4' -> Switch to Box 4: Menu
    app.handle_key(KeyCode::Char('4'));
    assert_eq!(app.focus, Focus::Menu);

    // Press '5' -> Switch to Box 5: Cart
    app.handle_key(KeyCode::Char('5'));
    assert_eq!(app.focus, Focus::Cart);

    // Press '6' -> Switch to Box 6: Recent Bills
    app.handle_key(KeyCode::Char('6'));
    assert_eq!(app.focus, Focus::RecentBills);
    assert_eq!(app.recent_tab, RecentTab::Bills);

    // Press '7' -> Switch to Box 7: KOT Bills
    app.handle_key(KeyCode::Char('7'));
    assert_eq!(app.focus, Focus::RecentBills);
    assert_eq!(app.recent_tab, RecentTab::Kots);

    // Press '1' -> Switch back to Box 1: Tables
    app.handle_key(KeyCode::Char('1'));
    assert_eq!(app.focus, Focus::Tables);

    // Press '2' -> Switch to Box 2: Table Jump
    app.handle_key(KeyCode::Char('2'));
    assert_eq!(app.focus, Focus::TableJump);

    // Press Esc in Table Jump -> Returns to Box 1: Tables
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.focus, Focus::Tables);

    // Press '3' -> Switch to Box 3: Search
    app.handle_key(KeyCode::Char('3'));
    assert_eq!(app.focus, Focus::Search);

    // Press Esc in Search -> Exits to Box 4: Menu
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.focus, Focus::Menu);

    // Test Tab cycling across all boxes:
    // From Menu (4) -> Cart (5)
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.focus, Focus::Cart);

    // From Cart (5) -> Recent Bills (6)
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.focus, Focus::RecentBills);
    assert_eq!(app.recent_tab, RecentTab::Bills);

    // From Recent Bills (6) -> KOT Bills (7)
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.focus, Focus::RecentBills);
    assert_eq!(app.recent_tab, RecentTab::Kots);

    // From KOT Bills (7) -> Tables (1)
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.focus, Focus::Tables);

    // From Tables (1) -> Search (3)
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.focus, Focus::Search);

    // From Search (3) -> Menu (4)
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.focus, Focus::Menu);

    // Test BackTab (reverse cycling):
    // From Menu (4) -> Search (3)
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Search);

    // From Search (3) -> Tables (1)
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Tables);

    // From Tables (1) -> KOT Bills (7)
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.focus, Focus::RecentBills);
    assert_eq!(app.recent_tab, RecentTab::Kots);

    // From KOT Bills (7) -> Recent Bills (6)
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.focus, Focus::RecentBills);
    assert_eq!(app.recent_tab, RecentTab::Bills);

    // From Recent Bills (6) -> Cart (5)
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Cart);

    // Verify UI rendering contains all 7 box numbers
    let backend = TestBackend::new(100, 35);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui(frame, &app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();

    assert!(text.contains("[1]"), "UI should show Box [1]");
    assert!(text.contains("[2]"), "UI should show Box [2]");
    assert!(text.contains("[3]"), "UI should show Box [3]");
    assert!(text.contains("[4]"), "UI should show Box [4]");
    assert!(text.contains("[5]"), "UI should show Box [5]");
    assert!(text.contains("[6]"), "UI should show Box [6]");
    assert!(text.contains("[7]"), "UI should show Box [7]");

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_advanced_delta_kots_and_reprints() {
    let (mut app, database_path) = fixture();
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Enter); // Open T1

    // Add Samosa (qty 2)
    app.focus = Focus::Menu;
    app.menu_index = 0;
    app.handle_key(KeyCode::Enter);
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.order().cart[0].qty, 2);
    assert_eq!(app.order().cart[0].kot_sent_qty, 0);

    // Generate 1st KOT
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('k'));
    assert_eq!(app.order().cart[0].kot_sent_qty, 2);
    assert_eq!(app.order().kot_sent_count, 1);
    assert_eq!(app.recent_kots.len(), 1);
    assert!(!app.recent_kots[0].is_reprint);

    // Add Paneer Curry (qty 1) and increase Samosa to 3
    app.focus = Focus::Menu;
    app.menu_index = 1;
    app.handle_key(KeyCode::Enter); // Paneer Curry
    app.menu_index = 0;
    app.handle_key(KeyCode::Enter); // Samosa qty + 1 = 3

    assert_eq!(app.order().cart.len(), 2);
    assert_eq!(app.order().cart[0].qty, 3);
    assert_eq!(app.order().cart[0].kot_sent_qty, 2); // 1 delta unsent
    assert_eq!(app.order().cart[1].qty, 1);
    assert_eq!(app.order().cart[1].kot_sent_qty, 0); // 1 delta unsent

    // Fire delta KOT
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('k'));
    assert_eq!(app.order().cart[0].kot_sent_qty, 3);
    assert_eq!(app.order().cart[1].kot_sent_qty, 1);
    assert_eq!(app.order().kot_sent_count, 2);
    assert_eq!(app.recent_kots.len(), 2);
    assert!(
        app.recent_kots[0].ticket_text.contains("DELTA")
            || app.recent_kots[0].ticket_text.contains("ADD-ON")
    );

    // Full reprint via Shift+K
    app.handle_key(KeyCode::Char('K'));
    assert_eq!(app.recent_kots.len(), 3);
    assert!(app.recent_kots[0].is_reprint);
    assert!(app.recent_kots[0].ticket_text.contains("REPRINT"));

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_advanced_kds_workflow() {
    let (mut app, database_path) = fixture();
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Enter); // Open T1

    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Enter); // Samosa
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('k')); // Send KOT

    // Open KDS
    app.open_kds();
    assert_eq!(app.focus, Focus::KitchenDisplay);
    assert_eq!(app.kds_kots.len(), 1);
    assert_eq!(app.kds_kots[0].status, "PENDING");

    // Bump status: PENDING -> PREPARING
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.kds_kots[0].status, "PREPARING");

    // Bump status: PREPARING -> READY
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.kds_kots[0].status, "READY");

    // Bump status: READY -> SERVED
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.kds_kots[0].status, "SERVED");

    // Exit KDS
    app.handle_key(KeyCode::Esc);
    assert_ne!(app.focus, Focus::KitchenDisplay);

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_advanced_split_payment_flow() {
    let (mut app, database_path) = fixture();
    app.offers.clear();
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Enter); // Open T1

    // Add items
    app.focus = Focus::Menu;
    app.menu_index = 1; // Paneer Curry: 100.0 + 5% GST = 105.0
    app.handle_key(KeyCode::Enter);

    let total = app.order().totals().total;
    assert!(total > 100.0);

    // Complete initial billing
    app.complete_billing("", None);
    assert_eq!(app.order().status, OrderStatus::Paid);

    // Open payment mode modal
    app.handle_key(KeyCode::Char('p'));
    assert_eq!(app.focus, Focus::PaymentMode);

    // Choose Split payment (shortcut 's' or '4')
    app.handle_key(KeyCode::Char('s'));
    assert_eq!(app.focus, Focus::SplitPayment);

    // Split field 0 (Cash): Enter "50"
    app.handle_key(KeyCode::Char('5'));
    app.handle_key(KeyCode::Char('0'));
    assert_eq!(app.split_cash, "50");

    // Tab to UPI field (index 1): Press 'a' to auto-fill remaining balance
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.split_field, 1);
    app.handle_key(KeyCode::Char('a'));
    let (_, cash, upi, card) = app.split_payment_totals();
    assert_eq!(cash, 50.0);
    assert!((cash + upi + card - total).abs() < 0.05);

    // Confirm split payment
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.order().status, OrderStatus::Paid);
    assert_eq!(app.order().payment_mode, Some(PaymentMode::Split));

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_advanced_category_filtering() {
    let (mut app, database_path) = fixture();
    // Setup 2 different categories
    app.items = vec![
        MenuItem::new("Starters", "Samosa", "1 pc", 20.0),
        MenuItem::new("Starters", "Vada Pav", "1 pc", 25.0),
        MenuItem::new("Mains", "Dal Makhani", "plate", 120.0),
        MenuItem::new("Mains", "Paneer Kadai", "plate", 150.0),
        MenuItem::new("Desserts", "Gulab Jamun", "2 pcs", 40.0),
    ];
    app.categories = vec![
        "ALL".to_string(),
        "Starters".to_string(),
        "Mains".to_string(),
        "Desserts".to_string(),
    ];
    app.selected_category_index = 0;

    // With "ALL" selected, all 5 items visible
    assert_eq!(app.visible_items().len(), 5);

    // Switch to Starters (category 1)
    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Right);
    assert_eq!(app.selected_category_index, 1);
    assert_eq!(app.visible_items().len(), 2);
    assert_eq!(app.items[app.visible_items()[0]].name, "Samosa");
    assert_eq!(app.items[app.visible_items()[1]].name, "Vada Pav");

    // Switch to Mains (category 2)
    app.handle_key(KeyCode::Right);
    assert_eq!(app.selected_category_index, 2);
    assert_eq!(app.visible_items().len(), 2);
    assert_eq!(app.items[app.visible_items()[0]].name, "Dal Makhani");

    // Switch back to Starters via Left
    app.handle_key(KeyCode::Left);
    assert_eq!(app.selected_category_index, 1);
    assert_eq!(app.visible_items().len(), 2);

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_advanced_complimentary_and_discount_items() {
    let (mut app, database_path) = fixture();
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Enter); // Open T1

    // Add Paneer Curry (100.0)
    app.focus = Focus::Menu;
    app.menu_index = 1;
    app.handle_key(KeyCode::Enter);

    app.focus = Focus::Cart;
    assert_eq!(app.order().cart[0].total(), 100.0);

    // Toggle Complimentary (NC)
    app.handle_key(KeyCode::Char('c'));
    assert!(app.order().cart[0].is_complimentary);
    assert_eq!(app.order().cart[0].total(), 0.0);

    // Toggle off NC
    app.handle_key(KeyCode::Char('c'));
    assert!(!app.order().cart[0].is_complimentary);
    assert_eq!(app.order().cart[0].total(), 100.0);

    // Cycle discounts: 0% -> 10% -> 20% -> 50% -> 100% -> 0%
    app.handle_key(KeyCode::Char('d'));
    assert_eq!(app.order().cart[0].discount_percent, 10.0);
    assert_eq!(app.order().cart[0].total(), 90.0);

    app.handle_key(KeyCode::Char('d'));
    assert_eq!(app.order().cart[0].discount_percent, 20.0);
    assert_eq!(app.order().cart[0].total(), 80.0);

    app.handle_key(KeyCode::Char('d'));
    assert_eq!(app.order().cart[0].discount_percent, 50.0);
    assert_eq!(app.order().cart[0].total(), 50.0);

    app.handle_key(KeyCode::Char('d'));
    assert_eq!(app.order().cart[0].discount_percent, 100.0);
    assert_eq!(app.order().cart[0].total(), 0.0);

    app.handle_key(KeyCode::Char('d'));
    assert_eq!(app.order().cart[0].discount_percent, 0.0);
    assert_eq!(app.order().cart[0].total(), 100.0);

    // Clear cart via Shift+C
    app.handle_key(KeyCode::Char('C'));
    assert!(app.order().cart.is_empty());

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_advanced_customer_crm_profile() {
    let (mut app, database_path) = fixture();
    app.offers.clear();
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Enter); // Open T1

    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Enter); // Samosa

    // Complete billing for customer 9876543210
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('p'));
    for digit in "9876543210".chars() {
        app.handle_key(KeyCode::Char(digit));
    }
    app.handle_key(KeyCode::Enter); // Enter payment mode
    app.handle_key(KeyCode::Char('c')); // Cash

    // Now start a second order for the same customer
    app.focus = Focus::Tables;
    app.selected_table_index = 1;
    app.handle_key(KeyCode::Enter); // Open T2

    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Enter); // Samosa

    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('p')); // Open MobileEntry

    for digit in "9876543210".chars() {
        app.handle_key(KeyCode::Char(digit));
    }

    assert!(app.customer_crm.is_some());
    let crm = app.customer_crm.as_ref().unwrap();
    assert_eq!(crm.phone, "9876543210");
    assert_eq!(crm.visit_count, 1);
    assert!(crm.total_spent > 0.0);
    assert!(crm.favorite_items.iter().any(|(name, _)| name == "Samosa"));

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_advanced_sales_analytics_modal() {
    let (mut app, database_path) = fixture();
    app.offers.clear();
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Enter);

    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Enter);

    // Pay bill
    app.complete_billing("", None);
    app.handle_key(KeyCode::Char('p'));
    app.handle_key(KeyCode::Char('c'));

    // Open sales analytics via 'A'
    app.handle_key(KeyCode::Char('A'));
    assert_eq!(app.focus, Focus::Analytics);
    assert!(app.sales_analytics.is_some());

    let a = app.sales_analytics.as_ref().unwrap();
    assert_eq!(a.total_orders, 1);
    assert!(a.net_sales > 0.0);
    assert_eq!(a.payment_breakdown[0].0, "CASH");
    assert_eq!(a.top_items[0].0, "Samosa");

    // Close analytics
    app.handle_key(KeyCode::Esc);
    assert_ne!(app.focus, Focus::Analytics);

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_advanced_backup_retention_purge() {
    let temp_dir = std::env::temp_dir().join(format!("backup_purge_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    // Create a fresh backup file
    let fresh_file = temp_dir.join("billing_2026-09-14.db");
    fs::write(&fresh_file, b"fresh backup data").unwrap();

    // Create an old backup file older than 30 days
    let old_file = temp_dir.join("billing_2026-07-01.db");
    fs::write(&old_file, b"old backup data").unwrap();

    let purged = Database::purge_old_backups(&temp_dir, 30);
    assert_eq!(purged, 1);
    assert!(!old_file.exists());
    assert!(fresh_file.exists());

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn comprehensive_ui_views_and_modals_render() {
    let (mut app, database_path) = fixture();
    let mut terminal = Terminal::new(TestBackend::new(140, 45)).unwrap();

    // 1. Full non-compact standard layout
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 2. KDS empty
    app.focus = Focus::KitchenDisplay;
    app.kds_kots.clear();
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 3. KDS with multiple tickets and statuses
    app.kds_kots = vec![
        dinein_takeout_billing::models::KotSummary {
            id: 1,
            order_id: 10,
            label: "T-1".into(),
            area: "Main Hall".into(),
            item_count: 2,
            ticket_text: "ITEM                           QTY\n------------------------------------------\nMasala Dosa                      2\n  ↳ Extra chutney\nSend to Kitchen".into(),
            is_reprint: false,
            created_at: "2026-09-14 20:00:00".into(),
            status: "PENDING".into(),
        },
        dinein_takeout_billing::models::KotSummary {
            id: 2,
            order_id: 11,
            label: "T-2".into(),
            area: "AC Dining".into(),
            item_count: 1,
            ticket_text: "ITEM                           QTY\nPaneer Curry                     1\nSend to Kitchen".into(),
            is_reprint: false,
            created_at: "2026-09-14 21:20:00".into(),
            status: "PREPARING".into(),
        },
        dinein_takeout_billing::models::KotSummary {
            id: 3,
            order_id: 12,
            label: "Take-Out #1".into(),
            area: "".into(),
            item_count: 3,
            ticket_text: "ITEM                           QTY\nSamosa                           3\nSend to Kitchen".into(),
            is_reprint: false,
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            status: "READY".into(),
        },
        dinein_takeout_billing::models::KotSummary {
            id: 4,
            order_id: 13,
            label: "T-3".into(),
            area: "Patio".into(),
            item_count: 1,
            ticket_text: "ITEM                           QTY\nChai                             1\nSend to Kitchen".into(),
            is_reprint: false,
            created_at: "invalid_time_format".into(),
            status: "SERVED".into(),
        },
    ];
    app.kds_index = 1;
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 4. Sales Analytics Modal
    app.focus = Focus::Analytics;
    app.sales_analytics = Some(dinein_takeout_billing::models::SalesAnalytics {
        date: "2026-09-14".into(),
        total_orders: 20,
        gross_sales: 6000.0,
        total_discounts: 300.0,
        total_tax: 270.0,
        cgst: 135.0,
        sgst: 135.0,
        net_sales: 5970.0,
        avg_bill_value: 298.5,
        payment_breakdown: vec![
            ("CASH".into(), 10, 3000.0),
            ("UPI".into(), 8, 2500.0),
            ("CARD".into(), 2, 470.0),
        ],
        top_items: vec![
            ("Samosa".into(), 30, 600.0),
            ("Paneer Curry".into(), 15, 1500.0),
        ],
    });
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 5. Mobile Entry Modal (without CRM, with CRM, 10 digits)
    app.focus = Focus::MobileEntry;
    app.mobile_buffer = "98765".into();
    terminal.draw(|f| ui(f, &app)).unwrap();

    app.mobile_buffer = "9876543210".into();
    app.customer_crm = Some(dinein_takeout_billing::models::CustomerCrmProfile {
        phone: "9876543210".into(),
        visit_count: 5,
        total_spent: 3500.0,
        favorite_items: vec![("Paneer Curry".into(), 6)],
        last_visit: Some("2026-09-10".into()),
    });
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 6. Payment Mode Modal
    app.focus = Focus::PaymentMode;
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 7. Offer Select Modal
    app.focus = Focus::OfferSelect;
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 8. Daily Report Modal
    app.focus = Focus::DailyReport;
    app.daily_report_summary = Some(dinein_takeout_billing::models::DailySalesSummary {
        date: "2026-09-14".into(),
        total_orders: 10,
        dine_in_orders: 6,
        takeout_orders: 4,
        subtotal: 2500.0,
        discount: 100.0,
        ac_charge: 50.0,
        tax: 120.0,
        total_sales: 2570.0,
        cash_count: 4,
        cash_total: 1000.0,
        upi_count: 5,
        upi_total: 1200.0,
        card_count: 1,
        card_total: 370.0,
        split_count: 0,
        split_total: 0.0,
        person_credit_count: 0,
        person_credit_total: 0.0,
        have_it_on_hotel_count: 0,
        have_it_on_hotel_total: 0.0,
        other_count: 0,
        other_total: 0.0,
    });
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 9. Bill Search Modal
    app.focus = Focus::BillSearch;
    app.bill_search_query = "9876".into();
    app.bill_search_results = vec![dinein_takeout_billing::models::HistoricalBill {
        id: 1,
        label: "T-1".into(),
        service: dinein_takeout_billing::models::Service::DineIn,
        customer_mobile: "9876543210".into(),
        total: 350.0,
        payment_mode: Some(PaymentMode::Cash),
        created_at: "2026-09-14 12:00".into(),
    }];
    app.bill_search_index = 0;
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 10. UPI QR Modal
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.focus = Focus::UpiQr;
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 11. Table Move Modal
    app.focus = Focus::TableMove;
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 12. Item Note Modal
    app.focus = Focus::ItemNote;
    app.item_note_buffer = "Extra spicy, no garlic".into();
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 13. Split Payment Modal
    app.focus = Focus::SplitPayment;
    app.split_cash = "50".into();
    app.split_upi = "50".into();
    app.split_card = "0".into();
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 14. Switch recent tab to KOTs
    app.focus = Focus::RecentBills;
    app.recent_tab = RecentTab::Kots;
    app.recent_kots = vec![dinein_takeout_billing::models::KotSummary {
        id: 99,
        order_id: 1,
        label: "T-1".into(),
        area: "Main Hall".into(),
        item_count: 1,
        ticket_text: "Sample ticket".into(),
        is_reprint: false,
        status: "SERVED".into(),
        created_at: "12:00:00".into(),
    }];
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 15. Search focus with query
    app.focus = Focus::Search;
    app.search = "sam".into();
    terminal.draw(|f| ui(f, &app)).unwrap();

    // 16. Tall search bar (height >= 3)
    let tall_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 60,
        height: 4,
    };
    terminal
        .draw(|f| dinein_takeout_billing::ui::search::render_search(f, &app, tall_area))
        .unwrap();
    app.search.clear();
    app.focus = Focus::Menu;
    terminal
        .draw(|f| dinein_takeout_billing::ui::search::render_search(f, &app, tall_area))
        .unwrap();

    // 17. Bill rendering with various order statuses & cart items
    let bill_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 60,
        height: 25,
    };
    app.focus = Focus::Cart;
    app.order_mut().status = OrderStatus::Serving;
    terminal
        .draw(|f| dinein_takeout_billing::ui::bill::render_bill(f, &app, bill_area))
        .unwrap();

    app.order_mut().status = OrderStatus::BillRequested;
    terminal
        .draw(|f| dinein_takeout_billing::ui::bill::render_bill(f, &app, bill_area))
        .unwrap();

    app.order_mut().status = OrderStatus::Paid;
    app.order_mut().payment_mode = None;
    terminal
        .draw(|f| dinein_takeout_billing::ui::bill::render_bill(f, &app, bill_area))
        .unwrap();

    app.order_mut().payment_mode = Some(PaymentMode::Upi);
    app.order_mut().cart[0].kot_sent_qty = 1; // partial KOT
    terminal
        .draw(|f| dinein_takeout_billing::ui::bill::render_bill(f, &app, bill_area))
        .unwrap();

    app.order_mut().cart[0].kot_sent_qty = 2; // full KOT
    terminal
        .draw(|f| dinein_takeout_billing::ui::bill::render_bill(f, &app, bill_area))
        .unwrap();

    // Bill in recent bills mode
    app.focus = Focus::RecentBills;
    app.recent_tab = RecentTab::Bills;
    terminal
        .draw(|f| dinein_takeout_billing::ui::bill::render_bill(f, &app, bill_area))
        .unwrap();
    app.recent_tab = RecentTab::Kots;
    terminal
        .draw(|f| dinein_takeout_billing::ui::bill::render_bill(f, &app, bill_area))
        .unwrap();

    // 18. Table details with Dirty and Paid states
    let table_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 34,
        height: 8,
    };
    app.focus = Focus::Tables;
    app.physical_tables[0].status = TableStatus::Dirty;
    app.physical_tables[0].dirty_since = Some(chrono::Local::now() - chrono::Duration::minutes(3));
    terminal
        .draw(|f| dinein_takeout_billing::ui::table_info::render_table_info(f, &app, table_area))
        .unwrap();

    app.physical_tables[0].status = TableStatus::Paid;
    terminal
        .draw(|f| dinein_takeout_billing::ui::table_info::render_table_info(f, &app, table_area))
        .unwrap();

    // 19. All Footers
    let footer_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 140,
        height: 1,
    };
    for focus in [
        Focus::Menu,
        Focus::Cart,
        Focus::Tables,
        Focus::RecentBills,
        Focus::Search,
        Focus::MobileEntry,
        Focus::PaymentMode,
        Focus::OfferSelect,
        Focus::DailyReport,
        Focus::BillSearch,
        Focus::UpiQr,
        Focus::TableMove,
        Focus::ItemNote,
        Focus::KitchenDisplay,
        Focus::SplitPayment,
        Focus::Analytics,
        Focus::TableJump,
    ] {
        app.focus = focus;
        terminal
            .draw(|f| dinein_takeout_billing::ui::footer::render_footer(f, &app, footer_area))
            .unwrap();
    }

    let _ = fs::remove_file(database_path);
}

#[test]
fn comprehensive_app_event_handling_and_shortcuts() {
    let (mut app, database_path) = fixture();

    // Order switching '[' and ']' with no orders
    assert!(!app.handle_key(KeyCode::Char('[')));
    assert!(!app.handle_key(KeyCode::Char(']')));

    // Open two orders
    app.open_takeout_order();
    app.add_selected_to_cart();
    app.open_takeout_order();
    assert_eq!(app.orders.len(), 2);
    assert_eq!(app.active_order, 1);
    app.handle_key(KeyCode::Char(']'));
    assert_eq!(app.active_order, 0);
    app.handle_key(KeyCode::Char('['));
    assert_eq!(app.active_order, 1);

    // Help toggle
    assert!(!app.show_help);
    app.handle_key(KeyCode::Char('?'));
    assert!(app.show_help);
    app.handle_key(KeyCode::Char('?'));
    assert!(!app.show_help);

    // Numbered box jumping 1-7
    for ch in ['1', '2', '3', '4', '5', '6', '7'] {
        app.handle_key(KeyCode::Char(ch));
    }

    // Cart line quantity adjustments
    app.active_order = 0;
    app.adjust_selected_line_quantity(1);
    assert_eq!(app.order().cart[0].qty, 2);
    app.adjust_selected_line_quantity(-1);
    assert_eq!(app.order().cart[0].qty, 1);
    // Cannot decrease below 1
    app.adjust_selected_line_quantity(-1);
    assert_eq!(app.order().cart[0].qty, 1);

    // Clear active cart
    app.clear_active_cart();
    assert!(app.order().cart.is_empty());

    // Cart operations on empty order or non-editable
    app.adjust_selected_line_quantity(1);
    app.remove_selected_line();
    app.clear_active_cart();

    // Re-add item to cart
    app.add_selected_to_cart();
    assert_eq!(app.order().cart.len(), 1);

    // Complimentary toggle
    app.toggle_selected_line_complimentary();
    assert!(app.order().cart[0].is_complimentary);
    app.toggle_selected_line_complimentary();
    assert!(!app.order().cart[0].is_complimentary);

    // Cycle discounts (0 -> 10 -> 20 -> 50 -> 100 -> 0)
    for _ in 0..5 {
        app.cycle_selected_line_discount();
    }
    assert_eq!(app.order().cart[0].discount_percent, 0.0);

    // Item notes workflow
    app.open_item_note_prompt();
    assert_eq!(app.focus, Focus::ItemNote);
    app.handle_key(KeyCode::Char('E'));
    app.handle_key(KeyCode::Char('x'));
    app.handle_key(KeyCode::Backspace);
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.order().cart[0].note.as_deref(), Some("E"));

    // Item note cancel
    app.open_item_note_prompt();
    app.handle_key(KeyCode::Esc);
    assert_ne!(app.focus, Focus::ItemNote);

    // Stock toggle (86)
    app.focus = Focus::Menu;
    assert!(app.items[app.menu_index].is_available);
    app.toggle_selected_menu_item_stock();
    assert!(!app.items[app.menu_index].is_available);
    app.toggle_selected_menu_item_stock();
    assert!(app.items[app.menu_index].is_available);

    // Category cycling via Right / Left
    app.handle_key(KeyCode::Right);
    assert_eq!(app.selected_category_index, 1);
    app.handle_key(KeyCode::Left);
    assert_eq!(app.selected_category_index, 0);

    // Table move workflow
    app.open_table_order();
    app.open_table_move();
    assert_eq!(app.focus, Focus::TableMove);
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Esc);
    assert_ne!(app.focus, Focus::TableMove);

    // Daily report shortcut 'z'
    app.handle_key(KeyCode::Char('z'));
    assert_eq!(app.focus, Focus::DailyReport);
    app.handle_key(KeyCode::Esc);
    assert_ne!(app.focus, Focus::DailyReport);

    // Analytics shortcut 'a' / 'A' / F8
    app.handle_key(KeyCode::F(8));
    assert_eq!(app.focus, Focus::Analytics);
    app.handle_key(KeyCode::Esc);

    // KDS shortcut F7
    app.handle_key(KeyCode::F(7));
    assert_eq!(app.focus, Focus::KitchenDisplay);
    // KDS navigation
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Char('s'));
    app.handle_key(KeyCode::Char('w'));
    app.handle_key(KeyCode::Char('r'));
    app.handle_key(KeyCode::F(5));
    app.handle_key(KeyCode::Char(' ')); // bump status
    app.handle_key(KeyCode::Esc); // exit KDS
    assert_ne!(app.focus, Focus::KitchenDisplay);

    // UPI QR workflow
    app.show_upi_qr();
    assert_eq!(app.focus, Focus::UpiQr);
    app.handle_key(KeyCode::Enter);
    assert_ne!(app.focus, Focus::UpiQr);

    // Bill Search workflow
    app.open_bill_search();
    assert_eq!(app.focus, Focus::BillSearch);
    app.handle_key(KeyCode::Char('1'));
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Enter); // reprint attempt
    app.handle_key(KeyCode::Esc);
    assert_ne!(app.focus, Focus::BillSearch);

    // Table cleaning tick and manual clean
    app.tick_cleaning();
    app.clean_selected_table();

    // Table search / jump workflow
    app.focus = Focus::TableJump;
    app.handle_key(KeyCode::Char('1'));
    app.handle_key(KeyCode::Backspace);
    app.handle_key(KeyCode::Esc);

    // Notification tick
    app.notify("Test notification");
    app.tick_notification();

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_deep_workflows_and_edge_cases() {
    let (mut app, database_path) = fixture();

    // 1. Focus::Tables key events
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Char('l')); // table right
    app.handle_key(KeyCode::Char('h')); // table left
    app.handle_key(KeyCode::Char('j')); // area down
    app.handle_key(KeyCode::Char('k')); // area up
    app.handle_key(KeyCode::Char('t')); // open takeout
    assert_eq!(app.orders.len(), 1);
    app.add_selected_to_cart();
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Char('s')); // advance stage
    app.handle_key(KeyCode::Char('r')); // clean table
    app.handle_key(KeyCode::Char('K')); // generate KOT
    app.handle_key(KeyCode::Char('g')); // table jump
    assert_eq!(app.focus, Focus::TableJump);
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.focus, Focus::Tables);
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.focus, Focus::Search);
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.focus, Focus::RecentBills);

    // 2. Focus::RecentBills key events
    app.handle_key(KeyCode::Char('l')); // Kots tab
    assert_eq!(app.recent_tab, RecentTab::Kots);
    app.handle_key(KeyCode::Char('h')); // Bills tab
    assert_eq!(app.recent_tab, RecentTab::Bills);
    app.handle_key(KeyCode::Char('j')); // down
    app.handle_key(KeyCode::Char('k')); // up
    app.handle_key(KeyCode::Char('p')); // reprint
    app.handle_key(KeyCode::Char('/')); // open bill search
    assert_eq!(app.focus, Focus::BillSearch);
    app.handle_key(KeyCode::Esc);
    app.focus = Focus::RecentBills;
    app.handle_key(KeyCode::Char('K')); // open KDS
    assert_eq!(app.focus, Focus::KitchenDisplay);
    app.handle_key(KeyCode::Esc);
    app.focus = Focus::RecentBills;
    app.recent_tab = RecentTab::Bills;
    app.handle_key(KeyCode::Tab); // switch to Kots
    assert_eq!(app.recent_tab, RecentTab::Kots);
    app.handle_key(KeyCode::Tab); // switch to Tables
    assert_eq!(app.focus, Focus::Tables);

    // 3. Focus::TableJump workflow
    app.focus = Focus::TableJump;
    app.handle_key(KeyCode::Char('M'));
    app.handle_key(KeyCode::Char('a'));
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Up);
    app.handle_key(KeyCode::Tab);
    app.handle_key(KeyCode::BackTab);
    app.handle_key(KeyCode::Enter); // jump
    assert_eq!(app.focus, Focus::Tables);

    // 4. Focus::PaymentMode key events
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('j')); // down
    app.handle_key(KeyCode::Char('k')); // up
    app.handle_key(KeyCode::Char('c')); // Cash
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('u')); // Upi
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('d')); // Card
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('1')); // 1: Cash
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('2')); // 2: UPI
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('3')); // 3: Card
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('4')); // 4: Split
    assert_eq!(app.focus, Focus::SplitPayment);
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.focus, Focus::PaymentMode);
    app.handle_key(KeyCode::Char('q')); // UPI QR
    assert_eq!(app.focus, Focus::UpiQr);
    app.handle_key(KeyCode::Char('q'));
    assert_ne!(app.focus, Focus::UpiQr);

    // 5. Focus::SplitPayment workflow
    app.focus = Focus::SplitPayment;
    let (total, _, _, _) = app.split_payment_totals();
    // Auto-fill cash field
    app.handle_key(KeyCode::Char('a'));
    assert!(!app.split_cash.is_empty());
    // Cycle fields Tab and BackTab
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.split_field, 1); // UPI
    app.handle_key(KeyCode::Char('1'));
    app.handle_key(KeyCode::Char('.'));
    app.handle_key(KeyCode::Char('5'));
    app.handle_key(KeyCode::Backspace);
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.split_field, 2); // Card
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.split_field, 1);
    app.handle_key(KeyCode::Up);
    assert_eq!(app.split_field, 0);
    app.handle_key(KeyCode::Down);
    assert_eq!(app.split_field, 1);
    // Confirm valid split
    app.split_cash = format!("{total:.2}");
    app.split_upi = "0.00".into();
    app.split_card = "0.00".into();
    app.handle_key(KeyCode::Enter);
    assert_ne!(app.focus, Focus::SplitPayment);

    // 6. Focus::OfferSelect workflow
    app.focus = Focus::OfferSelect;
    app.handle_key(KeyCode::Char('j'));
    app.handle_key(KeyCode::Char('k'));
    app.handle_key(KeyCode::Char('0')); // No offer
    assert_ne!(app.focus, Focus::OfferSelect);

    // 7. Focus::DailyReport print
    app.focus = Focus::DailyReport;
    app.handle_key(KeyCode::Char('p'));
    app.handle_key(KeyCode::Char('z'));
    assert_ne!(app.focus, Focus::DailyReport);

    // 8. Order reprints
    app.reprint_full_kot();

    let _ = fs::remove_file(database_path);
}

#[test]
fn app_bootstrap_new_and_lifecycle() {
    let mut app = App::new();
    assert!(!app.restaurant_name.is_empty());
    assert!(!app.physical_tables.is_empty());
    assert!(!app.items.is_empty());

    // Tick notification and cleaning
    app.notify("Boot notification");
    app.tick_notification();
    let _ = app.tick_cleaning();

    // Table jump matching
    app.table_input = "Main".into();
    let matches = app.matching_tables();
    assert!(!matches.is_empty());

    // UPI QR URI generation
    let uri_res = app.upi_qr_uri_and_blocks();
    assert_eq!(uri_res.is_some(), !app.orders.is_empty());

    // Open an order and test UPI QR
    app.open_takeout_order();
    app.add_selected_to_cart();
    let uri_res2 = app.upi_qr_uri_and_blocks();
    assert!(uri_res2.is_some());
    let (uri, blocks, total) = uri_res2.unwrap();
    assert!(uri.starts_with("upi://pay"));
    assert!(!blocks.is_empty());
    assert!(total > 0.0);
}

#[test]
fn app_cancellation_and_table_merge_workflows() {
    let (mut app, database_path) = fixture();

    // Cancellation with no order
    app.cancel_order();

    // Open table 1 order and cancel it
    app.selected_area_index = 0;
    app.selected_table_index = 0;
    app.open_table_order();
    assert_eq!(app.orders.len(), 1);
    assert_eq!(app.physical_tables[0].status, TableStatus::Ordering);
    app.cancel_order();
    assert_eq!(app.orders.len(), 0);
    assert_eq!(app.physical_tables[0].status, TableStatus::Ready);

    // Open takeout order and cancel it
    app.open_takeout_order();
    assert_eq!(app.orders.len(), 1);
    app.cancel_order();
    assert_eq!(app.orders.len(), 0);

    // Table move to empty table
    app.selected_area_index = 0;
    app.selected_table_index = 0;
    app.open_table_order();
    app.add_selected_to_cart();
    assert_eq!(app.order().cart.len(), 1);

    // Targets for table 1 should include table 2 (empty)
    let targets = app.table_move_targets();
    assert!(!targets.is_empty());
    app.table_move_target_index = 0;
    app.execute_table_move_or_merge();
    assert_eq!(app.physical_tables[0].status, TableStatus::Ready);
    assert_eq!(app.physical_tables[1].status, TableStatus::Ordering);

    // Now open an order on table 1 and merge it into table 2
    app.selected_table_index = 0;
    app.open_table_order();
    app.add_selected_to_cart();
    app.execute_table_move_or_merge(); // merge into table 2
    assert_eq!(app.physical_tables[0].status, TableStatus::Ready);

    let _ = fs::remove_file(database_path);
}

#[test]
fn pos_ultimate_edge_cases_and_100_percent_coverage() {
    // 1. App::default()
    let default_app = App::default();
    assert!(!default_app.items.is_empty());

    let (mut app, database_path) = fixture();

    // 2. Unmatched switch_to_box
    app.switch_to_box(0);
    app.switch_to_box(8);
    app.switch_to_box(99);

    // 3. Focus::Search key handling
    app.focus = Focus::Search;
    app.handle_key(KeyCode::Char('s'));
    app.handle_key(KeyCode::Char('a'));
    app.handle_key(KeyCode::Backspace);
    app.open_takeout_order();
    app.focus = Focus::Search;
    app.handle_key(KeyCode::Enter);
    assert!(!app.order().cart.is_empty());
    app.handle_key(KeyCode::Tab);
    assert_eq!(app.focus, Focus::Menu);
    app.focus = Focus::Search;
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Tables);
    app.focus = Focus::Search;
    app.handle_key(KeyCode::Down);
    assert_eq!(app.focus, Focus::Menu);
    app.focus = Focus::Search;
    app.handle_key(KeyCode::Esc);
    assert_eq!(app.focus, Focus::Menu);

    // 4. Focus::Menu key handling
    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Char('k'));
    app.handle_key(KeyCode::Char('j'));
    app.handle_key(KeyCode::Char('/'));
    assert_eq!(app.focus, Focus::Search);
    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Char('c')); // clear cart
    app.handle_key(KeyCode::Char('e')); // export config
    app.handle_key(KeyCode::Char('i')); // import config
    app.handle_key(KeyCode::Char('K')); // generate KOT on empty cart
    app.add_selected_to_cart();
    app.handle_key(KeyCode::Char('K')); // generate KOT with items
    app.handle_key(KeyCode::Char('g')); // table jump
    assert_eq!(app.focus, Focus::TableJump);
    app.handle_key(KeyCode::Esc);
    app.focus = Focus::Menu;
    app.handle_key(KeyCode::Char('p')); // begin billing
    assert_eq!(app.focus, Focus::MobileEntry);
    app.handle_key(KeyCode::Esc);

    // 5. Focus::Cart key handling
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('w'));
    app.handle_key(KeyCode::Char('s'));
    app.handle_key(KeyCode::Char('k'));
    app.handle_key(KeyCode::Char('K'));
    app.handle_key(KeyCode::Char('c'));
    app.handle_key(KeyCode::Char('d'));
    app.handle_key(KeyCode::Char('n'));
    assert_eq!(app.focus, Focus::ItemNote);
    app.handle_key(KeyCode::Esc);
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::Char('='));
    app.handle_key(KeyCode::Char('+'));
    app.handle_key(KeyCode::Char('-'));
    app.handle_key(KeyCode::Char('x'));
    app.handle_key(KeyCode::Delete);
    app.handle_key(KeyCode::Char('C'));
    app.handle_key(KeyCode::Char('g'));
    assert_eq!(app.focus, Focus::TableJump);
    app.handle_key(KeyCode::Esc);
    app.focus = Focus::Cart;
    app.add_selected_to_cart();
    app.handle_key(KeyCode::Char('p'));
    assert_eq!(app.focus, Focus::MobileEntry);
    app.handle_key(KeyCode::Esc);
    app.focus = Focus::Cart;
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.focus, Focus::Menu);

    // 6. Focus::Tables key handling
    app.focus = Focus::Tables;
    app.handle_key(KeyCode::Char('b')); // begin billing
    assert_eq!(app.focus, Focus::MobileEntry);
    app.handle_key(KeyCode::Esc);
    app.selected_table_index = 1;
    app.open_table_order();
    let ord_id = app.order().id;
    app.open_takeout_order();
    app.focus = Focus::Tables;
    app.selected_table_index = 1;
    app.handle_key(KeyCode::Enter); // switch to existing table order
    assert_eq!(app.order().id, ord_id);
    app.physical_tables[1].status = TableStatus::Dirty;
    app.handle_key(KeyCode::Enter); // dirty table enter cleans table
    assert_eq!(app.physical_tables[1].status, TableStatus::Ready);

    // 7. Focus::RecentBills key handling
    app.focus = Focus::RecentBills;
    app.recent_tab = RecentTab::Kots;
    app.handle_key(KeyCode::Enter);
    app.handle_key(KeyCode::Char('r'));
    app.handle_key(KeyCode::BackTab);
    assert_eq!(app.recent_tab, RecentTab::Bills);
    app.handle_key(KeyCode::Enter);
    app.handle_key(KeyCode::Char('r'));

    // 8. Focus::MobileEntry key handling
    app.focus = Focus::MobileEntry;
    app.mobile_buffer = "123".into();
    app.handle_key(KeyCode::Enter); // invalid length
    app.mobile_buffer = "9876543210".into();
    app.offers.clear();
    app.handle_key(KeyCode::Enter); // completes billing without offers

    // 9. Focus::PaymentMode key handling
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Enter);
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('s')); // split
    assert_eq!(app.focus, Focus::SplitPayment);
    app.handle_key(KeyCode::Esc);
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('5')); // PersonCredit
    app.focus = Focus::PaymentMode;
    app.handle_key(KeyCode::Char('6')); // HaveItOnHotel

    // 10. Focus::OfferSelect key handling
    app.focus = Focus::OfferSelect;
    app.offers.push(Offer {
        id: 1,
        name: "Special 10".into(),
        discount_percent: 10.0,
    });
    app.handle_key(KeyCode::Char('1'));
    app.focus = Focus::OfferSelect;
    app.handle_key(KeyCode::Enter);
    app.focus = Focus::OfferSelect;
    app.handle_key(KeyCode::Esc);

    // 11. Focus::TableJump empty / invalid query
    app.focus = Focus::TableJump;
    app.table_input = "".into();
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.focus, Focus::Tables);
    app.focus = Focus::TableJump;
    app.table_input = "NONEXISTENT_XYZ".into();
    app.handle_key(KeyCode::Enter);

    // 12. Focus::DailyReport
    app.focus = Focus::DailyReport;
    app.handle_key(KeyCode::Enter);
    app.focus = Focus::DailyReport;
    app.handle_key(KeyCode::Char('q'));

    // 13. Focus::UpiQr
    app.focus = Focus::UpiQr;
    app.handle_key(KeyCode::Char('Q'));

    // 14. Focus::ItemNote
    app.focus = Focus::ItemNote;
    app.item_note_buffer = "Test Note".into();
    app.handle_key(KeyCode::Enter);

    // 15. Focus::KitchenDisplay
    app.open_kds();
    app.handle_key(KeyCode::Char('q'));
    app.open_kds();
    app.handle_key(KeyCode::Char('k'));
    app.open_kds();
    app.handle_key(KeyCode::Char('K'));

    // 16. Focus::SplitPayment
    app.focus = Focus::SplitPayment;
    app.handle_key(KeyCode::Char('A')); // autofill
    app.handle_key(KeyCode::Enter); // confirm

    // 17. Focus::Analytics
    app.open_sales_analytics();
    app.handle_key(KeyCode::Char('q'));
    app.open_sales_analytics();
    app.handle_key(KeyCode::Char('a'));
    app.open_sales_analytics();
    app.handle_key(KeyCode::Char('A'));
    app.open_sales_analytics();
    app.handle_key(KeyCode::Enter);

    // 18. rebuild_physical_tables
    app.rebuild_physical_tables();

    // 19. UI Search view render
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            app.search = "tea".into();
            app.focus = Focus::Search;
            dinein_takeout_billing::ui::search::render_search(f, &app, area);
            let compact = ratatui::layout::Rect::new(0, 0, 80, 1);
            dinein_takeout_billing::ui::search::render_search(f, &app, compact);
        })
        .unwrap();

    // 20. Config import/export with empty directory
    let temp_dir = std::env::temp_dir().join(format!("test_import_export_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    app.data_file = temp_dir.join("menu.csv");
    app.export_config();
    app.import_config();
    let _ = std::fs::remove_dir_all(&temp_dir);

    let _ = fs::remove_file(database_path);
}
