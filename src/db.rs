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
    BillTotals, CartLine, MenuItem, Order, OrderStatus, PhysicalTable, Service, TableArea,
    TableStatus, CLEANING_MINUTES,
};

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

const SCHEMA: [&str; 6] = [
    "CREATE TABLE IF NOT EXISTS menu_items (
         name     TEXT PRIMARY KEY,
         category TEXT NOT NULL,
         unit     TEXT NOT NULL DEFAULT '',
         price    REAL NOT NULL CHECK (price >= 0)
     )",
    "CREATE TABLE IF NOT EXISTS orders (
         id              INTEGER PRIMARY KEY,
         label           TEXT NOT NULL,
         service         TEXT NOT NULL,
         table_number    INTEGER,
         area            TEXT,
         customer_mobile TEXT NOT NULL,
         subtotal        REAL NOT NULL,
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
         line_total REAL NOT NULL
     )",
    "CREATE TABLE IF NOT EXISTS open_orders (
         id           INTEGER PRIMARY KEY,
         label        TEXT NOT NULL,
         service      TEXT NOT NULL,
         table_number INTEGER,
         area         TEXT,
         status       TEXT NOT NULL
     )",
    "CREATE TABLE IF NOT EXISTS open_order_items (
         order_id   INTEGER NOT NULL REFERENCES open_orders(id) ON DELETE CASCADE,
         position   INTEGER NOT NULL,
         name       TEXT NOT NULL,
         unit_price REAL NOT NULL,
         qty        INTEGER NOT NULL CHECK (qty > 0),
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
            Ok(())
        })
    }

    // -- menu -----------------------------------------------------------------

    pub fn load_menu(&self) -> Vec<MenuItem> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = match conn
                .query(
                    "SELECT category, name, unit, price FROM menu_items ORDER BY rowid",
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
                });
            }
            items
        })
    }

    pub fn menu_is_empty(&self) -> bool {
        self.scalar_i64("SELECT COUNT(*) FROM menu_items") == 0
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
                    "INSERT INTO menu_items (name, category, unit, price) VALUES (?1, ?2, ?3, ?4)",
                    (
                        item.name.clone(),
                        item.category.clone(),
                        item.unit.clone(),
                        item.price,
                    ),
                )
                .await
                .map_err(|e| format!("{e} (item: {})", item.name))?;
            }
            tx.commit().await.map_err(|e| e.to_string())
        })
    }

    // -- orders ---------------------------------------------------------------

    /// Next bill number: one past the highest stored order id.
    pub fn next_bill_number(&self) -> u32 {
        self.scalar_i64("SELECT COALESCE(MAX(id), 0) FROM orders")
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
            tx.execute(
                "INSERT INTO orders (id, label, service, table_number, area, customer_mobile,
                                     subtotal, ac_charge, tax, total, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'PAID')",
                (
                    i64::from(order.id),
                    order.label.clone(),
                    order.service.label(),
                    order.table_number.map(|number| number as i64),
                    order.area.map(|area| area.label()),
                    mobile,
                    totals.subtotal,
                    totals.ac_charge,
                    totals.gst,
                    totals.total,
                ),
            )
            .await
            .map_err(|e| e.to_string())?;

            for line in &order.cart {
                tx.execute(
                    "INSERT INTO order_items (order_id, name, unit_price, qty, line_total)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    (
                        i64::from(order.id),
                        line.name.clone(),
                        line.unit_price,
                        i64::from(line.qty),
                        line.total(),
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
                "INSERT INTO open_orders (id, label, service, table_number, area, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                     label = ?2, service = ?3, table_number = ?4, area = ?5, status = ?6",
                (
                    i64::from(order.id),
                    order.label.clone(),
                    order.service.label(),
                    order.table_number.map(|number| number as i64),
                    order.area.map(|area| area.label()),
                    order.status.label(),
                ),
            )
            .await
            .map_err(|e| e.to_string())?;
            for (position, line) in order.cart.iter().enumerate() {
                tx.execute(
                    "INSERT INTO open_order_items (order_id, position, name, unit_price, qty)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    (
                        i64::from(order.id),
                        position as i64,
                        line.name.clone(),
                        line.unit_price,
                        i64::from(line.qty),
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
            area: Option<TableArea>,
            status: OrderStatus,
        }

        let mut headers: Vec<Header> = self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut out = Vec::new();
            let Ok(mut stmt) = conn
                .prepare(
                    "SELECT id, label, service, table_number, area, status
                     FROM open_orders ORDER BY id",
                )
                .await
            else {
                return out;
            };
            let Ok(mut rows) = stmt.query(()).await else {
                return out;
            };
            while let Ok(Some(row)) = rows.next().await {
                out.push(Header {
                    id: row_i64(&row, 0) as u32,
                    label: row_string(&row, 1),
                    service: Service::parse(&row_string(&row, 2)),
                    table_number: row_opt_i64(&row, 3).map(|n| n as usize),
                    area: TableArea::parse(&row_string(&row, 4)),
                    status: OrderStatus::parse(&row_string(&row, 5))
                        .unwrap_or(OrderStatus::Ordering),
                });
            }
            out
        });

        headers
            .drain(..)
            .filter_map(|header| {
                let service = header.service?;
                let id = header.id;
                let cart: Vec<CartLine> = self.rt.block_on(async {
                    let conn = self.conn.lock().await;
                    let mut cart = Vec::new();
                    let Ok(mut stmt) = conn
                        .prepare(
                            "SELECT name, unit_price, qty FROM open_order_items
                             WHERE order_id = ?1 ORDER BY position",
                        )
                        .await
                    else {
                        return cart;
                    };
                    let Ok(mut items) = stmt.query([i64::from(id)]).await else {
                        return cart;
                    };
                    while let Ok(Some(item)) = items.next().await {
                        cart.push(CartLine {
                            name: row_string(&item, 0),
                            unit_price: row_f64(&item, 1),
                            qty: row_i64(&item, 2) as u32,
                        });
                    }
                    cart
                });
                Some(Order {
                    id,
                    label: header.label,
                    service,
                    table_number: header.table_number,
                    area: header.area,
                    cart,
                    cart_index: 0,
                    status: header.status,
                })
            })
            .collect()
    }

    // -- physical tables ------------------------------------------------------

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
                    table.area.label(),
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
    pub fn load_tables(&self, now: DateTime<Local>) -> Vec<PhysicalTable> {
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
                let area = TableArea::parse(&area_raw)?;
                if number < 1 || number > area.table_count() {
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
                    area,
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
        Ok(Value::Real(x)) => x as i64,
        _ => 0,
    }
}

