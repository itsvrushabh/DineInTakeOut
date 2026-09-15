//! Application state management, user actions, and event routing.

pub mod billing;
pub mod cart;
pub mod events;
pub mod kot;
pub mod orders;
pub mod reports;
pub mod tables;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use chrono::Local;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::{
    config::{
        default_menu, export_areas_csv, export_config_csv, export_menu_csv, export_offers_csv,
        export_table_csv, load_areas_csv, load_config_csv, load_menu, load_offers_csv,
        load_table_csv,
    },
    db::Database,
    models::{
        Area, BillSummary, CustomerCrmProfile, DailySalesSummary, Focus, HistoricalBill,
        KotSummary, MenuItem, Offer, Order, PhysicalTable, RecentTab, SalesAnalytics, Service,
        TableStatus, CLEANING_MINUTES,
    },
    printer::PrinterConfig,
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
    pub restaurant_name: String,             // hotel / restaurant name printed on bills
    pub restaurant_address: String,          // restaurant address printed on bills
    pub restaurant_contact: String,          // contact phone/mobile printed on bills
    pub gst_number: String,                  // registered GSTIN printed on receipts
    pub ac_rate: f64,                        // AC surcharge rate (fraction) for AC areas
    pub offers: Vec<Offer>,                  // discount offers selectable at billing
    pub upi_id: String,                      // UPI VPA address for QR payments
    pub menu_index: usize,
    pub search: String,
    pub focus: Focus,
    pub data_file: PathBuf,
    pub matcher: SkimMatcherV2,
    pub selected_area_index: usize, // current area when navigating tables
    pub selected_table_index: usize, // current table index within area
    pub notifications: Vec<(String, chrono::DateTime<Local>)>,
    pub recent_bills: Vec<BillSummary>, // newest first, up to five completed bills
    pub recent_bill_index: usize,
    pub recent_kots: Vec<KotSummary>, // newest first, up to ten KOTs
    pub recent_kot_index: usize,
    pub recent_tab: RecentTab, // active box in Recent panel (Bills vs KOTs)
    pub focus_return: Focus,   // panel to restore when the prompt closes
    pub database: Option<Database>, // None only when the DB could not be opened
    pub mobile_buffer: String, // customer mobile captured in the billing prompt
    pub payment_mode_index: usize, // selection inside the payment-mode popup
    pub close_on_payment: bool, // true if payment modal closes order, false if updating bill
    pub offer_index: usize,    // selection inside the offer-selection popup
    pub pending_mobile: String, // customer mobile captured before offer pick
    pub show_help: bool,
    pub table_input: String,
    pub table_search_index: usize,
    pub bill_search_query: String,
    pub bill_search_results: Vec<HistoricalBill>,
    pub bill_search_index: usize,
    pub item_note_buffer: String,
    pub table_move_target_index: usize,
    pub daily_report_summary: Option<DailySalesSummary>,
    pub categories: Vec<String>,
    pub selected_category_index: usize,
    pub customer_crm: Option<CustomerCrmProfile>,
    pub split_cash: String,
    pub split_upi: String,
    pub split_card: String,
    pub split_field: usize,
    pub kds_kots: Vec<KotSummary>,
    pub kds_index: usize,
    pub sales_analytics: Option<SalesAnalytics>,
    pub printer_config: PrinterConfig,
    pub pending_effects: std::collections::VecDeque<AppEffect>,
}

/// Commands and I/O effects emitted by App business logic to be executed
/// asynchronously or in background tasks without stalling the UI rendering thread.
#[derive(Clone, Debug, PartialEq)]
pub enum AppEffect {
    PrintReceipt { printer: String, data: Vec<u8> },
    PrintKot { printer: String, data: Vec<u8> },
    SaveFile { path: PathBuf, content: String },
    TriggerBackup,
}

