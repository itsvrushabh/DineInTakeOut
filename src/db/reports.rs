use crate::{
    db::{row_f64, row_i64, row_opt_i64, row_string, Database},
    models::{
        BillSummary, CartLine, CustomerCrmProfile, DailySalesSummary, HistoricalBill, Order,
        OrderStatus, PaymentMode, SalesAnalytics, Service,
    },
};

impl Database {
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
                            Some(PaymentMode::Split) => {
                                summary.split_count += 1;
                                summary.split_total += total;
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
                    "SELECT name, unit_price, qty, notes, is_nc, discount_percent FROM order_items WHERE order_id = ?1 ORDER BY id",
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
                    kot_sent_qty: row_i64(&item, 2) as u32,
                    is_complimentary: row_i64(&item, 4) != 0,
                    discount_percent: row_f64(&item, 5),
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

    /// Fetches customer CRM profile including lifetime visits, spend, and favorites.
    pub fn get_customer_crm_profile(&self, phone: &str) -> CustomerCrmProfile {
        let phone = phone.trim();
        if phone.is_empty() {
            return CustomerCrmProfile::default();
        }
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut profile = CustomerCrmProfile {
                phone: phone.to_string(),
                ..Default::default()
            };

            if let Ok(mut rows) = conn
                .query(
                    "SELECT COUNT(id), COALESCE(SUM(total), 0.0), MAX(created_at)
                     FROM orders WHERE customer_mobile = ?1",
                    [phone],
                )
                .await
            {
                if let Ok(Some(row)) = rows.next().await {
                    profile.visit_count = row_i64(&row, 0) as usize;
                    profile.total_spent = row_f64(&row, 1);
                    let last = row_string(&row, 2);
                    if !last.is_empty() {
                        profile.last_visit = Some(last);
                    }
                }
            }

            if let Ok(mut item_rows) = conn
                .query(
                    "SELECT oi.name, SUM(oi.qty) as total_qty
                     FROM order_items oi
                     JOIN orders o ON o.id = oi.order_id
                     WHERE o.customer_mobile = ?1
                     GROUP BY oi.name
                     ORDER BY total_qty DESC LIMIT 3",
                    [phone],
                )
                .await
            {
                while let Ok(Some(row)) = item_rows.next().await {
                    let name = row_string(&row, 0);
                    let qty = row_i64(&row, 1) as usize;
                    profile.favorite_items.push((name, qty));
                }
            }

            profile
        })
    }

    /// Aggregates sales analytics for a specific date (YYYY-MM-DD) or all time if date is empty.
    pub fn get_sales_analytics(&self, date_prefix: &str) -> SalesAnalytics {
        self.rt.block_on(async {
            let conn = self.conn.lock().await;
            let mut analytics = SalesAnalytics {
                date: if date_prefix.trim().is_empty() {
                    chrono::Local::now().format("%Y-%m-%d").to_string()
                } else {
                    date_prefix.trim().to_string()
                },
                ..Default::default()
            };

            let filter = if date_prefix.trim().is_empty() {
                "1=1".to_string()
            } else {
                format!("DATE(created_at) = '{}'", date_prefix.trim())
            };

            // Overall totals
            let sql = format!(
                "SELECT COUNT(id), COALESCE(SUM(subtotal), 0.0), COALESCE(SUM(discount), 0.0),
                        COALESCE(SUM(tax), 0.0), COALESCE(SUM(total), 0.0)
                 FROM orders WHERE {filter}"
            );
            if let Ok(mut rows) = conn.query(&sql, ()).await {
                if let Ok(Some(row)) = rows.next().await {
                    analytics.total_orders = row_i64(&row, 0) as usize;
                    analytics.gross_sales = row_f64(&row, 1);
                    analytics.total_discounts = row_f64(&row, 2);
                    analytics.total_tax = row_f64(&row, 3);
                    analytics.cgst = analytics.total_tax / 2.0;
                    analytics.sgst = analytics.total_tax / 2.0;
                    analytics.net_sales = row_f64(&row, 4);
                    if analytics.total_orders > 0 {
                        analytics.avg_bill_value =
                            analytics.net_sales / analytics.total_orders as f64;
                    }
                }
            }

            // Payment breakdown
            let psql = format!(
                "SELECT payment_mode, COUNT(id), COALESCE(SUM(total), 0.0)
                 FROM orders WHERE {filter} GROUP BY payment_mode"
            );
            if let Ok(mut rows) = conn.query(&psql, ()).await {
                while let Ok(Some(row)) = rows.next().await {
                    let mode = row_string(&row, 0);
                    let count = row_i64(&row, 1) as usize;
                    let total = row_f64(&row, 2);
                    analytics.payment_breakdown.push((
                        if mode.is_empty() {
                            "UNSPECIFIED".to_string()
                        } else {
                            mode
                        },
                        count,
                        total,
                    ));
                }
            }

            // Top items by quantity
            let isql = format!(
                "SELECT oi.name, SUM(oi.qty), SUM(oi.line_total)
                 FROM order_items oi
                 JOIN orders o ON o.id = oi.order_id
                 WHERE {filter}
                 GROUP BY oi.name
                 ORDER BY SUM(oi.qty) DESC LIMIT 5"
            );
            if let Ok(mut rows) = conn.query(&isql, ()).await {
                while let Ok(Some(row)) = rows.next().await {
                    let name = row_string(&row, 0);
                    let qty = row_i64(&row, 1) as usize;
                    let total = row_f64(&row, 2);
                    analytics.top_items.push((name, qty, total));
                }
            }

            analytics
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
}
