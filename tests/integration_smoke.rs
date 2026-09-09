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

fn test_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dinein_takeout_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
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
        offer_index: 0,
        pending_mobile: String::new(),
        show_help: false,
        table_input: String::new(),
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
    let _ = fs::remove_file(database_path);
}