fn row_f64(row: &Row, idx: usize) -> f64 {
    match row.get_value(idx) {
        Ok(Value::Real(x)) => x,
        Ok(Value::Integer(n)) => n as f64,
        _ => 0.0,
    }
}

fn row_opt_i64(row: &Row, idx: usize) -> Option<i64> {
    match row.get_value(idx) {
        Ok(Value::Integer(n)) => Some(n),
        Ok(Value::Real(x)) => Some(x as i64),
        _ => None,
    }
}

#[cfg(test)]
impl Database {
    /// Opening is serialised because the Turso engine takes a process-wide
    /// lock while initialising a database file; concurrent opens from parallel
    /// test threads otherwise fail spuriously with "database is locked".
    fn open_serialised(path: &std::path::Path) -> Self {
        use std::sync::Mutex;

        static OPEN_LOCK: Mutex<()> = Mutex::new(());

        let _guard = OPEN_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::open_local(path).expect("test database")
    }

    /// Unique temporary file per call so tests never share state.
    fn open_for_tests() -> Self {
        use std::sync::atomic::Ordering;
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = NEXT_TEST_DB_ID.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dinein-billing-test-{}-{nanos}-{id}.db",
            std::process::id()
        ));
        Self::open_serialised(&path)
    }
}

#[cfg(test)]
static NEXT_TEST_DB_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
mod tests {
    use super::Database;
    use crate::models::{
        CartLine, MenuItem, Order, OrderStatus, PhysicalTable, Service, TableArea, TableStatus,
    };
    use chrono::Local;

    fn sample_order(id: u32) -> Order {
        Order {
            id,
            label: "Main-T1".to_string(),
            service: Service::DineIn,
            table_number: Some(1),
            area: Some(TableArea::MainHall),
            cart: vec![
                CartLine {
                    name: "Samosa".to_string(),
                    unit_price: 20.0,
                    qty: 2,
                },
                CartLine {
                    name: "Cutting Chai / Special Tea".to_string(),
                    unit_price: 15.0,
                    qty: 1,
                },
            ],
            cart_index: 0,
            status: OrderStatus::Paid,
        }
    }

    #[test]
    fn menu_roundtrip_and_seed_detection() {
        let db = Database::open_for_tests();
        assert!(db.menu_is_empty());

        db.replace_menu(&[MenuItem {
            category: "Desserts".to_string(),
            name: "Gulab Jamun".to_string(),
            unit: "2 pcs".to_string(),
            price: 45.0,
        }])
        .unwrap();
        assert!(!db.menu_is_empty());

        let items = db.load_menu();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Gulab Jamun");
        assert_eq!(items[0].price, 45.0);
    }