/// Discrete modal dialog state machine encapsulating ephemeral buffers and popup invariants.
#[derive(Clone, Debug, PartialEq)]
pub enum ModalState {
    None,
    MobileEntry {
        buffer: String,
    },
    PaymentMode {
        index: usize,
        close_on_payment: bool,
    },
    OfferSelect {
        index: usize,
    },
    TableJump {
        query: String,
        index: usize,
    },
    TableMove {
        target_index: usize,
    },
    ItemNote {
        buffer: String,
    },
    BillSearch {
        query: String,
        index: usize,
    },
    DailyReport,
    SplitPayment {
        cash: String,
        upi: String,
        card: String,
        field: usize,
    },
    KitchenDisplay {
        index: usize,
    },
    SalesAnalytics,
    Help,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableDestination {
    pub area_name: String,
    pub table_number: usize,
    pub is_ac: bool,
    pub status: TableStatus,
    pub order_id: Option<u32>,
    pub order_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableSearchResult {
    pub area_index: usize,
    pub area_name: String,
    pub table_number: usize,
    pub is_ac: bool,
    pub status: TableStatus,
    pub dirty_since: Option<chrono::DateTime<Local>>,
    pub order_id: Option<u32>,
    pub order_label: Option<String>,
    pub order_total: Option<f64>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self::new_with_paths(Some(Path::new(DB_PATH)), Some(Path::new(".")))
    }

    pub fn new_with_paths(db_path: Option<&Path>, base_dir: Option<&Path>) -> Self {
        let base = base_dir.unwrap_or_else(|| Path::new("."));
        let data_file = base.join("menu.csv");
        let config_path = base.join("config.csv");
        let table_path = base.join("table.csv");
        let areas_path = base.join("areas.csv");

        // Open the embedded Turso database. A failure is not fatal: the app
        // keeps running from CSV/defaults.
        let database = match db_path {
            Some(p) => match Database::open(p) {
                Ok(db) => Some(db),
                Err(error) => {
                    eprintln!("Database unavailable ({error}); running without persistence.");
                    None
                }
            },
            None => None,
        };

        // Menu precedence: database catalogue → menu.csv → built-in defaults.
        // Whatever we fall back to is seeded into the DB so it becomes the
        // single source of truth for the next launch.
        let mut _db_notice = String::new();
        let items = match &database {
            Some(db) if !db.menu_is_empty() => db.load_menu(),
            _ => match load_menu(&data_file) {
                Ok(items) if !items.is_empty() => {
                    if let Some(db) = &database {
                        if let Err(error) = db.replace_menu(&items) {
                            _db_notice = format!("Menu seed failed: {error}");
                        }
                    }
                    items
                }
                _ => {
                    let items = default_menu();
                    if let Some(db) = &database {
                        if let Err(error) = db.replace_menu(&items) {
                            _db_notice = format!("Menu seed failed: {error}");
                        }
                    }
                    items
                }
            },
        };

        // Load config from config.csv (if present)
        let config_csv = load_config_csv(&config_path).unwrap_or_default();
        let get_cfg = |keys: &[&str]| -> Option<String> {
            for k in keys {
                if let Some(v) = config_csv.get(*k) {
                    if !v.trim().is_empty() {
                        return Some(v.trim().to_string());
                    }
                }
                for (mk, mv) in &config_csv {
                    if mk.eq_ignore_ascii_case(k) && !mv.trim().is_empty() {
                        return Some(mv.trim().to_string());
                    }
                }
            }
            None
        };

        // Dining areas: database → table.csv → areas.csv → defaults
        let areas = match &database {
            Some(db) => {
                let db_areas = db.load_areas();
                if db_areas.is_empty() {
                    let loaded = load_table_csv(&table_path)
                        .or_else(|_| load_areas_csv(&areas_path))
                        .unwrap_or_else(|_| Area::defaults());
                    let _ = db.replace_areas(&loaded);
                    loaded
                } else {
                    db_areas
                }
            }
            None => load_table_csv(&table_path)
                .or_else(|_| load_areas_csv(&areas_path))
                .unwrap_or_else(|_| Area::defaults()),
        };

        // Hotel & billing configuration
        let (
            restaurant_name,
            restaurant_address,
            restaurant_contact,
            gst_number,
            ac_rate,
            offers,
            upi_id,
        ) = match &database {
            Some(db) => {
                let db_name = db.get_setting("restaurant_name");
                let name = if !db_name.trim().is_empty() {
                    db_name
                } else {
                    get_cfg(&[
                        "RestaurantName",
                        "restaurant_name",
                        "HotelName",
                        "hotel_name",
                    ])
                    .unwrap_or_else(|| "SHREE KRISHNA RESTAURANT".to_string())
                };

                let db_addr = db.get_setting("restaurant_address");
                let address = if !db_addr.trim().is_empty() {
                    db_addr
                } else {
                    get_cfg(&["Address", "address", "HotelAddress", "restaurant_address"])
                        .unwrap_or_else(|| "Station Road, Near Main Market".to_string())
                };

                let db_contact = db.get_setting("restaurant_contact");
                let contact = if !db_contact.trim().is_empty() {
                    db_contact
                } else {
                    get_cfg(&["Contact", "contact", "Phone", "phone", "restaurant_contact"])
                        .unwrap_or_else(|| "+91 98765 43210".to_string())
                };

                let db_gst = db.get_setting("gst_number");
                let gst = if !db_gst.trim().is_empty() {
                    db_gst
                } else {
                    get_cfg(&["GSTNumber", "gst_number", "GST", "GSTIN"])
                        .unwrap_or_else(|| "27AAPFU0939F1ZV".to_string())
                };

                let db_ac = db.get_setting("ac_rate");
                let ac = if let Ok(rate) = db_ac.parse::<f64>() {
                    if rate > 1.0 {
                        rate / 100.0
                    } else {
                        rate
                    }
                } else if let Some(rate_str) = get_cfg(&["AcRate", "ac_rate", "ACRate"]) {
                    let r = rate_str.parse::<f64>().unwrap_or(6.0);
                    if r > 1.0 {
                        r / 100.0
                    } else {
                        r
                    }
                } else {
                    0.06
                };

                let db_upi = db.get_setting("upi_id");
                let upi = if !db_upi.trim().is_empty() {
                    db_upi
                } else {
                    get_cfg(&["UpiId", "upi_id", "UPI", "UPIID"])
                        .unwrap_or_else(|| "shreekrishna@upi".to_string())
                };

                let _ = db.set_setting("restaurant_name", &name);
                let _ = db.set_setting("restaurant_address", &address);
                let _ = db.set_setting("restaurant_contact", &contact);
                let _ = db.set_setting("gst_number", &gst);
                let _ = db.set_setting("ac_rate", &format!("{:.4}", ac));
                let _ = db.set_setting("upi_id", &upi);

                (name, address, contact, gst, ac, db.load_offers(), upi)
            }
            None => {
                let name = get_cfg(&[
                    "RestaurantName",
                    "restaurant_name",
                    "HotelName",
                    "hotel_name",
                ])
                .unwrap_or_else(|| "SHREE KRISHNA RESTAURANT".to_string());
                let address =
                    get_cfg(&["Address", "address", "HotelAddress", "restaurant_address"])
                        .unwrap_or_else(|| "Station Road, Near Main Market".to_string());
                let contact =
                    get_cfg(&["Contact", "contact", "Phone", "phone", "restaurant_contact"])
                        .unwrap_or_else(|| "+91 98765 43210".to_string());
                let gst = get_cfg(&["GSTNumber", "gst_number", "GST", "GSTIN"])
                    .unwrap_or_else(|| "27AAPFU0939F1ZV".to_string());
                let ac = if let Some(rate_str) = get_cfg(&["AcRate", "ac_rate", "ACRate"]) {
                    let r = rate_str.parse::<f64>().unwrap_or(6.0);
                    if r > 1.0 {
                        r / 100.0
                    } else {
                        r
                    }
                } else {
                    0.06
                };
                let upi = get_cfg(&["UpiId", "upi_id", "UPI", "UPIID"])
                    .unwrap_or_else(|| "shreekrishna@upi".to_string());

                (name, address, contact, gst, ac, Vec::new(), upi)
            }
        };

        // Restore unpaid orders and physical table states from the last run.
        let now = Local::now();

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
            None => (Vec::new(), 1, init_physical_tables(&areas)),
        };

        let (recent_bills, recent_kots) = if let Some(db) = &database {
            (db.load_recent_bills(), db.load_recent_kots())
        } else {
            (Vec::new(), Vec::new())
        };
        let next_takeout_id = max_takeout_number(&orders).saturating_add(1);

        let mut categories = vec!["ALL".to_string()];
        for it in &items {
            if !it.category.trim().is_empty() && !categories.contains(&it.category) {
                categories.push(it.category.clone());
            }
        }
        let printer_config = PrinterConfig::from_map(&config_csv);

        Self {
            items,
            orders: orders.to_vec(),
            active_order: 0,
            next_order_id,
            next_takeout_id,
            physical_tables,
            areas,
            restaurant_name,
            restaurant_address,
            restaurant_contact,
            gst_number,
            ac_rate,
            offers,
            upi_id,
            menu_index: 0,
            search: String::new(),
            focus: Focus::Menu,
            data_file,
            matcher: SkimMatcherV2::default().ignore_case(),
            selected_area_index: 0,
            selected_table_index: 0,
            notifications: Vec::new(),
            recent_bills,
            recent_bill_index: 0,
            recent_kots,
            recent_kot_index: 0,
            recent_tab: RecentTab::Bills,
            focus_return: Focus::Cart,
            database,
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
            categories,
            selected_category_index: 0,
            customer_crm: None,
            split_cash: String::new(),
            split_upi: String::new(),
            split_card: String::new(),
            split_field: 0,
            kds_kots: Vec::new(),
            kds_index: 0,
            sales_analytics: None,
            printer_config,
            pending_effects: std::collections::VecDeque::new(),
        }
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

    pub fn selected_physical_table(&self) -> Option<&PhysicalTable> {
        let area = self.selected_area_name();
        let number = self.selected_table_index + 1;
        self.physical_tables
            .iter()
            .find(|t| t.area == area && t.number == number)
    }

    pub fn selected_table_order(&self) -> Option<&Order> {
        let area = self.selected_area_name();
        let number = self.selected_table_index + 1;
        self.orders.iter().find(|o| {
            o.service == Service::DineIn
                && o.area.as_deref() == Some(&area)
                && o.table_number == Some(number)
        })
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

    pub fn order(&self) -> &Order {
        &self.orders[self.active_order]
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

    pub fn clean_selected_table(&mut self) {
        let area = self.selected_area_name();
        let table_num = self.selected_table_index + 1;
        self.transition_table_status(&area, table_num, TableStatus::Ready);
        self.notify(format!("Table {table_num} cleaned and ready."));
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notifications
            .push((msg.into(), Local::now() + chrono::Duration::seconds(10)));
    }

    pub fn tick_notification(&mut self) {
        self.notifications
            .retain(|(_, until)| Local::now() < *until);
    }

    /// Push an I/O effect or asynchronous command to be dispatched.
    pub fn queue_effect(&mut self, effect: AppEffect) {
        self.pending_effects.push_back(effect);
    }

    /// Drain all pending I/O effects and return them for processing.
    pub fn drain_effects(&mut self) -> Vec<AppEffect> {
        self.pending_effects.drain(..).collect()
    }

    /// Returns the currently active modal state according to focus and flags.
    pub fn active_modal(&self) -> ModalState {
        if self.show_help {
            return ModalState::Help;
        }
        match self.focus {
            Focus::MobileEntry => ModalState::MobileEntry {
                buffer: self.mobile_buffer.clone(),
            },
            Focus::PaymentMode => ModalState::PaymentMode {
                index: self.payment_mode_index,
                close_on_payment: self.close_on_payment,
            },
            Focus::OfferSelect => ModalState::OfferSelect {
                index: self.offer_index,
            },
            Focus::TableJump => ModalState::TableJump {
                query: self.table_input.clone(),
                index: self.table_search_index,
            },
            Focus::TableMove => ModalState::TableMove {
                target_index: self.table_move_target_index,
            },
            Focus::ItemNote => ModalState::ItemNote {
                buffer: self.item_note_buffer.clone(),
            },
            Focus::BillSearch => ModalState::BillSearch {
                query: self.bill_search_query.clone(),
                index: self.bill_search_index,
            },
            Focus::DailyReport => ModalState::DailyReport,
            Focus::SplitPayment => ModalState::SplitPayment {
                cash: self.split_cash.clone(),
                upi: self.split_upi.clone(),
                card: self.split_card.clone(),
                field: self.split_field,
            },
            Focus::KitchenDisplay => ModalState::KitchenDisplay {
                index: self.kds_index,
            },
            Focus::Analytics => ModalState::SalesAnalytics,
            _ => ModalState::None,
        }
    }

    /// Resets all ephemeral modal buffers and returns focus to the main panel.
    pub fn close_active_modal(&mut self) {
        self.show_help = false;
        self.mobile_buffer.clear();
        self.item_note_buffer.clear();
        self.table_input.clear();
        self.split_cash.clear();
        self.split_upi.clear();
        self.split_card.clear();
        self.split_field = 0;
        self.focus = self.focus_return;
    }

    /// Switch focus directly to a specific box by its 1-indexed number.
    /// [1] Tables & Floor Plan
    /// [2] Table Details & Jump
    /// [3] Menu Search
    /// [4] Menu Catalogue
    /// [5] Active Bill & Cart
    /// [6] Recent Bills
    /// [7] KOT Bills
    pub fn switch_to_box(&mut self, box_num: u8) {
        match box_num {
            1 => {
                self.focus = Focus::Tables;
            }
            2 => {
                self.focus = Focus::TableJump;
                self.table_input.clear();
                self.table_search_index = 0;
            }
            3 => {
                self.focus = Focus::Search;
            }
            4 => {
                self.focus = Focus::Menu;
            }
            5 => {
                self.focus = Focus::Cart;
            }
            6 => {
                self.focus = Focus::RecentBills;
                self.recent_tab = RecentTab::Bills;
            }
            7 => {
                self.focus = Focus::RecentBills;
                self.recent_tab = RecentTab::Kots;
            }
            _ => {}
        }
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
        let table_p = base.join("table.csv");
        let areas_p = base.join("areas.csv");
        let offers_p = base.join("offers.csv");
        let config_p = base.join("config.csv");

        let menu_n = export_menu_csv(&menu_p, &self.items).map_err(|e| e.to_string());
        let table_n = export_table_csv(&table_p, &self.areas).map_err(|e| e.to_string());
        let _ = export_areas_csv(&areas_p, &self.areas);
        let offers_n = export_offers_csv(&offers_p, &self.offers).map_err(|e| e.to_string());
        let config_ok = export_config_csv(
            &config_p,
            &self.restaurant_name,
            &self.restaurant_address,
            &self.restaurant_contact,
            &self.gst_number,
            self.ac_rate,
            &self.upi_id,
        )
        .map_err(|e| e.to_string());

        match (menu_n, table_n, offers_n, config_ok) {
            (Ok(mn), Ok(tn), Ok(on), Ok(())) => self.notify(format!(
                "Exported config: {mn} items, {tn} table types, {on} offers, settings."
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
        let table_p = base.join("table.csv");
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

        let areas = match load_table_csv(&table_p).or_else(|_| load_areas_csv(&areas_p)) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => {
                self.notify("Import skipped: table.csv / areas.csv empty.".to_string());
                return;
            }
            Err(e) => {
                self.notify(format!("Tables import failed: {e}"));
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

        let (restaurant_name, address, contact, gst, ac_rate, upi_id) =
            match load_config_csv(&config_p) {
                Ok(map) => (
                    map.get("RestaurantName")
                        .or_else(|| map.get("restaurant_name"))
                        .or_else(|| map.get("HotelName"))
                        .cloned()
                        .unwrap_or_else(|| self.restaurant_name.clone()),
                    map.get("Address")
                        .or_else(|| map.get("address"))
                        .or_else(|| map.get("restaurant_address"))
                        .cloned()
                        .unwrap_or_else(|| self.restaurant_address.clone()),
                    map.get("Contact")
                        .or_else(|| map.get("contact"))
                        .or_else(|| map.get("Phone"))
                        .or_else(|| map.get("restaurant_contact"))
                        .cloned()
                        .unwrap_or_else(|| self.restaurant_contact.clone()),
                    map.get("GSTNumber")
                        .or_else(|| map.get("gst_number"))
                        .or_else(|| map.get("GST"))
                        .cloned()
                        .unwrap_or_else(|| self.gst_number.clone()),
                    map.get("AcRate")
                        .or_else(|| map.get("ac_rate"))
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .map(|p| {
                            if p > 1.0 {
                                (p / 100.0).clamp(0.0, 1.0)
                            } else {
                                p.clamp(0.0, 1.0)
                            }
                        })
                        .unwrap_or(self.ac_rate),
                    map.get("UpiId")
                        .or_else(|| map.get("upi_id"))
                        .cloned()
                        .unwrap_or_else(|| self.upi_id.clone()),
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
            let _ = db.set_setting("restaurant_name", &restaurant_name);
            let _ = db.set_setting("restaurant_address", &address);
            let _ = db.set_setting("restaurant_contact", &contact);
            if let Err(e) = db.set_setting("gst_number", &gst) {
                self.notify(format!("DB gst sync failed: {e}"));
                return;
            }
            if let Err(e) = db.set_setting("ac_rate", &format!("{:.4}", ac_rate)) {
                self.notify(format!("DB ac sync failed: {e}"));
                return;
            }
            if let Err(e) = db.set_setting("upi_id", &upi_id) {
                self.notify(format!("DB upi sync failed: {e}"));
                return;
            }
        }

        self.items = items;
        self.areas = areas;
        self.restaurant_name = restaurant_name;
        self.restaurant_address = address;
        self.restaurant_contact = contact;
        self.offers = if let Some(db) = &self.database {
            db.load_offers()
        } else {
            offers
        };
        self.gst_number = gst;
        self.ac_rate = ac_rate;
        self.upi_id = upi_id;
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
