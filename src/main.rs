use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use chrono::Local;
mod db;
mod models;
mod receipts;

use db::Database;
use models::{
    BillSummary, CartLine, Focus, MenuItem, Order, OrderStatus, PaymentMode, PhysicalTable,
    Service, TableArea, TableStatus, CLEANING_MINUTES,
};
use receipts::{load_recent, money, next_bill_number, render_receipt, save_and_print};

/// Where the embedded Turso (SQLite-compatible) database file lives.
const DB_PATH: &str = "data/billing.db";

struct App {
    items: Vec<MenuItem>,
    orders: Vec<Order>,  // all open orders (dine-in & take-out)
    active_order: usize, // index into `orders`
    next_order_id: u32,
    next_takeout_id: u32,
    physical_tables: Vec<PhysicalTable>, // all physical tables by area
    menu_index: usize,
    search: String,
    focus: Focus,
    data_file: PathBuf,
    matcher: SkimMatcherV2,
    selected_table_area: TableArea, // current area when navigating tables
    selected_table_index: usize,    // current table index within area
    notification: String,           // top-left banner message
    notification_until: Option<chrono::DateTime<Local>>, // dismisses after this time
    recent_bills: Vec<BillSummary>, // newest first, up to five completed bills
    recent_bill_index: usize,
    focus_return: Focus,        // panel to restore when the prompt closes
    database: Option<Database>, // None only when the DB could not be opened
    mobile_buffer: String,      // customer mobile captured in the billing prompt
    payment_mode_index: usize,  // selection inside the payment-mode popup
}

/// Default menu — Indian restaurant items with a fixed price chosen from the
/// user's estimated range.
fn default_menu() -> Vec<MenuItem> {
    let rows: &[(&str, &str, &str, f64)] = &[
        ("Breakfast & Snacks", "Samosa", "1-2 pcs", 20.0),
        ("Breakfast & Snacks", "Vada Pav", "1 pc", 25.0),
        ("Breakfast & Snacks", "Idli Sambhar", "2 pcs", 35.0),
        ("Breakfast & Snacks", "Masala Dosa", "1 plate", 60.0),
        (
            "Breakfast & Snacks",
            "Aloo Paratha (with curd/pickle)",
            "1 plate (1-2 pcs)",
            45.0,
        ),
        (
            "Breakfast & Snacks",
            "Chole Bhature",
            "2 bhature + curry",
            75.0,
        ),
        (
            "Breakfast & Snacks",
            "Poori Bhaji / Sabzi",
            "4 pooris + curry",
            55.0,
        ),
        ("Breads & Rice", "Tawa Roti / Phulka", "Per piece", 10.0),
        (
            "Breads & Rice",
            "Tandoori Roti / Butter Roti",
            "Per piece",
            18.0,
        ),
        ("Breads & Rice", "Plain / Butter Naan", "Per piece", 35.0),
        (
            "Breads & Rice",
            "Plain Steamed / Jeera Rice",
            "Full plate",
            75.0,
        ),
        (
            "Main Course (Veg)",
            "Dal Tadka / Dal Fry",
            "Full plate",
            95.0,
        ),
        ("Main Course (Veg)", "Dal Makhani", "Full plate", 120.0),
        (
            "Main Course (Veg)",
            "Aloo Gobi / Jeera Aloo",
            "Full plate",
            100.0,
        ),
        (
            "Main Course (Veg)",
            "Mixed Vegetable Curry",
            "Full plate",
            125.0,
        ),
        (
            "Main Course (Veg)",
            "Paneer Butter Masala / Kadai Paneer",
            "Full plate",
            170.0,
        ),
        (
            "Main Course (Veg)",
            "Veg Pulao / Veg Biryani",
            "Full plate",
            125.0,
        ),
        (
            "Main Course (Veg)",
            "Standard Veg Thali",
            "Full meal platter",
            120.0,
        ),
        (
            "Main Course (Non-Veg)",
            "Egg Curry",
            "2 eggs + gravy",
            105.0,
        ),
        (
            "Main Course (Non-Veg)",
            "Chicken Curry / Chicken Masala",
            "3-4 pcs + gravy",
            190.0,
        ),
        (
            "Main Course (Non-Veg)",
            "Butter Chicken / Kadai Chicken",
            "Full/Half portion",
            225.0,
        ),
        (
            "Main Course (Non-Veg)",
            "Chicken Biryani",
            "Full plate",
            175.0,
        ),
        (
            "Main Course (Non-Veg)",
            "Fish Curry (Local catch)",
            "2-3 pcs + gravy",
            200.0,
        ),
        (
            "Main Course (Non-Veg)",
            "Mutton Curry / Rogan Josh",
            "3-4 pcs + gravy",
            310.0,
        ),
        (
            "Main Course (Non-Veg)",
            "Non-Veg Thali (Chicken)",
            "Full meal platter",
            190.0,
        ),
        (
            "Indo-Chinese & Fast Food",
            "Veg Fried Rice / Chowmein",
            "Full plate",
            90.0,
        ),
        (
            "Indo-Chinese & Fast Food",
            "Chicken Fried Rice / Chowmein",
            "Full plate",
            125.0,
        ),
        (
            "Indo-Chinese & Fast Food",
            "Veg / Chicken Momos",
            "6-8 pcs",
            65.0,
        ),
        ("Desserts", "Gulab Jamun", "2 pcs", 45.0),
        ("Desserts", "Rasgulla", "2 pcs", 40.0),
        (
            "Desserts",
            "Gajar Ka Halwa (Seasonal)",
            "1 cup (100g)",
            70.0,
        ),
        ("Desserts", "Kulfi / Ice Cream Scoop", "1 serving", 40.0),
        ("Beverages", "Cutting Chai / Special Tea", "1 cup", 15.0),
        ("Beverages", "Filter Coffee", "1 cup", 25.0),
        ("Beverages", "Sweet / Salted Lassi", "1 glass", 45.0),
        ("Beverages", "Chaas (Spiced Buttermilk)", "1 glass", 20.0),
        ("Beverages", "Fresh Lime Soda / Water", "1 glass", 35.0),
    ];
    rows.iter()
        .map(|(c, n, u, p)| MenuItem {
            category: c.to_string(),
            name: n.to_string(),
            unit: u.to_string(),
            price: *p,
        })
        .collect()
}

fn init_physical_tables() -> Vec<PhysicalTable> {
    let mut tables = Vec::new();
    let areas = TableArea::all();

    for area in areas.iter() {
        for number in 1..=area.table_count() {
            tables.push(PhysicalTable::ready(*area, number));
        }
    }
    tables
}

