//! Application state management, user actions, and event routing.

use std::{
    path::{Path, PathBuf},
};

use chrono::Local;
use crossterm::event::KeyCode;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::{
    config::{
        default_menu, export_areas_csv, export_config_csv, export_menu_csv, export_offers_csv,
        load_areas_csv, load_config_csv, load_menu, load_offers_csv,
    },
    db::Database,
    models::{
        Area, BillSummary, CartLine, Focus, MenuItem, Offer, Order, OrderStatus, PaymentMode,
        PhysicalTable, Service, TableStatus, CLEANING_MINUTES,
    },
    receipts::{render_receipt},
};

pub const DB_PATH: &str = "data/billing.db";

pub struct App {
    pub items: Vec<MenuItem>,
    pub orders: Vec<Order>,  // all open orders (dine-in & take-out)
    pub active_order: usize, // index into `orders`
    pub next_order_id: u32,
    pub next_takeout_id: u32,
    pub physical_tables: Vec<PhysicalTable>, // all physical tables by area
    pub areas: Vec<Area>,                    // configurable dining areas (CSV-managed)
    pub gst_number: String,                  // registered GSTIN printed on receipts
    pub ac_rate: f64,                        // AC surcharge rate (fraction) for AC areas
    pub offers: Vec<Offer>,                  // discount offers selectable at billing
    pub menu_index: usize,
    pub search: String,
    pub focus: Focus,
    pub data_file: PathBuf,
    pub matcher: SkimMatcherV2,
    pub selected_area_index: usize,  // current area when navigating tables
    pub selected_table_index: usize, // current table index within area
    pub notification: String,        // top-left banner message
    pub notification_until: Option<chrono::DateTime<Local>>, // dismisses after this time
    pub recent_bills: Vec<BillSummary>, // newest first, up to five completed bills
    pub recent_bill_index: usize,
    pub focus_return: Focus,        // panel to restore when the prompt closes
    pub database: Option<Database>, // None only when the DB could not be opened
    pub mobile_buffer: String,      // customer mobile captured in the billing prompt
    pub payment_mode_index: usize,  // selection inside the payment-mode popup
    pub offer_index: usize,         // selection inside the offer-selection popup
    pub pending_mobile: String,     // customer mobile captured before offer pick
}

