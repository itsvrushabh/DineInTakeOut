use crate::{
    db::{row_i64, row_string, Database},
    models::KotSummary,
};

impl Database {
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
                "INSERT INTO kots (order_id, label, area, item_count, ticket_text, is_reprint, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'PENDING')",
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

    /// Updates the prep/serving status of a KOT ("PENDING", "PREPARING", "READY", "SERVED").
    pub fn update_kot_status(&self, id: u32, status: &str) -> Result<(), String> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            conn.execute(
                "UPDATE kots SET status = ?1 WHERE id = ?2",
                (status, i64::from(id)),
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        })
    }

    /// Loads active KOTs for the Kitchen Display System (all not yet SERVED, newest first).
    pub fn load_active_kots(&self) -> Vec<KotSummary> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = match conn
                .query(
                    "SELECT id, order_id, label, area, item_count, ticket_text, is_reprint, created_at, status
                     FROM kots WHERE status != 'SERVED' ORDER BY id DESC LIMIT 50",
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
                    status: row_string(&row, 8),
                });
            }
            kots
        })
    }

    /// Loads the most recent kitchen order tickets (newest first, up to 10).
    pub fn load_recent_kots(&self) -> Vec<KotSummary> {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut rows = match conn
                .query(
                    "SELECT id, order_id, label, area, item_count, ticket_text, is_reprint, created_at, status
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
                    status: row_string(&row, 8),
                });
            }
            kots
        })
    }
}