/// Highest TKn suffix across the given orders, so take-out numbering survives
/// restarts.
fn max_takeout_number(orders: &[Order]) -> u32 {
    orders
        .iter()
        .filter_map(|order| order.label.strip_prefix("TK"))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

impl App {
    fn new() -> Self {
        let data_file = PathBuf::from("menu.csv");

        // Open the embedded Turso database. A failure is not fatal: the app
        // keeps running from CSV/defaults.
        let database = match Database::open(Path::new(DB_PATH)) {
            Ok(db) => Some(db),
            Err(error) => {
                eprintln!("Database unavailable ({error}); running without persistence.");
                None
            }
        };

        // Menu precedence: database catalogue → menu.csv → built-in defaults.
        // Whatever we fall back to is seeded into the DB so it becomes the
        // single source of truth for the next launch.
        let mut db_notice = String::new();
        let items = match &database {
            Some(db) if !db.menu_is_empty() => db.load_menu(),
            _ => match load_menu(&data_file) {
                Ok(items) if !items.is_empty() => {
                    if let Some(db) = &database {
                        if let Err(error) = db.replace_menu(&items) {
                            db_notice = format!("Menu seed failed: {error}");
                        }
                    }
                    items
                }
                _ => {
                    let items = default_menu();
                    if let Some(db) = &database {
                        if let Err(error) = db.replace_menu(&items) {
                            db_notice = format!("Menu seed failed: {error}");
                        }
                    }
                    items
                }
            },
        };

        // Restore unpaid orders and physical table states from the last run.
        let now = Local::now();
        let (orders, next_order_id, physical_tables) = match &database {
            Some(db) => {
                let orders = db.load_open_orders();
                let stored_tables = db.load_tables(now);
                let mut tables = init_physical_tables();
                for table in &mut tables {
                    if let Some(stored) = stored_tables
                        .iter()
                        .find(|s| s.area == table.area && s.number == table.number)
                    {
                        table.status = stored.status;
                        table.order_id = stored.order_id;
                        table.dirty_since = stored.dirty_since;
                    }
                }
                // Reconcile: a table marked Ready that still has an open order
                // follows the order's stage instead (crash-consistency fix).
                for order in &orders {
                    if let (Some(area), Some(number)) = (order.area, order.table_number) {
                        if let Some(table) = tables
                            .iter_mut()
                            .find(|t| t.area == area && t.number == number)
                        {
                            if table.status == TableStatus::Ready || table.order_id.is_none() {
                                table.status = order.status.table_status();
                                table.order_id = Some(order.id);
                                let _ = db.upsert_table(table);
                            }
                        }
                    }
                }
                let bill_number = db.next_bill_number();
                (orders, bill_number, tables)
            }
            None => {
                let recent_bills = load_recent();
                (
                    Vec::new(),
                    next_bill_number(&recent_bills),
                    init_physical_tables(),
                )
            }
        };

        let recent_bills = load_recent();
        let next_takeout_id = max_takeout_number(&orders).saturating_add(1);

        Self {
            items,
            orders,
            active_order: 0,
            next_order_id,
            next_takeout_id,
            physical_tables,
            menu_index: 0,
            search: String::new(),
            focus: Focus::Menu,
            data_file,
            matcher: SkimMatcherV2::default().ignore_case(),
            selected_table_area: TableArea::FrontGarden,
            selected_table_index: 0,
            notification: {
                let storage = if database.is_some() {
                    "data/billing.db"
                } else {
                    "no DB"
                };
                if db_notice.is_empty() {
                    format!("Loaded menu. Storage: {storage}.")
                } else {
                    db_notice
                }
            },
            notification_until: Some(Local::now() + chrono::Duration::minutes(10)),
            recent_bills,
            recent_bill_index: 0,
            focus_return: Focus::Cart,
            database,
            mobile_buffer: String::new(),
            payment_mode_index: 0,
        }
    }

    // -- persistence helpers ---------------------------------------------------

    fn persist_table(&self, area: TableArea, number: usize) {
        if let Some(db) = &self.database {
            if let Some(table) = self
                .physical_tables
                .iter()
                .find(|t| t.area == area && t.number == number)
            {
                if let Err(error) = db.upsert_table(table) {
                    eprintln!("Table persist failed: {error}");
                }
            }
        }
    }

    fn persist_active_order(&self) {
        if let Some(db) = &self.database {
            if let Some(order) = self.orders.get(self.active_order) {
                if let Err(error) = db.save_open_order(order) {
                    eprintln!("Order persist failed: {error}");
                }
            }
        }
    }

    /// Auto-clean sweep: dirty tables become Ready once the cleaning window
    /// has elapsed. Returns how many tables were cleaned.
    fn tick_cleaning(&mut self) -> usize {
        let deadline = chrono::Duration::minutes(CLEANING_MINUTES);
        let now = Local::now();
        let mut cleaned = 0;
        for table in &mut self.physical_tables {
            if table.status == TableStatus::Dirty {
                if let Some(since) = table.dirty_since {
                    if now - since >= deadline {
                        table.status = TableStatus::Ready;
                        table.dirty_since = None;
                        table.order_id = None;
                        cleaned += 1;
                    }
                } else {
                    // Defensive: a Dirty row without a timestamp cleans at once.
                    table.status = TableStatus::Ready;
                    table.order_id = None;
                    cleaned += 1;
                }
            }
        }
        if cleaned > 0 && self.database.is_some() {
            let snapshot: Vec<PhysicalTable> = self.physical_tables.clone();
            if let Some(db) = &self.database {
                for table in snapshot {
                    if table.status == TableStatus::Ready {
                        let _ = db.upsert_table(&table);
                    }
                }
            }
        }
        cleaned
    }

    // -- order (table) management --------------------------------------------

    fn order(&self) -> &Order {
        &self.orders[self.active_order]
    }

    fn order_mut(&mut self) -> &mut Order {
        &mut self.orders[self.active_order]
    }

    /// Marks the currently selected table as cleaned (back to Ready) when it
    /// is in the Cleaning state.
    fn clean_selected_table(&mut self) {
        let target = self
            .physical_tables
            .iter()
            .find(|t| {
                t.area == self.selected_table_area && t.number == self.selected_table_index + 1
            })
            .map(|t| (t.area, t.number, t.status));

        if let Some((area, table_num, status)) = target {
            if status != TableStatus::Dirty {
                self.notify(String::from("Selected table is not being cleaned."));
                return;
            }
            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == area && t.number == table_num)
            {
                pt.status = TableStatus::Ready;
                pt.dirty_since = None;
                pt.order_id = None;
            }
            self.persist_table(area, table_num);
            self.notify(format!("Table {table_num} cleaned and ready."));
        }
    }

    /// Opens order for the selected physical table (dine-in only). On a dirty
    /// table Enter marks it cleaned; on an occupied table it switches to that
    /// table's order.
    fn open_table_order(&mut self) {
        // Get table info without holding reference
        let table_info = self
            .physical_tables
            .iter()
            .find(|t| {
                t.area == self.selected_table_area && t.number == self.selected_table_index + 1
            })
            .map(|t| (t.area, t.number, t.status, t.order_id));

        if let Some((area, table_num, status, order_id)) = table_info {
            if status == TableStatus::Dirty {
                // Staff finished cleaning early — return the table to Ready.
                self.clean_selected_table();
                return;
            }

            if status != TableStatus::Ready {
                if let Some(order_id) = order_id {
                    if let Some(order_index) = self.orders.iter().position(|o| o.id == order_id) {
                        self.active_order = order_index;
                        self.notify(format!("Switched to {}.", self.order().label));
                    }
                }
                return;
            }

            let id = self.next_order_id;
            self.next_order_id += 1;
            let label = format!(
                "{}-T{}",
                area.label().split_whitespace().next().unwrap_or("T"),
                table_num
            );

            let order = Order {
                id,
                label,
                service: Service::DineIn,
                table_number: Some(table_num),
                area: Some(area),
                cart: Vec::new(),
                cart_index: 0,
                status: OrderStatus::Ordering,
            };

            self.orders.push(order);
            self.active_order = self.orders.len() - 1;

            // Update physical table status
            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == area && t.number == table_num)
            {
                pt.status = TableStatus::Ordering;
                pt.order_id = Some(id);
            }

            self.persist_active_order();
            self.persist_table(area, table_num);
            self.notify(format!("Opened order for Table {}", table_num));
        }
    }

    /// Opens a new take-out order (virtual table).
    fn open_takeout_order(&mut self) {
        let id = self.next_order_id;
        self.next_order_id += 1;
        let tk_count = self.next_takeout_id;
        self.next_takeout_id += 1;
        let order = Order {
            id,
            label: format!("TK{}", tk_count),
            service: Service::TakeOut,
            table_number: None,
            area: None,
            cart: Vec::new(),
            cart_index: 0,
            status: OrderStatus::Ordering,
        };

        self.orders.push(order);
        self.active_order = self.orders.len() - 1;
        self.persist_active_order();
        self.notify(format!("Opened take-out order TK{}.", tk_count));
    }

    /// Starts closing the active paid order: first confirm the mode of
    /// payment, then close (table goes to Cleaning).
    fn close_order(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No orders to close."));
            return;
        }

        if self.order().status != OrderStatus::Paid {
            let label = self.order().label.clone();
            self.notify(format!("Generate the bill for {label} before closing it."));
            return;
        }

        self.payment_mode_index = 0;
        self.focus_return = self.focus;
        self.focus = Focus::PaymentMode;
    }

    /// Confirms the mode of payment and closes the active paid order. Its
    /// table enters the Cleaning state and becomes Ready again automatically
    /// after CLEANING_MINUTES.
    fn perform_close_with_mode(&mut self, mode: PaymentMode) {
        if self.orders.is_empty() || self.order().status != OrderStatus::Paid {
            return; // stale prompt; nothing to close
        }

        let order = self.order();
        let label = order.label.clone();
        let table_info = if let (Some(table_num), Some(area)) = (order.table_number, order.area) {
            Some((table_num, area))
        } else {
            None
        };
        let closed_id = order.id;

        // Record how this bill was settled.
        if let Some(db) = &self.database {
            if let Err(error) = db.update_payment_mode(closed_id, mode.label()) {
                self.notify(format!("Could not save payment mode: {error}"));
            }
        }
        let mode_display = mode.display();

        self.orders.remove(self.active_order);
        if self.active_order >= self.orders.len() && !self.orders.is_empty() {
            self.active_order = self.orders.len() - 1;
        }
        self.focus = self.focus_return;

        // The paid order is already in the history table; drop its open copy.
        if let Some(db) = &self.database {
            let _ = db.delete_open_order(closed_id);
        }

        // Send the table to cleaning (auto-ready after the window elapses).
        if let Some((table_num, area)) = table_info {
            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == area && t.number == table_num)
            {
                pt.status = TableStatus::Dirty;
                pt.order_id = None;
                pt.dirty_since = Some(Local::now());
            }
            self.persist_table(area, table_num);
            self.notify(format!(
                "Closed {label} via {mode_display}. Table {table_num} cleaning — auto-ready in {CLEANING_MINUTES} min."
            ));
            return;
        }

        self.notify(format!("Closed {label} via {mode_display}."));
    }

    /// Advances the active order through its lifecycle:
    /// Ordering → Serving → Ready-for-bill.
    fn advance_stage(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No active order."));
            return;
        }

        let (current, table_number, area) = {
            let order = self.order();
            (order.status, order.table_number, order.area)
        };

        match current.advance() {
            Some(next) => {
                self.order_mut().status = next;
                let label = self.order().label.clone();

                if let (Some(table_num), Some(area)) = (table_number, area) {
                    if let Some(pt) = self
                        .physical_tables
                        .iter_mut()
                        .find(|t| t.area == area && t.number == table_num)
                    {
                        pt.status = next.table_status();
                    }
                    self.persist_table(area, table_num);
                }
                self.persist_active_order();
                let stage = match next {
                    OrderStatus::Serving => "now being served",
                    OrderStatus::BillRequested => "ready for bill",
                    _ => "advanced",
                };
                self.notify(format!("Table {label} — {stage}."));
            }
            None => {
                self.notify(String::from(
                    "Order already at final stage (use p to bill, c to close).",
                ));
            }
        }
    }

    // -- fuzzy filtering ------------------------------------------------------

    /// Fuzzy-matches the search query against item name + category.
    /// Empty query returns all items in order.
    fn visible_items(&self) -> Vec<usize> {
        let q = self.search.trim();
        if q.is_empty() {
            return (0..self.items.len()).collect();
        }
        let mut scored: Vec<(i64, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                // Prefer name matches; fall back to category with a penalty.
                let name_score = self.matcher.fuzzy_match(&it.name, q);
                let cat_score = self.matcher.fuzzy_match(&it.category, q).map(|s| s / 2); // category hits rank below name hits
                name_score.or(cat_score).map(|s| (s, i))
            })
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        scored.into_iter().map(|(_, i)| i).collect()
    }

    fn add_selected_to_cart(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }

        let vis = self.visible_items();
        if let Some(&idx) = vis.get(self.menu_index) {
            let item = self.items[idx].clone();
            let order = self.order_mut();
            if let Some(line) = order.cart.iter_mut().find(|l| l.name == item.name) {
                line.qty += 1;
            } else {
                order.cart.push(CartLine {
                    name: item.name.clone(),
                    unit_price: item.price,
                    qty: 1,
                });
            }
            self.notify(format!("Added {} to {}.", item.name, self.order().label));
            self.persist_active_order();
        }
    }

    fn remove_selected_line(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }

        let order = self.order_mut();
        if order.cart_index < order.cart.len() {
            let name = order.cart[order.cart_index].name.clone();
            order.cart.remove(order.cart_index);
            if order.cart_index >= order.cart.len() && !order.cart.is_empty() {
                order.cart_index = order.cart.len() - 1;
            }
            self.notify(format!("Removed {name}."));
            self.persist_active_order();
        }
    }

    /// Changes the quantity of the selected cart line. A quantity reduced to
    /// zero is removed from the bill.
    fn adjust_selected_line_quantity(&mut self, change: i32) {
        if !self.ensure_editable_order() {
            return;
        }

        /// What to do after mutating (or falling back to removing) a line.
        enum Outcome {
            Message(String),
            RemoveLine,
        }

        let outcome = {
            let order = self.order_mut();
            if order.cart_index >= order.cart.len() {
                Outcome::Message(String::from("No item selected in the bill."))
            } else if change > 0 {
                order.cart[order.cart_index].qty += change as u32;
                let name = order.cart[order.cart_index].name.clone();
                Outcome::Message(format!("Increased {name}."))
            } else if order.cart[order.cart_index].qty > 1 {
                order.cart[order.cart_index].qty -= (-change) as u32;
                let name = order.cart[order.cart_index].name.clone();
                Outcome::Message(format!("Decreased {name}."))
            } else {
                Outcome::RemoveLine
            }
        };

        match outcome {
            Outcome::Message(message) => {
                self.notify(message);
                self.persist_active_order();
            }
            Outcome::RemoveLine => self.remove_selected_line(),
        }
    }

    fn clear_active_cart(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }
        self.order_mut().cart.clear();
        let order = self.order_mut();
        order.cart_index = 0;
        self.persist_active_order();
        self.notify("Cart cleared.");
    }

    /// Paid orders are immutable because their receipt has already been saved.
    fn ensure_editable_order(&mut self) -> bool {
        match self.orders.get(self.active_order) {
            None => {
                self.notify("No active order. Open a table or take-out order first.");
                false
            }
            Some(order) if order.status == OrderStatus::Paid => {
                self.notify(format!("Bill #{} is paid and cannot be changed.", order.id));
                false
            }
            Some(_) => true,
        }
    }

    fn notify(&mut self, msg: impl Into<String>) {
        self.notification = msg.into();
        self.notification_until = Some(Local::now() + chrono::Duration::minutes(10));
    }

    fn tick_notification(&mut self) {
        if let Some(until) = self.notification_until {
            if Local::now() >= until {
                self.notification.clear();
                self.notification_until = None;
            }
        }
    }
    /// Starts the billing flow: validates the active order, then captures
    /// the customer's mobile number before generating the bill.
    fn begin_billing(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No active order."));
            return;
        }

        if self.order().status == OrderStatus::Paid {
            self.notify(format!(
                "Bill #{} has already been generated.",
                self.order().id
            ));
            return;
        }

        if self.order().cart.is_empty() {
            self.notify(String::from("Cart is empty."));
            return;
        }

        self.focus_return = self.focus;
        self.mobile_buffer.clear();
        self.focus = Focus::MobileEntry;
    }

    /// Generates, saves, and prints the bill for the active order. `mobile`
    /// is the customer's 10-digit number (empty when skipped). The mode of
    /// payment is confirmed later, when the order is closed.
    fn complete_billing(&mut self, mobile: &str) {
        if self.orders.is_empty() || self.order().status == OrderStatus::Paid {
            return; // stale prompt (e.g. order changed mid-entry); ignore
        }
        if self.order().cart.is_empty() {
            self.notify(String::from("Cart is empty."));
            return;
        }

        let customer_mobile = if mobile.trim().is_empty() {
            None
        } else {
            Some(mobile.trim())
        };
        let (order_id, order_label, order_service, table_info, totals, bill_text) = {
            let order = self.order();
            let table_info = match (order.table_number, order.area) {
                (Some(table_num), Some(area)) => Some((table_num, area)),
                _ => None,
            };
            (
                order.id,
                order.label.clone(),
                order.service,
                table_info,
                order.totals(),
                render_receipt(order, customer_mobile),
            )
        };

        match save_and_print(&bill_text, order_id) {
            Ok(msg) => {
                // Persist the paid bill (mobile + AC/GST breakdown) and
                // remove its open-order copy first.
                let mut db_error: Option<String> = None;
                if let Some(db) = &self.database {
                    if let Err(error) = db.save_paid_order(self.order(), mobile.trim(), &totals) {
                        db_error = Some(error);
                    }
                    let _ = db.delete_open_order(order_id);
                }
                if let Some(error) = db_error {
                    self.notify(format!("DB save failed: {error}"));
                }
                let summary = BillSummary {
                    id: order_id,
                    label: order_label.clone(),
                    service: order_service,
                    total: totals.total,
                    receipt: bill_text.clone(),
                };
                self.recent_bills.insert(0, summary);
                self.recent_bills.truncate(5);
                self.recent_bill_index = 0;
                self.notify(msg);

                self.order_mut().status = OrderStatus::Paid;

                // Bill settled: guests may still be seated, so the table shows
                // Paid until closed (then it goes to cleaning).
                if let Some((table_num, area)) = table_info {
                    if let Some(pt) = self
                        .physical_tables
                        .iter_mut()
                        .find(|t| t.area == area && t.number == table_num)
                    {
                        pt.status = TableStatus::Paid;
                    }
                    self.persist_table(area, table_num);
                }
                self.focus = self.focus_return;
            }
            Err(e) => self.notify(format!("Print failed: {e}")),
        }
    }

    // -- key handling ---------------------------------------------------------

    fn handle_key(&mut self, key: KeyCode) -> bool {
        // Global quit: q/Esc are captured by the billing prompts while open;
        // q is ignored while typing in the search box.
        if matches!(key, KeyCode::Char('q'))
            && self.focus != Focus::Search
            && self.focus != Focus::MobileEntry
            && self.focus != Focus::PaymentMode
        {
            return true;
        }
        if matches!(key, KeyCode::Esc)
            && self.focus != Focus::MobileEntry
            && self.focus != Focus::PaymentMode
        {
            return true;
        }

        if self.focus != Focus::Search {
            if matches!(key, KeyCode::Char(']')) {
                if !self.orders.is_empty() {
                    self.active_order = (self.active_order + 1) % self.orders.len();
                    self.notify(format!("Switched to {}.", self.order().label));
                }
                return false;
            }
            if matches!(key, KeyCode::Char('[')) {
                if !self.orders.is_empty() {
                    self.active_order =
                        self.active_order.saturating_add(self.orders.len() - 1) % self.orders.len();
                    self.notify(format!("Switched to {}.", self.order().label));
                }
                return false;
            }
        }

        match self.focus {
            Focus::Search => match key {
                KeyCode::Char(c) => {
                    self.search.push(c);
                    self.menu_index = 0;
                }
                KeyCode::Backspace => {
                    self.search.pop();
                    self.menu_index = 0;
                }
                KeyCode::Down => self.focus = Focus::Menu,
                KeyCode::Enter => self.add_selected_to_cart(),
                KeyCode::Tab => self.focus = Focus::Menu,
                KeyCode::BackTab => self.focus = Focus::RecentBills,
                _ => {}
            },
            Focus::Menu => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.menu_index = self.menu_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let n = self.visible_items().len();
                    if self.menu_index + 1 < n {
                        self.menu_index += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => self.add_selected_to_cart(),
                KeyCode::Tab => self.focus = Focus::Cart,
                KeyCode::BackTab => self.focus = Focus::Search,
                KeyCode::Char('/') => {
                    self.focus = Focus::Search;
                }
                KeyCode::Char('c') => self.clear_active_cart(),
                KeyCode::Char('p') => self.begin_billing(),
                KeyCode::Char('e') => {
                    let res = export_menu_csv(&self.data_file, &self.items);
                    match res {
                        Ok(n) => {
                            if let Some(db) = &self.database {
                                if let Err(error) = db.replace_menu(&self.items) {
                                    self.notify(format!("DB menu sync failed: {error}"));
                                    return false;
                                }
                            }
                            self.notify(format!(
                                "Exported {n} items to {}.",
                                self.data_file.display()
                            ));
                        }
                        Err(e) => self.notify(format!("Export failed: {e}")),
                    }
                }
                KeyCode::Char('i') => {
                    let res = import_menu_csv(&self.data_file);
                    match res {
                        Ok(n) => {
                            self.items = load_menu(&self.data_file).unwrap_or_default();
                            if let Some(db) = &self.database {
                                if let Err(error) = db.replace_menu(&self.items) {
                                    self.notify(format!("DB menu sync failed: {error}"));
                                    return false;
                                }
                            }
                            self.notify(format!(
                                "Imported {n} items from {}.",
                                self.data_file.display()
                            ));
                        }
                        Err(e) => self.notify(format!("Import failed: {e}")),
                    }
                }
                _ => {}
            },
            Focus::Cart => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    if !self.orders.is_empty() {
                        let o = self.order_mut();
                        o.cart_index = o.cart_index.saturating_sub(1);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.orders.is_empty() {
                        let o = self.order_mut();
                        if o.cart_index + 1 < o.cart.len() {
                            o.cart_index += 1;
                        }
                    }
                }
                KeyCode::Char('=') | KeyCode::Char('+') => self.adjust_selected_line_quantity(1),
                KeyCode::Char('-') => self.adjust_selected_line_quantity(-1),
                KeyCode::Delete | KeyCode::Char('x') => self.remove_selected_line(),
                KeyCode::Char('c') => self.clear_active_cart(),
                KeyCode::Enter | KeyCode::Char('p') => self.begin_billing(),
                KeyCode::Tab => {
                    self.focus = Focus::Tables;
                }
                KeyCode::BackTab => self.focus = Focus::Menu,
                _ => {}
            },
            Focus::Tables => match key {
                // ←/→ pick a table within the focused area bar
                KeyCode::Left | KeyCode::Char('h') => {
                    self.selected_table_index = self.selected_table_index.saturating_sub(1);
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    let max = self.selected_table_area.table_count();
                    if self.selected_table_index + 1 < max {
                        self.selected_table_index += 1;
                    }
                }
                // ↑/↓ switch between the 4 area bars
                KeyCode::Up | KeyCode::Char('k') => {
                    let areas = TableArea::all();
                    let current_idx = areas
                        .iter()
                        .position(|&a| a == self.selected_table_area)
                        .unwrap_or(0);
                    let new_idx = if current_idx == 0 { 3 } else { current_idx - 1 };
                    self.selected_table_area = areas[new_idx];
                    self.selected_table_index = self
                        .selected_table_index
                        .min(self.selected_table_area.table_count() - 1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let areas = TableArea::all();
                    let current_idx = areas
                        .iter()
                        .position(|&a| a == self.selected_table_area)
                        .unwrap_or(0);
                    let new_idx = (current_idx + 1) % 4;
                    self.selected_table_area = areas[new_idx];
                    self.selected_table_index = self
                        .selected_table_index
                        .min(self.selected_table_area.table_count() - 1);
                }
                KeyCode::Enter => {
                    self.open_table_order();
                }
                KeyCode::Char('t') => {
                    self.open_takeout_order();
                }
                KeyCode::Char('c') => {
                    self.close_order();
                }
                KeyCode::Char('s') => {
                    self.advance_stage();
                }
                KeyCode::Char('r') => {
                    self.clean_selected_table();
                }
                KeyCode::Char('u') => {
                    self.focus_return = self.focus;
                    self.focus = Focus::AdminMode;
                }
                KeyCode::Char('b') | KeyCode::Char('p') => self.begin_billing(),
                KeyCode::BackTab => self.focus = Focus::Cart,
                KeyCode::Tab => self.focus = Focus::RecentBills,
                KeyCode::Char(d @ '1'..='4') => {
                    if let Some(area) = TableArea::from_digit(d) {
                        self.selected_table_area = area;
                        self.selected_table_index = 0;
                    }
                }
                _ => {}
            },
            Focus::RecentBills => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.recent_bill_index = self.recent_bill_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.recent_bill_index + 1 < self.recent_bills.len() {
                        self.recent_bill_index += 1;
                    }
                }
                KeyCode::Tab => self.focus = Focus::Search,
                KeyCode::BackTab => self.focus = Focus::Tables,
                _ => {}
            },
            Focus::MobileEntry => {
                match key {
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        if self.mobile_buffer.len() < 10 {
                            self.mobile_buffer.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        self.mobile_buffer.pop();
                    }
                    KeyCode::Enter => {
                        if self.mobile_buffer.len() == 10 {
                            let mobile = std::mem::take(&mut self.mobile_buffer);
                            self.complete_billing(&mobile);
                        } else {
                            self.notify(format!(
                                "Mobile number needs 10 digits ({} so far).",
                                self.mobile_buffer.len()
                            ));
                        }
                    }
                    KeyCode::Esc => {
                        // Cancel billing; return to the panel that opened it.
                        self.focus = self.focus_return;
                    }
                    _ => {}
                }
            }
            Focus::PaymentMode => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.payment_mode_index = self.payment_mode_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let last = PaymentMode::all().len() - 1;
                    if self.payment_mode_index < last {
                        self.payment_mode_index += 1;
                    }
                }
                // Quick select: 1–5 picks a mode and closes at once.
                KeyCode::Char(d @ '1'..='5') => {
                    if let Some(mode) = PaymentMode::all().get((d as u8 - b'1') as usize) {
                        self.perform_close_with_mode(*mode);
                    }
                }
                KeyCode::Enter => {
                    if let Some(mode) = PaymentMode::all().get(self.payment_mode_index) {
                        self.perform_close_with_mode(*mode);
                    }
                }
                KeyCode::Esc => {
                    // Cancel closing; return to the panel that opened it.
                    self.focus = self.focus_return;
                }
                _ => {}
            },
            Focus::AdminMode => {
                if key == KeyCode::Esc {
                    self.focus = self.focus_return;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// CSV import / export
// ---------------------------------------------------------------------------

const CSV_HEADER: [&str; 4] = ["Category", "Item Name", "Unit", "Price"];

/// All receipts are saved into this single folder (created on demand).
const _BILLS_DIR: &str = "bills";

fn load_menu(path: &std::path::Path) -> io::Result<Vec<MenuItem>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(io::Error::other)?;
    let mut items = Vec::new();
    for row in rdr.records() {
        let rec = row.map_err(io::Error::other)?;
        let get = |i: usize| rec.get(i).unwrap_or("").trim().to_string();
        let price: f64 = get(3).parse().unwrap_or(0.0);
        if get(1).is_empty() || price <= 0.0 {
            continue;
        }
        items.push(MenuItem {
            category: get(0),
            name: get(1),
            unit: get(2),
            price,
        });
    }
    Ok(items)
}

fn export_menu_csv(path: &std::path::Path, items: &[MenuItem]) -> io::Result<usize> {
    let mut wtr = csv::Writer::from_path(path).map_err(io::Error::other)?;
    wtr.write_record(CSV_HEADER).map_err(io::Error::other)?;
    for it in items {
        wtr.write_record(it.to_csv_row())
            .map_err(io::Error::other)?;
    }
    wtr.flush()?;
    Ok(items.len())
}

fn import_menu_csv(path: &std::path::Path) -> io::Result<usize> {
    let items = load_menu(path)?;
    Ok(items.len())
}

// Kept only as a reference while migrating receipt handling to `receipts.rs`.
// It is excluded from the build and can be removed in the next cleanup pass.
#[cfg(any())]
mod legacy_receipts {
    use super::{BillSummary, MenuItem, Order, Service};
    use chrono::Local;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    /// Loads the five newest saved receipts so the recent-bills panel survives an
    /// application restart. Malformed or unrelated text files are ignored.
    fn load_recent_bills() -> Vec<BillSummary> {
        if !std::path::Path::new(BILLS_DIR).is_dir() {
            return Vec::new();
        }
        let mut bill_files: Vec<_> = fs::read_dir(BILLS_DIR)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let name = path.file_name()?.to_str()?;
                (name.starts_with("bill_") && name.ends_with(".txt")).then_some(path)
            })
            .collect();

        bill_files.sort_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        });
        bill_files.reverse();

        bill_files
            .into_iter()
            .filter_map(|path| parse_bill_summary(&path))
            .take(5)
            .collect()
    }

    fn parse_bill_summary(path: &std::path::Path) -> Option<BillSummary> {
        let contents = fs::read_to_string(path).ok()?;
        let bill_line = contents
            .lines()
            .find(|line| line.trim_start().starts_with("Bill #"))?;
        let id = bill_line
            .trim_start()
            .strip_prefix("Bill #")?
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()?;
        let label = bill_line
            .split_once("Table:")
            .map(|(_, label)| label.trim().to_string())
            .unwrap_or_else(|| "Saved receipt".to_string());
        let service = if contents.contains("Mode : TAKE-OUT") {
            Service::TakeOut
        } else {
            Service::DineIn
        };
        let total_line = contents
            .lines()
            .find(|line| line.trim_start().starts_with("TOTAL"))?;
        let total = total_line.split('₹').nth(1)?.trim().parse().ok()?;

        Some(BillSummary {
            id,
            label,
            service,
            total,
            receipt: contents,
        })
    }

    // ---------------------------------------------------------------------------
    // Bill printing
    // ---------------------------------------------------------------------------

    fn money(v: f64) -> String {
        format!("₹{:.2}", v)
    }

    /// Plain-text receipt (thermal-printer friendly, 42 columns).
    fn render_bill_text_for_order(_items: &[MenuItem], order: &Order) -> String {
        let mut out = String::new();
        out.push_str(&center("SHREE KRISHNA RESTAURANT", 42));
        out.push('\n');
        out.push_str(&center("Dine-In & Take-Out", 42));
        out.push('\n');
        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&format!("Bill #{:<6} Table: {}\n", order.id, order.label));
        out.push_str(&format!("{}\n", Local::now().format("%d-%m-%Y %H:%M")));
        out.push_str(&format!("Mode : {}\n", order.service.label()));
        out.push_str(&"-".repeat(42));
        out.push('\n');

        for line in &order.cart {
            out.push_str(&line.name);
            out.push('\n');
            out.push_str(&format!(
                "  {:>3} x {:>9} {:>14}\n",
                line.qty,
                money(line.unit_price),
                money(line.total())
            ));
        }

        let subtotal: f64 = order.cart.iter().map(|l| l.total()).sum();
        let tax = subtotal * order.service.tax_rate();

        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&format!("{:<22}{:>20}\n", "Subtotal", money(subtotal)));
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            format!("GST ({:.0}%)", order.service.tax_rate() * 100.0),
            money(tax)
        ));
        out.push_str(&format!("{:<22}{:>20}\n", "TOTAL", money(subtotal + tax)));
        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&center("Thank you! Visit again!", 42));
        out.push('\n');
        out.push('\n'); // feed so tear-off is clean
        out
    }

    fn center(s: &str, width: usize) -> String {
        let len = s.chars().count();
        if len >= width {
            return s.to_string();
        }
        let pad = (width - len) / 2;
        format!("{}{}", " ".repeat(pad), s)
    }

    /// Prints via `lp` when CUPS is available, otherwise saves a receipt file.
    fn print_bill_for_order(text: &str, order_no: u32) -> Result<String, String> {
        let stamp = Local::now().format("%Y%m%d_%H%M%S");
        fs::create_dir_all(BILLS_DIR).map_err(|e| e.to_string())?;
        let path = PathBuf::from(BILLS_DIR).join(format!("bill_{}_{}.txt", order_no, stamp));

        fs::write(&path, text).map_err(|e| e.to_string())?;

        match Command::new("lp").arg(path.to_str().unwrap_or("")).output() {
            Ok(out) if out.status.success() => Ok(format!(
                "Bill #{} printed and saved to {}.",
                order_no,
                path.display()
            )),
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr);
                Ok(format!(
                    "Saved to {} (printer not available: {}).",
                    path.display(),
                    err.trim()
                ))
            }
            Err(_) => Ok(format!(
                "Saved to {} (no printer found on this system).",
                path.display()
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    loop {
        app.tick_notification();
        let cleaned = app.tick_cleaning();
        if cleaned > 0 {
            app.notify(format!("{cleaned} table(s) cleaned and back to Ready."));
        }
        terminal.draw(|f| ui(f, &app))?;

        // Poll so the 10-minute banner can expire even without keypresses.
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && app.handle_key(key.code) {
                    return Ok(());
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    // A normal layout needs 33 rows to show the table map, search, bill list,
    // notification, five recent bills, and the complete shortcut guide. On a
    // smaller terminal, reserve space for the order panels first and hide the
    // history rather than allowing the lower panels to crowd the menu.
    let compact = f.area().height < 33;
    let [tabs_area, search_area, body, notification_area, recent_bills_area, footer_area] =
        Layout::vertical(if compact {
            [
                Constraint::Length(8),
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(2),
                Constraint::Length(0),
                Constraint::Length(2),
            ]
        } else {
            [
                Constraint::Length(8),
                Constraint::Length(3),
                Constraint::Fill(1),
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Length(4),
            ]
        })
        .areas(f.area());

    let [menu_area, cart_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(body);

    render_tabs(f, app, tabs_area);
    render_search(f, app, search_area);
    render_menu(f, app, menu_area);
    render_bill(f, app, cart_area);
    render_notification(f, app, notification_area);
    if !compact {
        render_recent_bills(f, app, recent_bills_area);
    }
    render_footer(f, app, footer_area);
    if app.focus == Focus::MobileEntry {
        render_mobile_entry(f, app);
    }
    if app.focus == Focus::PaymentMode {
        render_payment_mode(f, app);
    }
}

/// A fixed-size rectangle centred inside `outer`.
fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(outer);
    Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0])[0]
}

/// Modal prompt that captures the customer's 10-digit mobile number during
/// billing.
fn render_mobile_entry(f: &mut Frame, app: &App) {
    let area = centered_rect(46, 7, f.area());
    f.render_widget(Clear, area);

    let complete = app.mobile_buffer.len() == 10;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Customer mobile ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(if complete {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Cyan)
        });

    // Group the digits 5 + 5 for readability; pad with underscores.
    let mut padded: String = app.mobile_buffer.clone();
    for _ in app.mobile_buffer.len()..10 {
        padded.push('_');
    }
    let display = format!("{} {}", &padded[..5], &padded[5..]);

    let lines = vec![
        Line::styled(
            "Enter the customer's mobile number",
            Style::default().fg(Color::DarkGray),
        ),
        Line::from(Span::styled(
            format!("   {display}|   "),
            Style::default()
                .fg(if complete {
                    Color::Green
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        )),
        Line::styled(
            "Enter: generate bill · Backspace: edit · Esc: cancel",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .split(inner)
    .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line).alignment(Alignment::Center), rect);
    }
}

/// Modal confirmation of the mode of payment, shown when a paid order is
/// about to be closed.
fn render_payment_mode(f: &mut Frame, app: &App) {
    let modes = PaymentMode::all();
    let area = centered_rect(46, (modes.len() as u16) + 7, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Mode of payment ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let header = match app.orders.get(app.active_order) {
        Some(order) => format!(
            "Close {} · Bill #{} — {}",
            order.label,
            order.id,
            money(order.totals().total)
        ),
        None => String::from("No order selected"),
    };
    let mut lines = vec![Line::styled(
        header,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )];
    for (idx, mode) in modes.iter().enumerate() {
        let selected = idx == app.payment_mode_index;
        let marker = if selected { "▸ " } else { "  " };
        let line_text = format!("{marker}{}. {:<16}", idx + 1, mode.display());
        lines.push(Line::from(Span::styled(
            line_text,
            if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }
    lines.push(Line::styled(
        "↑↓/1–5: select · Enter: confirm & close · Esc: cancel",
        Style::default().fg(Color::DarkGray),
    ));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical(vec![Constraint::Length(1); lines.len()])
        .flex(Flex::Center)
        .split(inner)
        .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line), rect);
    }
}

fn render_notification(f: &mut Frame, app: &App, area: Rect) {
    if app.notification.is_empty() {
        return;
    }
    let msg = app.notification.as_str();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Notice ")
        .border_style(Style::default().fg(Color::Green));
    f.render_widget(
        Paragraph::new(msg)
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .block(block),
        area,
    );
}

fn render_recent_bills(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();
    let title_style = if app.focus == Focus::RecentBills {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    };

    let title = Span::styled(" Recent bills (latest 5) — ↑/↓: review ", title_style);

    if app.recent_bills.is_empty() {
        lines.push(Line::styled(
            " No completed bills yet.",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        lines.extend(app.recent_bills.iter().enumerate().map(|(index, bill)| {
            let line = format!(
                " Bill #{:<4} {:<14} {:<10} {}",
                bill.id,
                bill.label,
                bill.service.label(),
                money(bill.total)
            );
            if app.focus == Focus::RecentBills && index == app.recent_bill_index {
                Line::styled(
                    line,
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::raw(line)
            }
        }));
    }

    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}

/// One-line legend entry for a lifecycle stage.
fn legend_span(glyph: char, label: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!("{glyph} {label}  "),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

/// Renders the floor plan: one bar per area with colour-coded status cards
/// (`R1` = table 1 Ready), plus take-out chips and a lifecycle legend.
fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let active = app.focus == Focus::Tables;
    let border_style = if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let title_style = if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Floor plan — ↑↓: area · ←→: table · 1–4: jump · Enter: open/switch/clean · s: next stage ",
            title_style,
        ))
        .border_style(border_style);

    // Render the bordered block (title + frame) first, then lay out bars inside it.
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let rows: Vec<Rect> = Layout::vertical([
        Constraint::Length(1), // Front Garden
        Constraint::Length(1), // AC Rooms
        Constraint::Length(1), // Main Hall
        Constraint::Length(1), // Back Garden
        Constraint::Length(1), // TakeOut
        Constraint::Length(1), // Legend
    ])
    .split(inner)
    .to_vec();

    let areas = TableArea::all();
    let now = Local::now();

    // One horizontal bar per physical area.
    for (bar_i, area_type) in areas.iter().enumerate() {
        let is_focused_bar = active && app.selected_table_area == *area_type;

        // Free-seating summary for the area label.
        let free = app
            .physical_tables
            .iter()
            .filter(|t| t.area == *area_type && t.status == TableStatus::Ready)
            .count();
        let total = area_type.table_count();

        let (label_fg, label_mod) = if is_focused_bar {
            (Color::Yellow, Modifier::BOLD | Modifier::REVERSED)
        } else {
            (Color::DarkGray, Modifier::BOLD)
        };
        let mut spans = vec![
            Span::styled(
                format!(" {:<12}", area_type.label()),
                Style::default().fg(label_fg).add_modifier(label_mod),
            ),
            Span::styled(
                format!("{free}/{total} "),
                Style::default().fg(if free > 0 { Color::Green } else { Color::Red }),
            ),
        ];

        for table_num in 1..=total {
            let table = app
                .physical_tables
                .iter()
                .find(|t| t.area == *area_type && t.number == table_num);

            let status = table.map_or(TableStatus::Ready, |t| t.status);
            let glyph = match status {
                TableStatus::Ready => 'R',
                TableStatus::Ordering => 'O',
                TableStatus::Serving => 'S',
                TableStatus::BillRequested => 'B',
                TableStatus::Paid => 'P',
                TableStatus::Dirty => 'C',
            };
            let status_color = status.color();

            // Dirty tables count down to auto-ready.
            let countdown = if status == TableStatus::Dirty {
                table.and_then(|t| t.dirty_since).map(|since| {
                    let elapsed = now - since;
                    let remaining = chrono::Duration::minutes(CLEANING_MINUTES) - elapsed;
                    let mins = remaining.num_minutes().max(0);
                    format!(" {mins}m")
                })
            } else {
                None
            };

            let is_selected = is_focused_bar && app.selected_table_index == table_num - 1;

            let cell_text = match &countdown {
                Some(mins) => format!("{glyph}{table_num}{mins}"),
                None => format!("{glyph}{table_num}"),
            };
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("[{cell_text}]"),
                if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD)
                },
            ));
        }

        f.render_widget(Line::from(spans), rows[bar_i]);
    }

    // Take-out bar (virtual tables).
    let tk_active = active
        && app
            .orders
            .get(app.active_order)
            .is_some_and(|o| o.service == Service::TakeOut);
    let tk_label_style = if tk_active {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    };
    let mut tk_spans = vec![Span::styled(format!(" {:<12}", "TakeOut"), tk_label_style)];
    let has_takeout = app.orders.iter().any(|o| o.service == Service::TakeOut);
    for (idx, order) in app.orders.iter().enumerate() {
        if order.service != Service::TakeOut {
            continue;
        }
        let is_active = app.active_order == idx;
        let style = if is_active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Magenta)
        };
        let label = order.label.clone();
        tk_spans.push(Span::styled(format!(" [{label}] "), style));
    }
    if !has_takeout {
        tk_spans.push(Span::styled(
            " (none — press t)",
            Style::default().fg(Color::DarkGray),
        ));
    }
    f.render_widget(Line::from(tk_spans), rows[4]);

    // Lifecycle legend.
    let legend = Line::from(vec![
        Span::raw(" "),
        legend_span('R', "Ready", TableStatus::Ready.color()),
        legend_span('O', "Ordering", TableStatus::Ordering.color()),
        legend_span('S', "Serving", TableStatus::Serving.color()),
        legend_span('B', "For bill", TableStatus::BillRequested.color()),
        legend_span('P', "Paid", TableStatus::Paid.color()),
        legend_span(
            'C',
            &format!("Cleaning ({CLEANING_MINUTES}m)"),
            TableStatus::Dirty.color(),
        ),
    ]);
    f.render_widget(legend, rows[5]);
}

