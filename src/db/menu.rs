use crate::{
    db::{row_f64, row_i64, row_string, Database},
    models::MenuItem,
};

impl Database {
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
}
