//! Database persistence via the Turso engine: menu catalogue, physical table
//! states, unpaid in-progress orders, and paid order history.
//!
//! The database is an embedded Turso file (SQLite-compatible) at
//! `data/billing.db`, created automatically on first run. The same schema and
//! code work against a synced Turso Cloud replica by enabling the crate's
//! `sync` feature later without any table changes.

use std::path::Path;

use chrono::{DateTime, Local, NaiveDateTime};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio::sync::Mutex;
use turso::{Connection, Row, Value};

use crate::models::{
    Area, BillSummary, BillTotals, CartLine, DailySalesSummary, HistoricalBill, KotSummary,
    MenuItem, Offer, Order, OrderStatus, PaymentMode, PhysicalTable, Service, TableStatus,
    CLEANING_MINUTES,
};

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

const SCHEMA: [&str; 11] = [
    "CREATE TABLE IF NOT EXISTS menu_items (
         name         TEXT PRIMARY KEY,
         category     TEXT NOT NULL,
         unit         TEXT NOT NULL DEFAULT '',
         price        REAL NOT NULL CHECK (price >= 0),
         is_available INTEGER NOT NULL DEFAULT 1
     )",
    "CREATE TABLE IF NOT EXISTS orders (
         id              INTEGER PRIMARY KEY,
         label           TEXT NOT NULL,
         service         TEXT NOT NULL,
         table_number    INTEGER,
         area            TEXT,
         customer_mobile TEXT NOT NULL,
         subtotal        REAL NOT NULL,
         discount        REAL NOT NULL DEFAULT 0,
         ac_charge       REAL NOT NULL DEFAULT 0,
         tax             REAL NOT NULL,
         total           REAL NOT NULL,
         payment_mode    TEXT NOT NULL DEFAULT '',
         status          TEXT NOT NULL DEFAULT 'PAID',
         created_at      TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
     )",
    "CREATE TABLE IF NOT EXISTS order_items (
         id         INTEGER PRIMARY KEY AUTOINCREMENT,
         order_id   INTEGER NOT NULL REFERENCES orders(id),
         name       TEXT NOT NULL,
         unit_price REAL NOT NULL,
         qty        INTEGER NOT NULL CHECK (qty > 0),
         line_total REAL NOT NULL,
         notes      TEXT NOT NULL DEFAULT ''
     )",
    "CREATE TABLE IF NOT EXISTS open_orders (
         id           INTEGER PRIMARY KEY,
         label        TEXT NOT NULL,
         service      TEXT NOT NULL,
         table_number INTEGER,
         area         TEXT,
         is_ac        INTEGER NOT NULL DEFAULT 0,
         ac_rate      REAL NOT NULL DEFAULT 0,
         status       TEXT NOT NULL
     )",
    "CREATE TABLE IF NOT EXISTS open_order_items (
         order_id   INTEGER NOT NULL REFERENCES open_orders(id) ON DELETE CASCADE,
         position   INTEGER NOT NULL,
         name       TEXT NOT NULL,
         unit_price REAL NOT NULL,
         qty        INTEGER NOT NULL CHECK (qty > 0),
         notes      TEXT NOT NULL DEFAULT '',
         PRIMARY KEY (order_id, position)
     )",
    "CREATE TABLE IF NOT EXISTS tables (
         area       TEXT NOT NULL,
         number     INTEGER NOT NULL,
         status     TEXT NOT NULL,
         order_id   INTEGER,
         updated_at TEXT NOT NULL,
         PRIMARY KEY (area, number)
     )",
    "CREATE TABLE IF NOT EXISTS areas (
         name        TEXT PRIMARY KEY,
         is_ac       INTEGER NOT NULL DEFAULT 0,
         table_count INTEGER NOT NULL CHECK (table_count >= 0),
         sort_order  INTEGER NOT NULL DEFAULT 0
     )",
    "CREATE TABLE IF NOT EXISTS settings (
         key   TEXT PRIMARY KEY,
         value TEXT NOT NULL
     )",
    "CREATE TABLE IF NOT EXISTS offers (
         id                INTEGER PRIMARY KEY AUTOINCREMENT,
         name              TEXT NOT NULL,
         discount_percent  REAL NOT NULL CHECK (discount_percent >= 0 AND discount_percent <= 100)
     )",
    "CREATE TABLE IF NOT EXISTS kots (
         id          INTEGER PRIMARY KEY AUTOINCREMENT,
         order_id    INTEGER NOT NULL,
         label       TEXT NOT NULL,
         area        TEXT NOT NULL DEFAULT '',
         item_count  INTEGER NOT NULL DEFAULT 0,
         ticket_text TEXT NOT NULL,
         is_reprint  INTEGER NOT NULL DEFAULT 0,
         created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
     )",
    "CREATE TABLE IF NOT EXISTS z_reports (
         id          INTEGER PRIMARY KEY AUTOINCREMENT,
         report_date TEXT NOT NULL,
         gross_sales REAL NOT NULL DEFAULT 0,
         net_sales   REAL NOT NULL DEFAULT 0,
         bill_count  INTEGER NOT NULL DEFAULT 0,
         report_text TEXT NOT NULL,
         created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
     )",
];