fn render_search(f: &mut Frame, app: &App, area: Rect) {
    let active = app.focus == Focus::Search;
    let title_style = if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            if active { " Search (/) " } else { " Search " },
            title_style,
        ))
        .border_style(if active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let cursor = if active { "|" } else { "" };
    let text = Text::from(Line::from(format!(" {}{}", app.search, cursor)));
    f.render_widget(Paragraph::new(text).block(block), area);

    // Show match count in the top-right corner.
    let count = app.visible_items().len();
    let info = Paragraph::new(format!("{} matches ", count))
        .alignment(Alignment::Right)
        .style(Style::default().fg(Color::DarkGray));
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    f.render_widget(info, inner);
}

fn render_menu(f: &mut Frame, app: &App, area: Rect) {
    let vis = app.visible_items();

    let rows: Vec<Row> = vis
        .iter()
        .enumerate()
        .map(|(row_i, &idx)| {
            let it = &app.items[idx];
            let selected = row_i == app.menu_index && app.focus == Focus::Menu;
            Row::new(vec![
                Cell::from(it.name.clone()),
                Cell::from(Text::from(it.unit.clone()).alignment(Alignment::Right)),
                Cell::from(Text::from(money(it.price)).alignment(Alignment::Right)),
            ])
            .style(if selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Percentage(30),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec!["Item", "Unit", "Price"]).bold().underlined())
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        " Menu ",
        if app.focus == Focus::Menu {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    )));

    f.render_widget(table, area);
}