    #[test]
    fn paid_orders_persist_with_items_and_drive_bill_numbers() {
        let db = Database::open_for_tests();
        assert_eq!(db.next_bill_number(), 1);

        let order7 = sample_order(7);
        let mut ac = sample_order(8);
        ac.area = Some(TableArea::ACRooms);
        db.save_paid_order(&order7, "9876543210", &order7.totals())
            .unwrap();
        db.save_paid_order(&ac, "9123456780", &ac.totals()).unwrap();

        assert_eq!(db.next_bill_number(), 9);
        assert_eq!(
            db.scalar_string("SELECT customer_mobile FROM orders WHERE id = 7"),
            "9876543210"
        );
        assert_eq!(db.scalar_f64("SELECT total FROM orders WHERE id = 7"), 55.0);
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
        let takeout = Order {
            id: 12,
            label: "TK4".to_string(),
            service: Service::TakeOut,
            table_number: None,
            area: None,
            cart: vec![CartLine {
                name: "Veg / Chicken Momos".to_string(),
                unit_price: 65.0,
                qty: 2,
            }],
            cart_index: 0,
            status: OrderStatus::Ordering,
        };

        db.save_open_order(&dine_in).unwrap();
        db.save_open_order(&takeout).unwrap();
        // Upsert with an extra line must replace the stored cart, not append.
        dine_in.cart.push(CartLine {
            name: "Gulab Jamun".to_string(),
            unit_price: 45.0,
            qty: 1,
        });
        db.save_open_order(&dine_in).unwrap();

        let restored = db.load_open_orders();
        assert_eq!(restored.len(), 2);

        let back = restored.iter().find(|o| o.id == 11).unwrap();
        assert_eq!(back.label, "Main-T1");
        assert_eq!(back.status, OrderStatus::Serving);
        assert_eq!(back.area, Some(TableArea::MainHall));
        assert_eq!(back.cart.len(), 3);
        assert_eq!(back.cart[2].name, "Gulab Jamun");

        let tk = restored.iter().find(|o| o.id == 12).unwrap();
        assert_eq!(tk.service, Service::TakeOut);
        assert!(tk.area.is_none());

        db.delete_open_order(11).unwrap();
        let after = db.load_open_orders();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, 12);
    }

    #[test]
    fn tables_persist_and_cleaning_expires() {
        let now = Local::now();
        let db = Database::open_for_tests();

        let mut dirty_old = PhysicalTable::ready(TableArea::FrontGarden, 1);
        dirty_old.status = TableStatus::Dirty;
        let mut dirty_new = PhysicalTable::ready(TableArea::BackGarden, 8);
        dirty_new.status = TableStatus::Dirty;

        db.upsert_table(&dirty_old).unwrap();
        db.upsert_table(&dirty_new).unwrap();
        db.rt.block_on(async {
            let conn = db.conn.lock().await;
            conn.execute(
                "UPDATE tables SET updated_at = datetime('now', '-15 minutes')
                 WHERE area = 'Front Garden' AND number = 1",
                (),
            )
            .await
            .unwrap();
        });

        let loaded = db.load_tables(now);
        assert_eq!(loaded.len(), 2);
        let fg1 = loaded.iter().find(|t| t.number == 1).unwrap();
        assert_eq!(fg1.status, TableStatus::Ready);
        assert!(fg1.dirty_since.is_none());
        let bg8 = loaded.iter().find(|t| t.number == 8).unwrap();
        assert_eq!(bg8.status, TableStatus::Dirty);
        assert!(bg8.dirty_since.is_some());

        // A table outside the configured layout is ignored.
        db.rt.block_on(async {
            let conn = db.conn.lock().await;
            conn.execute(
                "INSERT INTO tables (area, number, status, updated_at)
                 VALUES ('Front Garden', 99, 'READY', datetime('now'))",
                (),
            )
            .await
            .unwrap();
        });
        assert_eq!(db.load_tables(now).len(), 2);
    }

    #[test]
    fn replace_menu_clears_previous_rows() {
        let db = Database::open_for_tests();
        db.replace_menu(&[
            MenuItem {
                category: "A".to_string(),
                name: "Old".to_string(),
                unit: String::new(),
                price: 10.0,
            },
            MenuItem {
                category: "A".to_string(),
                name: "Newer".to_string(),
                unit: String::new(),
                price: 12.0,
            },
        ])
        .unwrap();
        db.replace_menu(&[MenuItem {
            category: "B".to_string(),
            name: "Only".to_string(),
            unit: String::new(),
            price: 99.0,
        }])
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

    /// Regression guard: the production file may have been written by the
    /// previous libSQL driver (plain SQLite format). The Turso engine must be
    /// able to open it and read the stored data.
    #[test]
    fn opens_existing_production_database_file() {
        let source = std::path::Path::new("data/billing.db");
        if !source.exists() {
            return; // no production database yet on this machine
        }
        let copy =
            std::env::temp_dir().join(format!("dinein-billing-compat-{}.db", std::process::id()));
        std::fs::copy(source, &copy).expect("copy production database");

        let db = Database::open_serialised(&copy);
        assert!(db.next_bill_number() >= 1);
        assert!(!db.menu_is_empty());

        let _ = std::fs::remove_file(&copy);
    }
}
