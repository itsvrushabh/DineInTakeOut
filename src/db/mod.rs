//! Database persistence via the Turso engine: menu catalogue, physical table
//! states, unpaid in-progress orders, and paid order history.
//!
//! The database is an embedded Turso file (SQLite-compatible) at
//! `data/billing.db`, created automatically on first run. The same schema and
//! code work against a synced Turso Cloud replica by enabling the crate's
//! `sync` feature later without any table changes.

pub mod kots;
pub mod menu;
pub mod orders;
pub mod reports;
pub mod tables;

#[cfg(test)]
mod tests;

use std::path::Path;

use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio::sync::Mutex;
use turso::{Connection, Row, Value};

use crate::models::Area;

pub(crate) const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

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
         id               INTEGER PRIMARY KEY AUTOINCREMENT,
         order_id         INTEGER NOT NULL REFERENCES orders(id),
         name             TEXT NOT NULL,
         unit_price       REAL NOT NULL,
         qty              INTEGER NOT NULL CHECK (qty > 0),
         line_total       REAL NOT NULL,
         notes            TEXT NOT NULL DEFAULT '',
         is_nc            INTEGER NOT NULL DEFAULT 0,
         discount_percent REAL NOT NULL DEFAULT 0
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
         order_id         INTEGER NOT NULL REFERENCES open_orders(id) ON DELETE CASCADE,
         position         INTEGER NOT NULL,
         name             TEXT NOT NULL,
         unit_price       REAL NOT NULL,
         qty              INTEGER NOT NULL CHECK (qty > 0),
         notes            TEXT NOT NULL DEFAULT '',
         kot_sent_qty     INTEGER NOT NULL DEFAULT 0,
         is_nc            INTEGER NOT NULL DEFAULT 0,
         discount_percent REAL NOT NULL DEFAULT 0,
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
         created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
         status      TEXT NOT NULL DEFAULT 'PENDING'
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
    pub(crate) rt: Runtime,
    /// Serialised access: every operation locks the single connection, which
    /// also satisfies `transaction()`'s `&mut Connection` requirement.
    pub(crate) conn: Mutex<Connection>,
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
        // Purge backups older than 30 days
        Self::purge_old_backups(&backup_dir, 30);
    }

    /// Purges SQLite backup files in `backup_dir` older than `retention_days`.
    pub fn purge_old_backups(backup_dir: &Path, retention_days: u32) -> usize {
        if !backup_dir.exists() || retention_days == 0 {
            return 0;
        }
        let cutoff = chrono::Local::now() - chrono::Duration::days(retention_days as i64);
        let cutoff_str = cutoff.format("%Y-%m-%d").to_string();

        let mut purged = 0;
        if let Ok(entries) = std::fs::read_dir(backup_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.starts_with("billing_") && file_name.ends_with(".db") {
                        let date_part = &file_name[8..file_name.len() - 3];
                        if date_part.len() == 10
                            && date_part < cutoff_str.as_str()
                            && std::fs::remove_file(&path).is_ok()
                        {
                            purged += 1;
                        }
                    }
                }
            }
        }
        purged
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
            let _ = conn
                .execute(
                    "ALTER TABLE open_order_items ADD COLUMN kot_sent_qty INTEGER NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE open_order_items ADD COLUMN is_nc INTEGER NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE open_order_items ADD COLUMN discount_percent REAL NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE order_items ADD COLUMN is_nc INTEGER NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE order_items ADD COLUMN discount_percent REAL NOT NULL DEFAULT 0",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE kots ADD COLUMN status TEXT NOT NULL DEFAULT 'PENDING'",
                    (),
                )
                .await;

            // Seed the dynamic configuration the first time the schema exists.
            Self::seed_defaults(&conn).await;
            Ok(())
        })
    }

    // -- areas / settings / offers seed ---------------------------------------

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

    // -- scalar helpers -------------------------------------------------------

    #[allow(dead_code)]
    pub(crate) async fn scalar_async(&self, sql: &str) -> i64 {
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

    pub(crate) fn scalar_i64(&self, sql: &str) -> i64 {
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
    pub(crate) fn scalar_string(&self, sql: &str) -> String {
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
    pub(crate) fn scalar_f64(&self, sql: &str) -> f64 {
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
    pub(crate) fn scalar_optional_string(&self, sql: &str) -> Option<String> {
        self.rt.block_on(self.scalar_optional_string_async(sql))
    }
}

// -- row extraction helpers ---------------------------------------------------

pub(crate) fn row_string(row: &Row, idx: usize) -> String {
    match row.get_value(idx) {
        Ok(Value::Text(text)) => text,
        _ => String::new(),
    }
}

pub(crate) fn row_i64(row: &Row, idx: usize) -> i64 {
    match row.get_value(idx) {
        Ok(Value::Integer(n)) => n,
        _ => 0,
    }
}

pub(crate) fn row_f64(row: &Row, idx: usize) -> f64 {
    match row.get_value(idx) {
        Ok(Value::Real(f)) => f,
        Ok(Value::Integer(n)) => n as f64,
        _ => 0.0,
    }
}

pub(crate) fn row_opt_i64(row: &Row, idx: usize) -> Option<i64> {
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