pub struct Database {
    rt: Runtime,
    /// Serialised access: every operation locks the single connection, which
    /// also satisfies `transaction()`'s `&mut Connection` requirement.
    conn: Mutex<Connection>,
}

impl Database {
    /// Opens the embedded database file, creating parent directories and the
    /// schema when needed.
    pub fn open(path: &Path) -> Result<Self, String> {
        Self::open_local(path)
    }

    pub fn open_local(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        Self::create_daily_backup(path);
        let rt = Self::runtime()?;
        let db = rt
            .block_on(turso::Builder::new_local(&path.display().to_string()).build())
            .map_err(|e| e.to_string())?;
        let conn = db.connect().map_err(|e| e.to_string())?;
        let database = Self {
            rt,
            conn: Mutex::new(conn),
        };
        database.init_schema()?;
        Ok(database)
    }

    pub fn create_daily_backup(path: &Path) {
        if !path.exists() {
            return;
        }
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let backup_dir = path.parent().unwrap_or(Path::new("data")).join("backups");
        let _ = std::fs::create_dir_all(&backup_dir);
        let backup_file = backup_dir.join(format!("billing_{date}.db"));
        if !backup_file.exists() {
            let _ = std::fs::copy(path, backup_file);
        }
    }

    fn runtime() -> Result<Runtime, String> {
        RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())
    }

    fn init_schema(&self) -> Result<(), String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            // Best effort: foreign keys are an enforcement optimisation here.
            let _ = conn.execute("PRAGMA foreign_keys = ON", ()).await;
            for statement in SCHEMA {
                conn.execute(statement, ())
                    .await
                    .map_err(|e| format!("{e} (while running schema)"))?;
            }
            // Migration for databases created before discount existed.
            let _ = conn
                .execute(
                    "ALTER TABLE orders ADD COLUMN discount REAL NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            // Migration for databases created before AC surcharges existed.
            let _ = conn
                .execute(
                    "ALTER TABLE orders ADD COLUMN ac_charge REAL NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            // Migration for databases created before payment modes existed.
            let _ = conn
                .execute(
                    "ALTER TABLE orders ADD COLUMN payment_mode TEXT NOT NULL DEFAULT ''",
                    (),
                )
                .await;
            // Migration for databases created before AC flags were stored on open orders.
            let _ = conn
                .execute(
                    "ALTER TABLE open_orders ADD COLUMN is_ac INTEGER NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE open_orders ADD COLUMN ac_rate REAL NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            // Migration for item availability
            let _ = conn
                .execute(
                    "ALTER TABLE menu_items ADD COLUMN is_available INTEGER NOT NULL DEFAULT 1",
                    (),
                )
                .await;
            // Migration for item notes
            let _ = conn
                .execute(
                    "ALTER TABLE order_items ADD COLUMN notes TEXT NOT NULL DEFAULT ''",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE open_order_items ADD COLUMN notes TEXT NOT NULL DEFAULT ''",
                    (),
                )
                .await;

            // Seed the dynamic configuration the first time the schema exists.
            Self::seed_defaults(&conn).await;
            Ok(())
        })
    }

    // -- menu -----------------------------------------------------------------

    pub fn load_menu(&self) -> Vec<MenuItem> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = match conn
                .query(
                    "SELECT category, name, unit, price, is_available FROM menu_items ORDER BY rowid",
                    (),
                )
                .await
            {
                Ok(rows) => rows,
                Err(_) => return Vec::new(),
            };
            let mut items = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                items.push(MenuItem {
                    category: row_string(&row, 0),
                    name: row_string(&row, 1),
                    unit: row_string(&row, 2),
                    price: row_f64(&row, 3),
                    is_available: row_i64(&row, 4) != 0,
                });
            }
            items
        })
    }

    pub fn menu_is_empty(&self) -> bool {
        self.scalar_i64("SELECT COUNT(*) FROM menu_items") == 0
    }

    pub fn update_menu_item_availability(
        &self,
        name: &str,
        is_available: bool,
    ) -> Result<(), String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute(
                "UPDATE menu_items SET is_available = ?2 WHERE name = ?1",
                (name, is_available as i64),
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        })
    }

    /// Replaces the whole menu catalogue in one transaction.
    pub fn replace_menu(&self, items: &[MenuItem]) -> Result<(), String> {
        self.rt.block_on(async {
            let mut conn = self.conn.lock().await;
            let tx = conn.transaction().await.map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM menu_items", ())
                .await
                .map_err(|e| e.to_string())?;
            for item in items {
                tx.execute(
                    "INSERT INTO menu_items (name, category, unit, price, is_available) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (
                        item.name.clone(),
                        item.category.clone(),
                        item.unit.clone(),
                        item.price,
                        item.is_available as i64,
                    ),
                )
                .await
                .map_err(|e| format!("{e} (item: {})", item.name))?;
            }
            tx.commit().await.map_err(|e| e.to_string())
        })
    }

    // -- areas / settings / offers -------------------------------------------

    /// Populates the dynamic configuration the first time the database is
    /// created: the seed dining areas and the default GST/AC settings.
    async fn seed_defaults(conn: &Connection) {
        let count: i64 = {
            let mut rows = conn
                .query("SELECT COUNT(*) FROM areas", ())
                .await
                .unwrap_or_else(|_| panic!("seed count"));
            let row = rows.next().await.expect("seed row").expect("seed value");
            row_i64(&row, 0)
        };
        if count == 0 {
            for (idx, area) in Area::defaults().into_iter().enumerate() {
                let _ = conn
                    .execute(
                        "INSERT INTO areas (name, is_ac, table_count, sort_order)
                         VALUES (?1, ?2, ?3, ?4)",
                        (
                            area.name,
                            area.is_ac as i64,
                            area.table_count as i64,
                            idx as i64,
                        ),
                    )
                    .await;
            }
        }
        let _ = conn
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES ('gst_number', '')",
                (),
            )
            .await;
        let _ = conn
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES ('ac_rate', '0.06')",
                (),
            )
            .await;
    }

    /// Loads areas in display order.
    pub fn load_areas(&self) -> Vec<Area> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = match conn
                .query(
                    "SELECT name, is_ac, table_count FROM areas ORDER BY sort_order, name",
                    (),
                )
                .await
            {
                Ok(rows) => rows,
                Err(_) => return Area::defaults(),
            };
            let mut areas = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                areas.push(Area {
                    name: row_string(&row, 0),
                    is_ac: row_i64(&row, 1) != 0,
                    table_count: row_i64(&row, 2).max(0) as usize,
                });
            }
            if areas.is_empty() {
                Area::defaults()
            } else {
                areas
            }
        })
    }

    /// Replaces the entire area configuration. Any in-memory physical tables
    /// are rebuilt by the caller afterwards.
    pub fn replace_areas(&self, areas: &[Area]) -> Result<(), String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute("DELETE FROM tables", ())
                .await
                .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM areas", ())
                .await
                .map_err(|e| e.to_string())?;
            for (i, area) in areas.iter().enumerate() {
                conn.execute(
                    "INSERT INTO areas (name, is_ac, table_count, sort_order) VALUES (?1, ?2, ?3, ?4)",
                    (
                        area.name.clone(),
                        area.is_ac as i64,
                        area.table_count as i64,
                        i as i64,
                    ),
                )
                .await
                .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }

    pub fn get_setting(&self, key: &str) -> String {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = match conn
                .query(
                    "SELECT value FROM settings WHERE key = ?1",
                    [key.to_string()],
                )
                .await
            {
                Ok(rows) => rows,
                Err(_) => return String::new(),
            };
            match rows.next().await {
                Ok(Some(row)) => row_string(&row, 0),
                _ => String::new(),
            }
        })
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = ?2",
                (key.to_string(), value.to_string()),
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        })
    }

    pub fn load_offers(&self) -> Vec<Offer> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = match conn
                .query(
                    "SELECT id, name, discount_percent FROM offers ORDER BY id",
                    (),
                )
                .await
            {
                Ok(rows) => rows,
                Err(_) => return Vec::new(),
            };
            let mut offers = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                offers.push(Offer {
                    id: row_i64(&row, 0) as u32,
                    name: row_string(&row, 1),
                    discount_percent: row_f64(&row, 2),
                });
            }
            offers
        })
    }

    /// Replaces the entire offer list (ids are reassigned on reload).
    pub fn replace_offers(&self, offers: &[Offer]) -> Result<(), String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute("DELETE FROM offers", ())
                .await
                .map_err(|e| e.to_string())?;
            for offer in offers {
                conn.execute(
                    "INSERT INTO offers (name, discount_percent) VALUES (?1, ?2)",
                    (offer.name.clone(), offer.discount_percent),
                )
                .await
                .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }

    // -- orders ---------------------------------------------------------------

    /// Next bill number: one past the highest stored order id across both paid
    /// and open orders.
    pub fn next_bill_number(&self) -> u32 {
        self.scalar_i64(
            "SELECT COALESCE(MAX(id), 0) FROM (
                 SELECT id FROM orders
                 UNION ALL
                 SELECT id FROM open_orders
             )",
        )
        .saturating_add(1) as u32
    }

    /// Persists a paid order with its line items. Called once at billing time;
    /// the payment mode is recorded later, when the order is closed.
    pub fn save_paid_order(
        &self,
        order: &Order,
        mobile: &str,
        totals: &BillTotals,
    ) -> Result<(), String> {
        self.rt.block_on(async {
            let mut conn = self.conn.lock().await;
            let tx = conn.transaction().await.map_err(|e| e.to_string())?;
            let mode_str = order.payment_mode.map_or("", |m| m.label());
            tx.execute(
                "INSERT INTO orders (id, label, service, table_number, area, customer_mobile,
                                     subtotal, discount, ac_charge, tax, total, payment_mode, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'PAID')",
                (
                    i64::from(order.id),
                    order.label.clone(),
                    order.service.label(),
                    order.table_number.map(|number| number as i64),
                    order.area.clone(),
                    mobile,
                    totals.subtotal,
                    totals.discount,
                    totals.ac_charge,
                    totals.gst,
                    totals.total,
                    mode_str,
                ),
            )
            .await
            .map_err(|e| e.to_string())?;

            for line in &order.cart {
                tx.execute(
                    "INSERT INTO order_items (order_id, name, unit_price, qty, line_total, notes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    (
                        i64::from(order.id),
                        line.name.clone(),
                        line.unit_price,
                        i64::from(line.qty),
                        line.total(),
                        line.note.as_deref().unwrap_or(""),
                    ),
                )
                .await
                .map_err(|e| e.to_string())?;
            }
            tx.commit().await.map_err(|e| e.to_string())
        })
    }

    /// Records how a paid bill was settled. Called when the order is closed.
    pub fn update_payment_mode(&self, order_id: u32, payment_mode: &str) -> Result<(), String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute(
                "UPDATE orders SET payment_mode = ?2 WHERE id = ?1",
                (i64::from(order_id), payment_mode),
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        })
    }

    // -- open orders (unpaid work-in-progress, survives restarts) -------------

    /// Writes the full current state of an unpaid order (upsert).
    pub fn save_open_order(&self, order: &Order) -> Result<(), String> {
        self.rt.block_on(async {
            let mut conn = self.conn.lock().await;
            let tx = conn.transaction().await.map_err(|e| e.to_string())?;
            tx.execute(
                "DELETE FROM open_order_items WHERE order_id = ?1",
                [i64::from(order.id)],
            )
            .await
            .map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO open_orders (id, label, service, table_number, area, is_ac, ac_rate, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                     label = ?2, service = ?3, table_number = ?4, area = ?5, is_ac = ?6,
                     ac_rate = ?7, status = ?8",
                (
                    i64::from(order.id),
                    order.label.clone(),
                    order.service.label(),
                    order.table_number.map(|number| number as i64),
                    order.area.clone(),
                    order.is_ac as i64,
                    order.ac_rate,
                    order.status.label(),
                ),
            )
            .await
            .map_err(|e| e.to_string())?;
            for (position, line) in order.cart.iter().enumerate() {
                tx.execute(
                    "INSERT INTO open_order_items (order_id, position, name, unit_price, qty, notes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    (
                        i64::from(order.id),
                        position as i64,
                        line.name.clone(),
                        line.unit_price,
                        i64::from(line.qty),
                        line.note.as_deref().unwrap_or(""),
                    ),
                )
                .await
                .map_err(|e| e.to_string())?;
            }
            tx.commit().await.map_err(|e| e.to_string())
        })
    }

    pub fn delete_open_order(&self, id: u32) -> Result<(), String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute("DELETE FROM open_orders WHERE id = ?1", [i64::from(id)])
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
    }

    /// Rebuilds all unpaid orders (with cart lines) saved by `save_open_order`.
    pub fn load_open_orders(&self) -> Vec<Order> {
        struct Header {
            id: u32,
            label: String,
            service: Option<Service>,
            table_number: Option<usize>,
            area: Option<String>,
            is_ac: bool,
            ac_rate: f64,
            status: OrderStatus,
        }

        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut headers = Vec::new();
            if let Ok(mut rows) = conn
                .query(
                    "SELECT id, label, service, table_number, area, is_ac, ac_rate, status
                     FROM open_orders ORDER BY id",
                    (),
                )
                .await
            {
                while let Ok(Some(row)) = rows.next().await {
                    headers.push(Header {
                        id: row_i64(&row, 0) as u32,
                        label: row_string(&row, 1),
                        service: Service::parse(&row_string(&row, 2)),
                        table_number: row_opt_i64(&row, 3).map(|n| n as usize),
                        area: {
                            let raw = row_string(&row, 4);
                            if raw.is_empty() {
                                None
                            } else {
                                Some(raw)
                            }
                        },
                        is_ac: row_i64(&row, 5) != 0,
                        ac_rate: row_f64(&row, 6),
                        status: OrderStatus::parse(&row_string(&row, 7))
                            .unwrap_or(OrderStatus::Ordering),
                    });
                }
            }

            let mut orders = Vec::new();
            for header in headers {
                let Some(service) = header.service else {
                    continue;
                };
                let mut cart = Vec::new();
                if let Ok(mut item_rows) = conn
                    .query(
                        "SELECT name, unit_price, qty, notes FROM open_order_items
                         WHERE order_id = ?1 ORDER BY position",
                        [i64::from(header.id)],
                    )
                    .await
                {
                    while let Ok(Some(item)) = item_rows.next().await {
                        let note_str = row_string(&item, 3);
                        cart.push(CartLine {
                            name: row_string(&item, 0),
                            unit_price: row_f64(&item, 1),
                            qty: row_i64(&item, 2) as u32,
                            note: if note_str.is_empty() {
                                None
                            } else {
                                Some(note_str)
                            },
                        });
                    }
                }

                orders.push(Order {
                    id: header.id,
                    label: header.label,
                    service,
                    table_number: header.table_number,
                    area: header.area,
                    is_ac: header.is_ac,
                    ac_rate: header.ac_rate,
                    discount_percent: 0.0,
                    cart,
                    cart_index: 0,
                    status: header.status,
                    customer_mobile: None,
                    payment_mode: None,
                    kot_sent_count: 0,
                });
            }
            orders
        })
    }

    // -- reports & historical bills -------------------------------------------

    pub fn get_daily_sales_summary(&self, date_prefix: &str) -> DailySalesSummary {
        let sql = format!(
            "SELECT service, subtotal, discount, ac_charge, tax, total, payment_mode
             FROM orders WHERE substr(created_at, 1, 10) = '{date_prefix}'"
        );
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut summary = DailySalesSummary {
                date: date_prefix.to_string(),
                ..Default::default()
            };
            let mut rows = match conn.query(&sql, ()).await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("QUERY ERROR IN GET_DAILY_SALES: {e}");
                    return summary;
                }
            };

            loop {
                let next_res = rows.next().await;
                match next_res {
                    Ok(Some(row)) => {
                        summary.total_orders += 1;
                        let srv = Service::parse(&row_string(&row, 0));
                        if srv == Some(Service::DineIn) {
                            summary.dine_in_orders += 1;
                        } else {
                            summary.takeout_orders += 1;
                        }
                        summary.subtotal += row_f64(&row, 1);
                        summary.discount += row_f64(&row, 2);
                        summary.ac_charge += row_f64(&row, 3);
                        summary.tax += row_f64(&row, 4);
                        let total = row_f64(&row, 5);
                        summary.total_sales += total;

                        let mode_str = row_string(&row, 6);
                        let mode = PaymentMode::parse(&mode_str);
                        match mode {
                            Some(PaymentMode::Upi) => {
                                summary.upi_count += 1;
                                summary.upi_total += total;
                            }
                            Some(PaymentMode::Cash) => {
                                summary.cash_count += 1;
                                summary.cash_total += total;
                            }
                            Some(PaymentMode::Card) => {
                                summary.card_count += 1;
                                summary.card_total += total;
                            }
                            Some(PaymentMode::PersonCredit) => {
                                summary.person_credit_count += 1;
                                summary.person_credit_total += total;
                            }
                            Some(PaymentMode::HaveItOnHotel) => {
                                summary.have_it_on_hotel_count += 1;
                                summary.have_it_on_hotel_total += total;
                            }
                            None => {
                                summary.other_count += 1;
                                summary.other_total += total;
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        eprintln!("ROW ERROR IN GET_DAILY_SALES: {e}");
                        break;
                    }
                }
            }
            summary
        })
    }

    pub fn search_bills(&self, query: &str) -> Vec<HistoricalBill> {
        let query = query.trim();
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = if query.is_empty() {
                match conn
                    .query(
                        "SELECT id, label, service, customer_mobile, total, payment_mode, created_at
                         FROM orders ORDER BY id DESC LIMIT 50",
                        (),
                    )
                    .await
                {
                    Ok(r) => r,
                    Err(_) => return Vec::new(),
                }
            } else {
                let pattern = format!("%{query}%");
                match conn
                    .query(
                        "SELECT id, label, service, customer_mobile, total, payment_mode, created_at
                         FROM orders
                         WHERE CAST(id AS TEXT) LIKE ?1 OR customer_mobile LIKE ?1 OR label LIKE ?1
                         ORDER BY id DESC LIMIT 50",
                        [pattern],
                    )
                    .await
                {
                    Ok(r) => r,
                    Err(_) => return Vec::new(),
                }
            };

            let mut bills = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                let payment_mode = PaymentMode::parse(&row_string(&row, 5));
                bills.push(HistoricalBill {
                    id: row_i64(&row, 0) as u32,
                    label: row_string(&row, 1),
                    service: Service::parse(&row_string(&row, 2)).unwrap_or(Service::DineIn),
                    customer_mobile: row_string(&row, 3),
                    total: row_f64(&row, 4),
                    payment_mode,
                    created_at: row_string(&row, 6),
                });
            }
            bills
        })
    }

    pub fn load_historical_order(&self, order_id: u32) -> Option<Order> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut order_rows = conn
                .query(
                    "SELECT id, label, service, table_number, area, customer_mobile, payment_mode, discount, subtotal, ac_charge
                     FROM orders WHERE id = ?1",
                    [i64::from(order_id)],
                )
                .await
                .ok()?;
            let row = order_rows.next().await.ok()??;
            let id = row_i64(&row, 0) as u32;
            let label = row_string(&row, 1);
            let service = Service::parse(&row_string(&row, 2)).unwrap_or(Service::DineIn);
            let table_number = row_opt_i64(&row, 3).map(|n| n as usize);
            let area = {
                let raw = row_string(&row, 4);
                if raw.is_empty() {
                    None
                } else {
                    Some(raw)
                }
            };
            let customer_mobile = {
                let m = row_string(&row, 5);
                if m.is_empty() {
                    None
                } else {
                    Some(m)
                }
            };
            let payment_mode = PaymentMode::parse(&row_string(&row, 6));
            let discount = row_f64(&row, 7);
            let subtotal = row_f64(&row, 8);
            let ac_charge = row_f64(&row, 9);
            let is_ac = ac_charge > 0.0;
            let ac_rate = if is_ac && (subtotal - discount) > 0.0 {
                ac_charge / (subtotal - discount)
            } else {
                0.0
            };
            let discount_percent = if subtotal > 0.0 {
                (discount / subtotal) * 100.0
            } else {
                0.0
            };

            let mut item_rows = conn
                .query(
                    "SELECT name, unit_price, qty, notes FROM order_items WHERE order_id = ?1 ORDER BY id",
                    [i64::from(order_id)],
                )
                .await
                .ok()?;
            let mut cart = Vec::new();
            while let Ok(Some(item)) = item_rows.next().await {
                let note_str = row_string(&item, 3);
                cart.push(CartLine {
                    name: row_string(&item, 0),
                    unit_price: row_f64(&item, 1),
                    qty: row_i64(&item, 2) as u32,
                    note: if note_str.is_empty() {
                        None
                    } else {
                        Some(note_str)
                    },
                });
            }

            Some(Order {
                id,
                label,
                service,
                table_number,
                area,
                is_ac,
                ac_rate,
                discount_percent,
                cart,
                cart_index: 0,
                status: OrderStatus::Paid,
                customer_mobile,
                payment_mode,
                kot_sent_count: 0,
            })
        })
    }

    // -- physical tables ------------------------------------------------------

    pub fn load_recent_bills(&self) -> Vec<BillSummary> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = conn
                .query(
                    "SELECT id, label, service, total, payment_mode FROM orders ORDER BY id DESC LIMIT 5",
                    (),
                )
                .await
                .unwrap();

            let mut bills = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                let payment_mode = PaymentMode::parse(&row_string(&row, 4));
                bills.push(BillSummary {
                    id: row_i64(&row, 0) as u32,
                    label: row_string(&row, 1),
                    service: Service::parse(&row_string(&row, 2)).unwrap_or(Service::DineIn),
                    total: row_f64(&row, 3),
                    receipt: String::new(), // Reconstruct on demand
                    payment_mode,
                });
            }
            bills
        })
    }

    /// Saves a kitchen order ticket (KOT) directly to the database.
    pub fn save_kot(
        &self,
        order_id: u32,
        label: &str,
        area: &str,
        item_count: u32,
        ticket_text: &str,
        is_reprint: bool,
    ) -> Result<u32, String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute(
                "INSERT INTO kots (order_id, label, area, item_count, ticket_text, is_reprint)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    i64::from(order_id),
                    label,
                    area,
                    i64::from(item_count),
                    ticket_text,
                    is_reprint as i64,
                ),
            )
            .await
            .map_err(|e| e.to_string())?;

            let mut rows = conn
                .query("SELECT last_insert_rowid()", ())
                .await
                .map_err(|e| e.to_string())?;
            if let Ok(Some(row)) = rows.next().await {
                Ok(row_i64(&row, 0) as u32)
            } else {
                Ok(0)
            }
        })
    }

    /// Loads the most recent kitchen order tickets (newest first, up to 10).
    pub fn load_recent_kots(&self) -> Vec<KotSummary> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = match conn
                .query(
                    "SELECT id, order_id, label, area, item_count, ticket_text, is_reprint, created_at
                     FROM kots ORDER BY id DESC LIMIT 10",
                    (),
                )
                .await
            {
                Ok(r) => r,
                Err(_) => return Vec::new(),
            };

            let mut kots = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                kots.push(KotSummary {
                    id: row_i64(&row, 0) as u32,
                    order_id: row_i64(&row, 1) as u32,
                    label: row_string(&row, 2),
                    area: row_string(&row, 3),
                    item_count: row_i64(&row, 4) as u32,
                    ticket_text: row_string(&row, 5),
                    is_reprint: row_i64(&row, 6) != 0,
                    created_at: row_string(&row, 7),
                });
            }
            kots
        })
    }

    /// Saves a daily sales summary / Z-Report directly to the database.
    pub fn save_z_report(
        &self,
        report_date: &str,
        gross_sales: f64,
        net_sales: f64,
        bill_count: u32,
        report_text: &str,
    ) -> Result<u32, String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute(
                "INSERT INTO z_reports (report_date, gross_sales, net_sales, bill_count, report_text)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    report_date,
                    gross_sales,
                    net_sales,
                    i64::from(bill_count),
                    report_text,
                ),
            )
            .await
            .map_err(|e| e.to_string())?;

            let mut rows = conn
                .query("SELECT last_insert_rowid()", ())
                .await
                .map_err(|e| e.to_string())?;
            if let Ok(Some(row)) = rows.next().await {
                Ok(row_i64(&row, 0) as u32)
            } else {
                Ok(0)
            }
        })
    }
    /// Upserts the state of one physical table.
    pub fn upsert_table(&self, table: &PhysicalTable) -> Result<(), String> {
        let updated_at = Local::now().format(TIMESTAMP_FORMAT).to_string();
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute(
                "INSERT INTO tables (area, number, status, order_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(area, number) DO UPDATE SET
                     status = ?3, order_id = ?4, updated_at = ?5",
                (
                    table.area.clone(),
                    table.number as i64,
                    table.status.label(),
                    table.order_id.map(i64::from),
                    updated_at,
                ),
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        })
    }

    /// Loads every stored table. Tables in the Cleaning state whose window has
    /// elapsed (relative to `now`) come back as Ready; tables that are unknown
    /// to the current layout are ignored.
    pub fn load_tables(&self, areas: &[Area], now: DateTime<Local>) -> Vec<PhysicalTable> {
        #[allow(clippy::type_complexity)]
        let raw: Vec<(String, usize, String, Option<u32>, Option<NaiveDateTime>)> =
            self.rt.block_on(async {
                let conn = self.conn.lock().await;
                let mut out = Vec::new();
                let Ok(mut stmt) = conn
                    .prepare(
                        "SELECT area, number, status, order_id, updated_at FROM tables ORDER BY rowid",
                    )
                    .await
                else {
                    return out;
                };
                let Ok(mut rows) = stmt.query(()).await else {
                    return out;
                };
                while let Ok(Some(row)) = rows.next().await {
                    let updated_at =
                        NaiveDateTime::parse_from_str(&row_string(&row, 4), TIMESTAMP_FORMAT).ok();
                    out.push((
                        row_string(&row, 0),
                        row_i64(&row, 1) as usize,
                        row_string(&row, 2),
                        row_opt_i64(&row, 3).map(|id| id as u32),
                        updated_at,
                    ));
                }
                out
            });

        raw.into_iter()
            .filter_map(|(area_raw, number, status_raw, order_id, updated_at)| {
                let area = areas.iter().find(|a| a.name == area_raw)?;
                if number < 1 || number > area.table_count {
                    return None;
                }
                let mut status = TableStatus::parse(&status_raw)?;
                let mut dirty_since = None;
                if status == TableStatus::Dirty {
                    match updated_at {
                        Some(since) => {
                            let elapsed = now.naive_local() - since;
                            if elapsed.num_minutes() >= CLEANING_MINUTES {
                                status = TableStatus::Ready;
                            } else {
                                dirty_since = Some(
                                    now - chrono::Duration::minutes(elapsed.num_minutes().max(0)),
                                );
                            }
                        }
                        None => status = TableStatus::Ready,
                    }
                }
                Some(PhysicalTable {
                    number,
                    area: area_raw.clone(),
                    status,
                    order_id,
                    dirty_since,
                })
            })
            .collect()
    }

    // -- helpers --------------------------------------------------------------

    async fn scalar_async(&self, sql: &str) -> i64 {
        let conn = self.conn.lock().await;
        let mut rows = match conn.query(sql, ()).await {
            Ok(rows) => rows,
            Err(_) => return 0,
        };
        match rows.next().await {
            Ok(Some(row)) => row_i64(&row, 0),
            _ => 0,
        }
    }

    fn scalar_i64(&self, sql: &str) -> i64 {
        self.rt.block_on(self.scalar_async(sql))
    }

    #[cfg(test)]
    async fn scalar_string_async(&self, sql: &str) -> String {
        let conn = self.conn.lock().await;
        let mut rows = conn.query(sql, ()).await.expect("query");
        let row = rows.next().await.expect("row").expect("value");
        row_string(&row, 0)
    }

    #[cfg(test)]
    fn scalar_string(&self, sql: &str) -> String {
        self.rt.block_on(self.scalar_string_async(sql))
    }

    #[cfg(test)]
    async fn scalar_f64_async(&self, sql: &str) -> f64 {
        let conn = self.conn.lock().await;
        let mut rows = conn.query(sql, ()).await.expect("query");
        let row = rows.next().await.expect("row").expect("value");
        row_f64(&row, 0)
    }

    #[cfg(test)]
    fn scalar_f64(&self, sql: &str) -> f64 {
        self.rt.block_on(self.scalar_f64_async(sql))
    }

    #[cfg(test)]
    async fn scalar_optional_string_async(&self, sql: &str) -> Option<String> {
        let conn = self.conn.lock().await;
        let mut rows = conn.query(sql, ()).await.expect("query");
        let row = rows.next().await.expect("row").expect("value");
        match row.get_value(0).expect("column") {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    #[cfg(test)]
    fn scalar_optional_string(&self, sql: &str) -> Option<String> {
        self.rt.block_on(self.scalar_optional_string_async(sql))
    }
}

// -- row extraction helpers ---------------------------------------------------

fn row_string(row: &Row, idx: usize) -> String {
    match row.get_value(idx) {
        Ok(Value::Text(text)) => text,
        _ => String::new(),
    }
}

fn row_i64(row: &Row, idx: usize) -> i64 {
    match row.get_value(idx) {
        Ok(Value::Integer(n)) => n,
        _ => 0,
    }
}

fn row_f64(row: &Row, idx: usize) -> f64 {
    match row.get_value(idx) {
        Ok(Value::Real(f)) => f,
        Ok(Value::Integer(n)) => n as f64,
        _ => 0.0,
    }
}

fn row_opt_i64(row: &Row, idx: usize) -> Option<i64> {
    match row.get_value(idx) {
        Ok(Value::Integer(n)) => Some(n),
        _ => None,
    }
}

#[cfg(test)]
impl Database {
    pub fn open_for_tests() -> Self {
        let id = NEXT_TEST_DB_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "dinein-billing-test-{}-{}.db",
            std::process::id(),
            id
        ));
        let _ = std::fs::remove_file(&path);
        Self::open(&path).expect("open test database")
    }

    #[allow(dead_code)]
    pub fn open_serialised(path: &Path) -> Self {
        Self::open(path).expect("open serialised database")
    }
}

#[cfg(test)]
static NEXT_TEST_DB_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
mod tests {
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
    }
}
