use crate::{
    db::{row_f64, row_i64, row_opt_i64, row_string, Database},
    models::{BillTotals, CartLine, Order, OrderStatus, Service},
};

impl Database {
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
                    "INSERT INTO order_items (order_id, name, unit_price, qty, line_total, notes, is_nc, discount_percent)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    (
                        i64::from(order.id),
                        line.name.clone(),
                        line.unit_price,
                        i64::from(line.qty),
                        line.total(),
                        line.note.as_deref().unwrap_or(""),
                        line.is_complimentary as i64,
                        line.discount_percent,
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
                    "INSERT INTO open_order_items (order_id, position, name, unit_price, qty, notes, kot_sent_qty, is_nc, discount_percent)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    (
                        i64::from(order.id),
                        position as i64,
                        line.name.clone(),
                        line.unit_price,
                        i64::from(line.qty),
                        line.note.as_deref().unwrap_or(""),
                        i64::from(line.kot_sent_qty),
                        line.is_complimentary as i64,
                        line.discount_percent,
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
                        "SELECT name, unit_price, qty, notes, kot_sent_qty, is_nc, discount_percent
                         FROM open_order_items
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
                            kot_sent_qty: row_i64(&item, 4) as u32,
                            is_complimentary: row_i64(&item, 5) != 0,
                            discount_percent: row_f64(&item, 6),
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
}
