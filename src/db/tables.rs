use chrono::{DateTime, Local, NaiveDateTime};

use crate::{
    db::{row_f64, row_i64, row_opt_i64, row_string, Database, TIMESTAMP_FORMAT},
    models::{Area, Offer, PhysicalTable, TableStatus, CLEANING_MINUTES},
};

impl Database {
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
}
