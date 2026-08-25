use std::io;
use std::path::PathBuf;
use std::time::Duration;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use chrono::Local;
mod models;
mod receipts;

use models::{
    BillSummary, CartLine, Focus, MenuItem, Order, OrderStatus, PhysicalTable, Service, TableArea,
    TableStatus,
};
use receipts::{load_recent, money, next_bill_number, render_receipt, save_and_print};

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
            tables.push(PhysicalTable {
                number,
                area: *area,
                status: TableStatus::Empty,
                order_id: None,
            });
        }
    }
    tables
}

impl App {
    fn new() -> Self {
        let data_file = PathBuf::from("menu.csv");
        let items = match load_menu(&data_file) {
            Ok(items) if !items.is_empty() => items,
            _ => default_menu(),
        };
        let recent_bills = load_recent();
        let next_order_id = next_bill_number(&recent_bills);
        Self {
            items,
            orders: Vec::new(),
            active_order: 0,
            next_order_id,
            next_takeout_id: 1,
            physical_tables: init_physical_tables(),
            menu_index: 0,
            search: String::new(),
            focus: Focus::Menu,
            data_file,
            matcher: SkimMatcherV2::default().ignore_case(),
            selected_table_area: TableArea::FrontGarden,
            selected_table_index: 0,
            notification: String::from("Loaded menu."),
            notification_until: Some(Local::now() + chrono::Duration::minutes(10)),
            recent_bills,
            recent_bill_index: 0,
        }
    }

    // -- order (table) management --------------------------------------------

    fn order(&self) -> &Order {
        &self.orders[self.active_order]
    }

    fn order_mut(&mut self) -> &mut Order {
        &mut self.orders[self.active_order]
    }

    /// Opens order for the selected physical table (dine-in only).
    fn open_table_order(&mut self) {
        // Get table info without holding reference
        let table_info = self
            .physical_tables
            .iter()
            .find(|t| {
                t.area == self.selected_table_area && t.number == self.selected_table_index + 1
            })
            .map(|t| (t.area, t.number, t.status));

        if let Some((area, table_num, status)) = table_info {
            if status != TableStatus::Empty {
                if let Some(order_id) = self
                    .physical_tables
                    .iter()
                    .find(|t| t.area == area && t.number == table_num)
                    .and_then(|t| t.order_id)
                {
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
                pt.status = TableStatus::HasOrder;
                pt.order_id = Some(id);
            }

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
        self.notify(format!("Opened take-out order TK{}.", tk_count));
    }

    /// Closes the active order and clears its table.
    fn close_order(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No orders to close."));
            return;
        }

        let order = self.order();
        if order.status != OrderStatus::Paid {
            self.notify(format!(
                "Generate the bill for {} before closing it.",
                order.label
            ));
            return;
        }
        let label = order.label.clone();
        let table_info = if let (Some(table_num), Some(area)) = (order.table_number, order.area) {
            Some((table_num, area))
        } else {
            None
        };

        self.orders.remove(self.active_order);
        if self.active_order >= self.orders.len() && !self.orders.is_empty() {
            self.active_order = self.orders.len() - 1;
        }

        // Clear physical table if applicable
        if let Some((table_num, area)) = table_info {
            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == area && t.number == table_num)
            {
                pt.status = TableStatus::Empty;
                pt.order_id = None;
            }
        }

        self.notify(format!("Closed {}.", label));
    }

    /// Mark current order as serving (change status from Ordering to Serving).
    fn start_serving(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No active order."));
            return;
        }

        let order_info = {
            let order = self.order();
            (order.status, order.table_number, order.area)
        };

        if let (OrderStatus::Ordering, Some(table_num), Some(area)) = order_info {
            self.order_mut().status = OrderStatus::Serving;

            // Update table color to blue (serving)
            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == area && t.number == table_num)
            {
                pt.status = TableStatus::Serving;
            }