fn render_bill(f: &mut Frame, app: &App, area: Rect) {
    if app.focus == Focus::RecentBills {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Previous bill — read only ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        let receipt = app
            .recent_bills
            .get(app.recent_bill_index)
            .map(|bill| bill.receipt.as_str())
            .unwrap_or("No saved bills to review.");
        f.render_widget(Paragraph::new(receipt).block(block), area);
        return;
    }

    if app.orders.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Cart ", Style::default().fg(Color::DarkGray)));
        f.render_widget(Paragraph::new("No active order").block(block), area);
        return;
    }

    let order = app.order();
    let paid = matches!(order.status, OrderStatus::Paid);
    let serving = matches!(order.status, OrderStatus::Serving);
    let bill_ready = matches!(order.status, OrderStatus::BillRequested);
    let title = if paid {
        format!(
            " Bill #{} — PAID ✔ ({} / {}) ",
            order.id,
            order.label,
            order.service.label()
        )
    } else if bill_ready {
        format!(
            " Bill #{} — READY FOR BILL ⏳ ({} / {}) ",
            order.id,
            order.label,
            order.service.label()
        )
    } else if serving {
        format!(
            " Bill #{} — SERVING ★ ({} / {}) ",
            order.id,
            order.label,
            order.service.label()
        )
    } else {
        format!(
            " Bill #{} — {} / {} ",
            order.id,
            order.label,
            order.service.label()
        )
    };

    let rows: Vec<Row> = order
        .cart
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let selected = i == order.cart_index && app.focus == Focus::Cart;
            Row::new(vec![
                Cell::from(line.name.clone()),
                Cell::from(Text::from(format!("{}", line.qty)).alignment(Alignment::Right)),
                Cell::from(Text::from(money(line.unit_price)).alignment(Alignment::Right)),
                Cell::from(Text::from(money(line.total())).alignment(Alignment::Right)),
            ])
            .style(if selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let widths = [
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Length(8),
        Constraint::Length(9),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Item", "Qty", "Each", "Total"])
                .bold()
                .underlined(),
        )
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            title,
            if paid {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if app.focus == Focus::Cart {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        )));

    f.render_widget(table, area);

    // Totals pinned to the bottom, inside the same block borders.
    let totals = order.totals();
    let mut total_lines = vec![Line::from(Span::raw(format!(
        " {:<16}{:>12}",
        "Subtotal",
        money(totals.subtotal)
    )))];
    if totals.ac_charge > 0.0 {
        total_lines.push(Line::from(Span::styled(
            format!(
                " {:<16}{:>12}",
                format!("AC charge ({:.0}%)", order.ac_surcharge() * 100.0),
                money(totals.ac_charge)
            ),
            Style::default().fg(Color::Cyan),
        )));
    }
    total_lines.push(Line::from(Span::raw(format!(
        " {:<16}{:>12}",
        format!("GST ({:.0}%)", totals.gst_rate * 100.0),
        money(totals.gst)
    ))));
    total_lines.push(Line::from(Span::styled(
        format!(" {:<16}{:>12}", "TOTAL", money(totals.total)),
        Style::default().add_modifier(Modifier::BOLD),
    )));

    const TOTALS_LINES: u16 = 4;
    let totals_para = Paragraph::new(total_lines);
    let bottom = Rect {
        x: area.x + 1,
        y: area.bottom().saturating_sub(TOTALS_LINES + 1),
        width: area.width.saturating_sub(2),
        height: TOTALS_LINES,
    };
    f.render_widget(totals_para, bottom);
}

fn render_footer(f: &mut Frame, _app: &App, area: Rect) {
    let help = if area.height < 4 {
        vec![Line::from(" Tab: focus · /: search · p: bill · q: quit ")]
    } else {
        vec![
            Line::from(" Tab/Shift+Tab: focus · ↑↓/j/k: select · /: search · [:] switch order "),
            Line::from(
                " Floor plan: Enter open/switch/clean · s next stage · t takeout · c close paid ",
            ),
            Line::from(
                " Bill: +/- qty · x remove line · c clear · p bill (mobile + payment mode) ",
            ),
            Line::from(" e/i menu export/import (DB synced) · q/Esc quit "),
        ]
    };
    f.render_widget(Paragraph::new(help).style(Style::default().dim()), area);
}
