use crate::{
    app::App,
    models::{CartLine, Focus, KotSummary},
    receipts::render_receipt,
};

impl App {
    pub fn generate_kot(&mut self) {
        if self.orders.is_empty() {
            self.notify("No active order for KOT.".to_string());
            return;
        }
        if self.order().cart.is_empty() {
            self.notify("Cart is empty - cannot generate KOT.".to_string());
            return;
        }

        let (kot_text, escpos, count, area, label, order_id, is_reprint, is_delta) = {
            let order = self.order();
            let delta_items: Vec<CartLine> = order
                .cart
                .iter()
                .filter_map(|line| {
                    let unsent = line.qty.saturating_sub(line.kot_sent_qty);
                    if unsent > 0 {
                        let mut cl = line.clone();
                        cl.qty = unsent;
                        Some(cl)
                    } else {
                        None
                    }
                })
                .collect();

            let is_delta = !delta_items.is_empty() && order.kot_sent_count > 0;
            let is_reprint = delta_items.is_empty() && order.kot_sent_count > 0;

            let items_to_print = if is_reprint {
                order.cart.clone()
            } else if !delta_items.is_empty() {
                delta_items
            } else {
                order.cart.clone()
            };

            let kot_text = crate::receipts::render_kot_items(
                order,
                &items_to_print,
                is_reprint,
                is_delta,
                &self.restaurant_name,
            );
            let escpos = crate::printer::build_escpos_kot(
                order,
                &items_to_print,
                is_reprint,
                is_delta,
                &self.restaurant_name,
            );
            let count: u32 = items_to_print.iter().map(|l| l.qty).sum();
            let area = order.area.clone().unwrap_or_default();
            let label = order.label.clone();
            let order_id = order.id;
            (
                kot_text, escpos, count, area, label, order_id, is_reprint, is_delta,
            )
        };

        let kot_id = if let Some(db) = &self.database {
            match db.save_kot(order_id, &label, &area, count, &kot_text, is_reprint) {
                Ok(id) => id,
                Err(e) => {
                    self.notify(format!("DB KOT save error: {e}"));
                    0
                }
            }
        } else {
            0
        };

        let now_str = chrono::Local::now().format("%H:%M:%S").to_string();
        self.recent_kots.insert(
            0,
            KotSummary {
                id: kot_id,
                order_id,
                label: label.clone(),
                area,
                item_count: count,
                ticket_text: kot_text.clone(),
                is_reprint,
                created_at: now_str,
                status: "PENDING".to_string(),
            },
        );
        self.recent_kots.truncate(10);

        let _ = crate::printer::send_bytes(
            &self.printer_config,
            Some(&self.printer_config.kot_printer),
            &escpos,
        );

        let order_mut = self.order_mut();
        order_mut.kot_sent_count += 1;
        for line in &mut order_mut.cart {
            line.kot_sent_qty = line.qty;
        }
        self.persist_active_order();

        let tag = if is_reprint {
            "REPRINT"
        } else if is_delta {
            "DELTA"
        } else {
            "NEW"
        };
        self.notify(format!(
            "KOT #{} ({tag}) sent to kitchen ({} items) for {}.",
            if kot_id > 0 { kot_id } else { order_id },
            count,
            label
        ));
    }

    pub fn reprint_full_kot(&mut self) {
        if self.orders.is_empty() {
            self.notify("No active order to reprint KOT.".to_string());
            return;
        }
        if self.order().cart.is_empty() {
            self.notify("Cart is empty.".to_string());
            return;
        }
        let (kot_text, escpos, count, area, label, order_id) = {
            let order = self.order();
            let kot_text = crate::receipts::render_kot_items(
                order,
                &order.cart,
                true,
                false,
                &self.restaurant_name,
            );
            let escpos = crate::printer::build_escpos_kot(
                order,
                &order.cart,
                true,
                false,
                &self.restaurant_name,
            );
            let count: u32 = order.cart.iter().map(|l| l.qty).sum();
            let area = order.area.clone().unwrap_or_default();
            let label = order.label.clone();
            let order_id = order.id;
            (kot_text, escpos, count, area, label, order_id)
        };

        let kot_id = if let Some(db) = &self.database {
            db.save_kot(order_id, &label, &area, count, &kot_text, true)
                .unwrap_or(0)
        } else {
            0
        };
        let now_str = chrono::Local::now().format("%H:%M:%S").to_string();
        self.recent_kots.insert(
            0,
            KotSummary {
                id: kot_id,
                order_id,
                label: label.clone(),
                area,
                item_count: count,
                ticket_text: kot_text.clone(),
                is_reprint: true,
                created_at: now_str,
                status: "PENDING".to_string(),
            },
        );
        self.recent_kots.truncate(10);
        let _ = crate::printer::send_bytes(
            &self.printer_config,
            Some(&self.printer_config.kot_printer),
            &escpos,
        );
        self.notify(format!("Reprinted full KOT for {label} ({} items).", count));
    }

    pub fn open_kds(&mut self) {
        if let Some(db) = &self.database {
            self.kds_kots = db.load_active_kots();
        } else {
            self.kds_kots = self.recent_kots.clone();
        }
        self.kds_index = 0;
        self.focus_return = self.focus;
        self.focus = Focus::KitchenDisplay;
    }

    pub fn kds_bump_status(&mut self) {
        if self.kds_kots.is_empty() {
            return;
        }
        if let Some(kot) = self.kds_kots.get_mut(self.kds_index) {
            let next_status = match kot.status.as_str() {
                "PENDING" => "PREPARING",
                "PREPARING" => "READY",
                "READY" => "SERVED",
                _ => "PENDING",
            };
            kot.status = next_status.to_string();
            let kot_id = kot.id;
            let table = kot.label.clone();
            if let Some(db) = &self.database {
                let _ = db.update_kot_status(kot_id, next_status);
            }
            if next_status == "SERVED" {
                self.notify(format!("KOT #{kot_id} for {table} marked as SERVED!"));
            } else {
                self.notify(format!("KOT #{kot_id} for {table} status: {next_status}"));
            }
        }
    }

    pub fn reprint_selected_recent_bill(&mut self) {
        if let Some(bill) = self.recent_bills.get(self.recent_bill_index) {
            let bill_id = bill.id;
            if let Some(db) = &self.database {
                if let Some(ord) = db.load_historical_order(bill_id) {
                    let receipt = render_receipt(
                        &ord,
                        ord.customer_mobile.as_deref(),
                        &self.gst_number,
                        &self.restaurant_name,
                        &self.restaurant_address,
                        &self.restaurant_contact,
                    );
                    let _ = crate::receipts::print_receipt_text(&receipt);
                    self.notify(format!("Reprinted Bill #{bill_id}!"));
                    return;
                }
            }
            let _ = crate::receipts::print_receipt_text(&bill.receipt);
            self.notify(format!("Reprinted Bill #{bill_id}!"));
        }
    }

    pub fn reprint_selected_recent_kot(&mut self) {
        if let Some(kot) = self.recent_kots.get(self.recent_kot_index) {
            let _ = crate::receipts::print_receipt_text(&kot.ticket_text);
            self.notify(format!("Reprinted KOT #{} for {}!", kot.id, kot.label));
        }
    }
}