            let label = self.order().label.clone();
            self.notify(format!("Order {} is now being served.", label));
        } else {
            self.notify(String::from(
                "Order is already paid or not a dine-in order.",
            ));
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
            self.notify(format!("Removed {}.", name));
        }
    }

    /// Changes the quantity of the selected cart line. A quantity reduced to
    /// zero is removed from the bill.
    fn adjust_selected_line_quantity(&mut self, change: i32) {
        if !self.ensure_editable_order() {
            return;
        }

        let order = self.order_mut();
        if order.cart_index >= order.cart.len() {
            self.notify(String::from("No item selected in the bill."));
            return;
        }

        if change > 0 {
            order.cart[order.cart_index].qty += change as u32;
            let name = order.cart[order.cart_index].name.clone();
            self.notify(format!("Increased {name}."));
        } else if order.cart[order.cart_index].qty > 1 {
            order.cart[order.cart_index].qty -= (-change) as u32;
            let name = order.cart[order.cart_index].name.clone();
            self.notify(format!("Decreased {name}."));
        } else {
            self.remove_selected_line();
        }
    }

    fn clear_active_cart(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }
        let order = self.order_mut();
        order.cart.clear();
        order.cart_index = 0;
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

    fn checkout(&mut self) {
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

        let order = self.order();
        if order.cart.is_empty() {
            self.notify(String::from("Cart is empty."));
            return;
        }

        let bill_text = render_receipt(order);
        match save_and_print(&bill_text, order.id) {
            Ok(msg) => {
                let summary = BillSummary {
                    id: order.id,
                    label: order.label.clone(),
                    service: order.service,
                    total: order.cart.iter().map(|line| line.total()).sum::<f64>()
                        * (1.0 + order.service.tax_rate()),
                    receipt: bill_text.clone(),
                };
                self.recent_bills.insert(0, summary);
                self.recent_bills.truncate(5);
                self.recent_bill_index = 0;
                self.notify(msg);
                self.order_mut().status = OrderStatus::Paid;
            }
            Err(e) => self.notify(format!("Print failed: {e}")),
        }
    }

    // -- key handling ---------------------------------------------------------

    fn handle_key(&mut self, key: KeyCode) -> bool {
        // Global quit.
        if matches!(key, KeyCode::Char('q') | KeyCode::Esc)
            && (self.focus != Focus::Search || matches!(key, KeyCode::Esc))
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
                KeyCode::Char('p') => self.checkout(),
                KeyCode::Char('e') => {
                    let res = export_menu_csv(&self.data_file, &self.items);
                    match res {
                        Ok(n) => self.notify(format!(
                            "Exported {n} items to {}.",
                            self.data_file.display()
                        )),
                        Err(e) => self.notify(format!("Export failed: {e}")),
                    }
                }
                KeyCode::Char('i') => {
                    let res = import_menu_csv(&self.data_file);
                    match res {
                        Ok(n) => {
                            self.items = load_menu(&self.data_file).unwrap_or_default();
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
                KeyCode::Enter | KeyCode::Char('p') => self.checkout(),
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
                    self.start_serving();
                }
                KeyCode::Char('b') | KeyCode::Char('p') => self.checkout(),
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
            " Tables — ↑/↓: area · ←/→: table · 1–4: jump · Enter: open · t: takeout · s: serve · c: close ",
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
    ])
    .split(inner)
    .to_vec();

    let areas = TableArea::all();

    // One horizontal bar per physical area.
    for (bar_i, area_type) in areas.iter().enumerate() {
        let is_focused_bar = active && app.selected_table_area == *area_type;

        let label_style = if is_focused_bar {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        };

        let mut spans = vec![Span::styled(
            format!(" {:<12}", area_type.label()),
            label_style,
        )];

        for table_num in 1..=area_type.table_count() {
            let status = app
                .physical_tables
                .iter()
                .find(|t| t.area == *area_type && t.number == table_num)
                .map(|t| t.status);

            let (color, symbol) = match status {
                Some(TableStatus::Empty) | None => (Color::Green, "Ready:"),
                Some(TableStatus::HasOrder) => (Color::Red, "HasOrder:"),
                Some(TableStatus::Serving) => (Color::Blue, "Serving:"),
            };

            let is_selected = is_focused_bar && app.selected_table_index == table_num - 1;

            let bg = if is_selected {
                Color::White
            } else {
                Color::Reset
            };
            let fg = if is_selected { Color::Black } else { color };

            spans.push(Span::styled(
                format!(" {}{} ", symbol, table_num),
                Style::default().fg(fg).bg(bg).add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
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
        tk_spans.push(Span::styled(format!(" [{}] ", order.label), style));
    }
    if !has_takeout {
        tk_spans.push(Span::styled(
            " (none)",
            Style::default().fg(Color::DarkGray),
        ));
    }
    f.render_widget(Line::from(tk_spans), rows[4]);
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
    let title = if paid {
        format!(
            " Bill #{} — PAID ✔ ({} / {}) ",
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
    const TOTALS_LINES: u16 = 3;
    let subtotal: f64 = order.cart.iter().map(|l| l.total()).sum();
    let tax = subtotal * order.service.tax_rate();
    let lines = vec![
        Line::from(Span::raw(format!(
            " {:<16}{:>12}",
            "Subtotal",
            money(subtotal)
        ))),
        Line::from(Span::raw(format!(
            " {:<16}{:>12}",
            format!("Tax ({:.0}%)", order.service.tax_rate() * 100.0),
            money(tax)
        ))),
        Line::from(Span::styled(
            format!(" {:<16}{:>12}", "TOTAL", money(subtotal + tax)),
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    let totals = Paragraph::new(lines);
    let bottom = Rect {
        x: area.x + 1,
        y: area.bottom().saturating_sub(TOTALS_LINES + 1),
        width: area.width.saturating_sub(2),
        height: TOTALS_LINES,
    };
    f.render_widget(totals, bottom);
}

fn render_footer(f: &mut Frame, _app: &App, area: Rect) {
    let help = if area.height < 4 {
        vec![Line::from(" Tab: focus · /: search · p: bill · q: quit ")]
    } else {
        vec![
            Line::from(" Tab/Shift+Tab: focus · ↑↓/j/k: select · /: search "),
            Line::from(" ←→/h/l: table · Enter: open, switch, add, or pay "),
            Line::from(" Bill: +/- qty · x: remove line · c: clear all · p/Enter: checkout "),
            Line::from(" s: serve · c: close paid order · e/i: menu export/import · q/Esc: quit "),
        ]
    };
    f.render_widget(Paragraph::new(help).style(Style::default().dim()), area);
}
