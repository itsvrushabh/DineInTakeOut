//! Deterministic workflow, persistence, receipt, and rendering coverage.

use std::{fs, path::PathBuf};

use crossterm::event::KeyCode;
use dinein_takeout_billing::{
    app::{init_physical_tables, App},
    config::{export_menu_csv, load_areas_csv, load_menu, load_offers_csv},
    db::Database,
    models::{Area, Focus, MenuItem, Offer, OrderStatus, PaymentMode, TableStatus},
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
            MenuItem {
                category: "Food".into(),
                name: "Samosa".into(),
                unit: "1 pc".into(),
                price: 20.0,
            },
            MenuItem {
                category: "Food".into(),
                name: "Paneer Curry".into(),
                unit: "plate".into(),
                price: 100.0,
            },
        ],
        orders: Vec::new(),
        active_order: 0,
        next_order_id: 1,
        next_takeout_id: 1,
        physical_tables: init_physical_tables(&areas),
        areas,
        gst_number: "27TESTGST".into(),
        ac_rate: 0.06,
        offers: vec![Offer {
            id: 1,
            name: "Student".into(),
            discount_percent: 10.0,
        }],
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
    let receipt = render_receipt(app.order(), Some("1234567890"), "GST-1");
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