impl App {
    pub fn new() -> Self {
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

        // Restore config, unpaid orders and physical table states from the last run.
        let now = Local::now();
        let (areas, gst_number, ac_rate, offers) = match &database {
            Some(db) => (
                db.load_areas(),
                db.get_setting("gst_number"),
                db.get_setting("ac_rate").parse().unwrap_or(0.06),
                db.load_offers(),
            ),
            None => (Area::defaults(), String::new(), 0.06, Vec::new()),
        };

        let (orders, next_order_id, physical_tables) = match &database {
            Some(db) => {
                let orders = db.load_open_orders();
                let stored_tables = db.load_tables(&areas, now);
                let mut tables = init_physical_tables(&areas);
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
                    if let (Some(area), Some(number)) = (order.area.clone(), order.table_number) {
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
                let max_open_id = orders.iter().map(|o| o.id).max().unwrap_or(0);
                let bill_number = db.next_bill_number().max(max_open_id.saturating_add(1));
                (orders, bill_number, tables)
            }
            None => {
                let recent_bills: Vec<BillSummary> = Vec::new();
                (
                    Vec::new(),
                    1,
                    init_physical_tables(&areas),
                )
            }
        };

        let recent_bills = if let Some(db) = &database {
            db.load_recent_bills()
        } else {
            Vec::new()
        };
        let next_takeout_id = max_takeout_number(&orders).saturating_add(1);

        Self {
            items,
            orders: orders.to_vec(),
            active_order: 0,
            next_order_id,
            next_takeout_id,
            physical_tables,
            areas,
            gst_number,
            ac_rate,
            offers,
            menu_index: 0,
            search: String::new(),
            focus: Focus::Menu,
            data_file,
            matcher: SkimMatcherV2::default().ignore_case(),
            selected_area_index: 0,
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
            offer_index: 0,
            pending_mobile: String::new(),
        }
    }

    pub fn order(&self) -> &Order {
        &self.orders[self.active_order]
    }

    pub fn order_mut(&mut self) -> &mut Order {
        &mut self.orders[self.active_order]
    }

    pub fn selected_area(&self) -> Option<&Area> {
        self.areas.get(self.selected_area_index)
    }

    pub fn selected_area_name(&self) -> String {
        self.selected_area()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    pub fn persist_table(&self, area: &str, number: usize) {
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

    pub fn persist_active_order(&self) {
        if let Some(db) = &self.database {
            if let Some(order) = self.orders.get(self.active_order) {
                if let Err(error) = db.save_open_order(order) {
                    eprintln!("Order persist failed: {error}");
                }
            }
        }
    }

    pub fn tick_cleaning(&mut self) -> usize {
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

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notification = msg.into();
        self.notification_until = Some(Local::now() + chrono::Duration::minutes(10));
    }

    pub fn tick_notification(&mut self) {
        if let Some(until) = self.notification_until {
            if Local::now() >= until {
                self.notification.clear();
                self.notification_until = None;
            }
        }
    }

    pub fn clean_selected_table(&mut self) {
        let area = self.selected_area_name();
        let target = self
            .physical_tables
            .iter()
            .find(|t| t.area == area && t.number == self.selected_table_index + 1)
            .map(|t| (t.area.clone(), t.number, t.status));

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
            self.persist_table(&area, table_num);
            self.notify(format!("Table {table_num} cleaned and ready."));
        }
    }

    pub fn open_table_order(&mut self) {
        let area = self.selected_area_name();
        if area.is_empty() {
            self.notify(String::from("No dining areas configured."));
            return;
        }

        let table_info = self
            .physical_tables
            .iter()
            .find(|t| t.area == area && t.number == self.selected_table_index + 1)
            .map(|t| (t.area.clone(), t.number, t.status, t.order_id));

        if let Some((area, table_num, status, order_id)) = table_info {
            if status == TableStatus::Dirty {
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
                area.split_whitespace().next().unwrap_or("T"),
                table_num
            );

            let (is_ac, ac_rate) = self
                .selected_area()
                .map(|a| (a.is_ac, self.ac_rate))
                .unwrap_or((false, 0.0));

            let order = Order {
                id,
                label,
                service: Service::DineIn,
                table_number: Some(table_num),
                area: Some(area.clone()),
                is_ac,
                ac_rate,
                discount_percent: 0.0,
                cart: Vec::new(),
                cart_index: 0,
                status: OrderStatus::Ordering,
            };

            self.orders.push(order);
            self.active_order = self.orders.len() - 1;

            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == area && t.number == table_num)
            {
                pt.status = TableStatus::Ordering;
                pt.order_id = Some(id);
            }

            self.persist_active_order();
            self.persist_table(&area, table_num);
            self.notify(format!("Opened order for Table {}", table_num));
        }
    }

    pub fn open_takeout_order(&mut self) {
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
            is_ac: false,
            ac_rate: 0.0,
            discount_percent: 0.0,
            cart: Vec::new(),
            cart_index: 0,
            status: OrderStatus::Ordering,
        };

        self.orders.push(order);
        self.active_order = self.orders.len() - 1;
        self.persist_active_order();
        self.notify(format!("Opened take-out order TK{}.", tk_count));
    }

    pub fn close_order(&mut self) {
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

    pub fn perform_close_with_mode(&mut self, mode: PaymentMode) {
        if self.orders.is_empty() || self.order().status != OrderStatus::Paid {
            return;
        }

        let order = self.order();
        let label = order.label.clone();
        let table_info =
            if let (Some(table_num), Some(area)) = (order.table_number, order.area.clone()) {
                Some((table_num, area))
            } else {
                None
            };
        let closed_id = order.id;

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

        if let Some(db) = &self.database {
            let _ = db.delete_open_order(closed_id);
        }

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
            self.persist_table(&area, table_num);
            self.notify(format!(
                "Closed {label} via {mode_display}. Table {table_num} cleaning — auto-ready in {CLEANING_MINUTES} min."
            ));
            return;
        }

        self.notify(format!("Closed {label} via {mode_display}."));
    }

    pub fn advance_stage(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No active order."));
            return;
        }

        let (current, table_number, area) = {
            let order = self.order();
            (order.status, order.table_number, order.area.clone())
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
                    self.persist_table(&area, table_num);
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

    pub fn visible_items(&self) -> Vec<usize> {
        let q = self.search.trim();
        if q.is_empty() {
            return (0..self.items.len()).collect();
        }
        let mut scored: Vec<(i64, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                let name_score = self.matcher.fuzzy_match(&it.name, q);
                let cat_score = self.matcher.fuzzy_match(&it.category, q).map(|s| s / 2);
                name_score.or(cat_score).map(|s| (s, i))
            })
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        scored.into_iter().map(|(_, i)| i).collect()
    }

    pub fn add_selected_to_cart(&mut self) {
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

    pub fn remove_selected_line(&mut self) {
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

    pub fn adjust_selected_line_quantity(&mut self, change: i32) {
        if !self.ensure_editable_order() {
            return;
        }

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

    pub fn clear_active_cart(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }
        self.order_mut().cart.clear();
        let order = self.order_mut();
        order.cart_index = 0;
        self.persist_active_order();
        self.notify("Cart cleared.");
    }

    pub fn ensure_editable_order(&mut self) -> bool {
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

    pub fn begin_billing(&mut self) {
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

    pub fn complete_billing(&mut self, mobile: &str, offer_id: Option<u32>) {
        if self.orders.is_empty() || self.order().status == OrderStatus::Paid {
            return;
        }
        if self.order().cart.is_empty() {
            self.notify(String::from("Cart is empty."));
            return;
        }

        if let Some(id) = offer_id {
            if let Some(offer) = self.offers.iter().find(|o| o.id == id) {
                self.order_mut().discount_percent = offer.discount_percent;
            }
        }

        let customer_mobile = if mobile.trim().is_empty() {
            None
        } else {
            Some(mobile.trim())
        };
        let gst_number = self.gst_number.clone();
        let (order_id, order_label, order_service, table_info, totals, bill_text) = {
            let order = self.order();
            let table_info = match (order.table_number, order.area.clone()) {
                (Some(table_num), Some(area)) => Some((table_num, area)),
                _ => None,
            };
            (
                order.id,
                order.label.clone(),
                order.service,
                table_info,
                order.totals(),
                render_receipt(order, customer_mobile, &gst_number),
            )
        };

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
                self.notify(format!("Bill #{} saved to database.", order_id));

                self.order_mut().status = OrderStatus::Paid;

                if let Some((table_num, area)) = table_info {
                    if let Some(pt) = self
                        .physical_tables
                        .iter_mut()
                        .find(|t| t.area == area && t.number == table_num)
                    {
                        pt.status = TableStatus::Paid;
                    }
                    self.persist_table(&area, table_num);
                }
                self.focus = self.focus_return;
    }

    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        let in_protected = matches!(
            self.focus,
            Focus::Search | Focus::MobileEntry | Focus::PaymentMode | Focus::OfferSelect
        );
        if matches!(key, KeyCode::Char('q')) && !in_protected {
            return true;
        }
        if matches!(key, KeyCode::Esc) && !in_protected {
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
                KeyCode::Char('e') => self.export_config(),
                KeyCode::Char('i') => self.import_config(),
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
                KeyCode::Left | KeyCode::Char('h') => {
                    self.selected_table_index = self.selected_table_index.saturating_sub(1);
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    let max = self.selected_area().map_or(0, |a| a.table_count);
                    if self.selected_table_index + 1 < max {
                        self.selected_table_index += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !self.areas.is_empty() {
                        let n = self.areas.len();
                        self.selected_area_index = (self.selected_area_index + n - 1) % n;
                        let max = self.selected_area().map_or(0, |a| a.table_count);
                        if self.selected_table_index >= max {
                            self.selected_table_index = max.saturating_sub(1);
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.areas.is_empty() {
                        let n = self.areas.len();
                        self.selected_area_index = (self.selected_area_index + 1) % n;
                        let max = self.selected_area().map_or(0, |a| a.table_count);
                        if self.selected_table_index >= max {
                            self.selected_table_index = max.saturating_sub(1);
                        }
                    }
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
                KeyCode::Char('b') | KeyCode::Char('p') => self.begin_billing(),
                KeyCode::BackTab => self.focus = Focus::Cart,
                KeyCode::Tab => self.focus = Focus::RecentBills,
                KeyCode::Char(d @ '1'..='9') => {
                    let idx = (d as u8 - b'1') as usize;
                    if idx < self.areas.len() {
                        self.selected_area_index = idx;
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
                        if self.mobile_buffer.len() == 10 || self.mobile_buffer.is_empty() {
                            let mobile = std::mem::take(&mut self.mobile_buffer);
                            if self.offers.is_empty() {
                                self.complete_billing(&mobile, None);
                            } else {
                                self.offer_index = 0;
                                self.focus = Focus::OfferSelect;
                                self.pending_mobile = mobile;
                            }
                        } else {
                            self.notify(format!(
                                "Mobile number needs 10 digits ({} so far, or press Enter on empty to skip).",
                                self.mobile_buffer.len()
                            ));
                        }
                    }
                    KeyCode::Esc => {
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
                    self.focus = self.focus_return;
                }
                _ => {}
            },
            Focus::OfferSelect => {
                let count = self.offers.len() + 1;
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.offer_index = self.offer_index.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.offer_index + 1 < count {
                            self.offer_index += 1;
                        }
                    }
                    KeyCode::Char(d @ '0'..='9') => {
                        let idx = (d as u8 - b'0') as usize;
                        if idx < count {
                            self.apply_offer_and_bill(idx);
                        }
                    }
                    KeyCode::Enter => self.apply_offer_and_bill(self.offer_index),
                    KeyCode::Esc => {
                        self.pending_mobile.clear();
                        self.focus = self.focus_return;
                    }
                    _ => {}
                }
            }
        }
        false
    }

    pub fn apply_offer_and_bill(&mut self, index: usize) {
        let offer_id = if index == 0 {
            None
        } else {
            self.offers.get(index - 1).map(|o| o.id)
        };
        let mobile = std::mem::take(&mut self.pending_mobile);
        self.complete_billing(&mobile, offer_id);
    }

    pub fn rebuild_physical_tables(&mut self) {
        let previous: Vec<PhysicalTable> = self.physical_tables.clone();
        let mut tables = init_physical_tables(&self.areas);
        for table in &mut tables {
            if let Some(old) = previous
                .iter()
                .find(|p| p.area == table.area && p.number == table.number)
            {
                table.status = old.status;
                table.order_id = old.order_id;
                table.dirty_since = old.dirty_since;
            }
        }
        self.physical_tables = tables;
    }

    pub fn export_config(&mut self) {
        let base = self
            .data_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let menu_p = base.join("menu.csv");
        let areas_p = base.join("areas.csv");
        let offers_p = base.join("offers.csv");
        let config_p = base.join("config.csv");

        let menu_n = export_menu_csv(&menu_p, &self.items).map_err(|e| e.to_string());
        let areas_n = export_areas_csv(&areas_p, &self.areas).map_err(|e| e.to_string());
        let offers_n = export_offers_csv(&offers_p, &self.offers).map_err(|e| e.to_string());
        let config_ok =
            export_config_csv(&config_p, &self.gst_number, self.ac_rate).map_err(|e| e.to_string());

        match (menu_n, areas_n, offers_n, config_ok) {
            (Ok(mn), Ok(an), Ok(on), Ok(())) => self.notify(format!(
                "Exported config: {mn} items, {an} areas, {on} offers, settings."
            )),
            _ => self.notify("Config export failed.".to_string()),
        }
    }

    pub fn import_config(&mut self) {
        let base = self
            .data_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let menu_p = base.join("menu.csv");
        let areas_p = base.join("areas.csv");
        let offers_p = base.join("offers.csv");
        let config_p = base.join("config.csv");

        let items = match load_menu(&menu_p) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => {
                self.notify("Import skipped: menu.csv empty.".to_string());
                return;
            }
            Err(e) => {
                self.notify(format!("Menu import failed: {e}"));
                return;
            }
        };

        let areas = match load_areas_csv(&areas_p) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => {
                self.notify("Import skipped: areas.csv empty.".to_string());
                return;
            }
            Err(e) => {
                self.notify(format!("Areas import failed: {e}"));
                return;
            }
        };

        let offers = match load_offers_csv(&offers_p) {
            Ok(v) => v,
            Err(e) => {
                self.notify(format!("Offers import failed: {e}"));
                return;
            }
        };

        let (gst, ac_rate) = match load_config_csv(&config_p) {
            Ok(map) => (
                map.get("GSTNumber")
                    .cloned()
                    .unwrap_or_else(|| self.gst_number.clone()),
                map.get("AcRate")
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .map(|p| (p / 100.0).clamp(0.0, 1.0))
                    .unwrap_or(self.ac_rate),
            ),
            Err(e) => {
                self.notify(format!("Config import failed: {e}"));
                return;
            }
        };

        if let Some(db) = &self.database {
            if let Err(e) = db.replace_menu(&items) {
                self.notify(format!("DB menu sync failed: {e}"));
                return;
            }
            if let Err(e) = db.replace_areas(&areas) {
                self.notify(format!("DB areas sync failed: {e}"));
                return;
            }
            if let Err(e) = db.replace_offers(&offers) {
                self.notify(format!("DB offers sync failed: {e}"));
                return;
            }
            if let Err(e) = db.set_setting("gst_number", &gst) {
                self.notify(format!("DB gst sync failed: {e}"));
                return;
            }
            if let Err(e) = db.set_setting("ac_rate", &format!("{:.4}", ac_rate)) {
                self.notify(format!("DB ac sync failed: {e}"));
                return;
            }
        }

        self.items = items;
        self.areas = areas;
        self.offers = if let Some(db) = &self.database {
            db.load_offers()
        } else {
            offers
        };
        self.gst_number = gst;
        self.ac_rate = ac_rate;
        self.rebuild_physical_tables();
        self.notify("Imported configuration from CSV files.".to_string());
    }
}

pub fn init_physical_tables(areas: &[Area]) -> Vec<PhysicalTable> {
    let mut tables = Vec::new();
    for area in areas {
        for number in 1..=area.table_count {
            tables.push(PhysicalTable::ready(&area.name, number));
        }
    }
    tables
}

pub fn max_takeout_number(orders: &[Order]) -> u32 {
    orders
        .iter()
        .filter(|o| o.service == Service::TakeOut)
        .filter_map(|o| o.label.strip_prefix("TK")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}
