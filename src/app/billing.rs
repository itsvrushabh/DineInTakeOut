use crate::{
    app::App,
    models::{BillSummary, Focus, OrderStatus, PaymentMode, TableStatus, CLEANING_MINUTES},
    receipts::render_receipt,
};

impl App {
    pub fn begin_billing(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No active order."));
            return;
        }

        if self.order().status == OrderStatus::Paid {
            self.payment_mode_index = self
                .order()
                .payment_mode
                .and_then(|m| PaymentMode::all().iter().position(|&x| x == m))
                .unwrap_or(0);
            self.close_on_payment = false;
            self.focus_return = self.focus;
            self.focus = Focus::PaymentMode;
            self.notify(format!(
                "Select payment type for Bill #{} (1/c: Cash, 2/u: UPI, 3/d: Card)",
                self.order().id
            ));
            return;
        }

        if self.order().cart.is_empty() {
            self.notify(String::from("Cart is empty."));
            return;
        }

        self.focus_return = self.focus;
        self.mobile_buffer.clear();
        self.focus = Focus::MobileEntry;
    }

    pub fn complete_billing(&mut self, mobile: &str, offer_id: Option<u32>) {
        if self.orders.is_empty() || self.order().status == OrderStatus::Paid {
            return;
        }
        if self.order().cart.is_empty() {
            self.notify(String::from("Cart is empty."));
            return;
        }
        if self.order().totals().total <= 0.0 {
            self.notify(String::from("Total must be greater than zero."));
            return;
        }

        if let Some(id) = offer_id {
            if let Some(offer) = self.offers.iter().find(|o| o.id == id) {
                self.order_mut().discount_percent = offer.discount_percent;
            }
        }

        let customer_mobile = if mobile.trim().is_empty() {
            None
        } else {
            Some(mobile.trim())
        };
        self.order_mut().customer_mobile = customer_mobile.map(String::from);
        let gst_number = self.gst_number.clone();
        let restaurant_name = self.restaurant_name.clone();
        let restaurant_address = self.restaurant_address.clone();
        let restaurant_contact = self.restaurant_contact.clone();
        let (order_id, order_label, order_service, table_info, totals, bill_text) = {
            let order = self.order();
            let table_info = match (order.table_number, order.area.clone()) {
                (Some(table_num), Some(area)) => Some((table_num, area)),
                _ => None,
            };
            (
                order.id,
                order.label.clone(),
                order.service,
                table_info,
                order.totals(),
                render_receipt(
                    order,
                    customer_mobile,
                    &gst_number,
                    &restaurant_name,
                    &restaurant_address,
                    &restaurant_contact,
                ),
            )
        };

        let mut db_error: Option<String> = None;
        if let Some(db) = &self.database {
            if let Err(error) = db.save_paid_order(self.order(), mobile.trim(), &totals) {
                db_error = Some(error);
            }
            let _ = db.delete_open_order(order_id);
        }
        if let Some(error) = db_error {
            self.notify(format!("DB save failed: {error}"));
        }

        let summary = BillSummary {
            id: order_id,
            label: order_label.clone(),
            service: order_service,
            total: totals.total,
            receipt: bill_text.clone(),
            payment_mode: self.order().payment_mode,
        };
        self.recent_bills.insert(0, summary);
        self.recent_bills.truncate(5);
        self.recent_bill_index = 0;
        let escpos_bytes = crate::printer::build_escpos_receipt(
            self.order(),
            &restaurant_name,
            &restaurant_address,
            &restaurant_contact,
            &gst_number,
            customer_mobile,
            self.printer_config.cash_drawer_enabled,
        );
        let _ = crate::printer::send_bytes(
            &self.printer_config,
            Some(&self.printer_config.bill_printer),
            &escpos_bytes,
        );
        self.queue_effect(crate::app::AppEffect::PrintReceipt {
            printer: self.printer_config.bill_printer.clone(),
            data: escpos_bytes,
        });
        self.queue_effect(crate::app::AppEffect::SaveFile {
            path: std::path::PathBuf::from(format!("receipts/bill_{order_id}.txt")),
            content: bill_text.clone(),
        });

        self.order_mut().status = OrderStatus::Paid;

        if let Some((table_num, area)) = table_info {
            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == area && t.number == table_num)
            {
                pt.status = TableStatus::Paid;
            }
            self.persist_table(&area, table_num);
        }
        self.focus = self.focus_return;
    }

    pub fn apply_offer_and_bill(&mut self, index: usize) {
        let offer_id = if index == 0 {
            None
        } else {
            self.offers.get(index - 1).map(|o| o.id)
        };
        let mobile = std::mem::take(&mut self.pending_mobile);
        self.complete_billing(&mobile, offer_id);
    }

    pub fn select_payment_mode(&mut self, mode: PaymentMode) {
        if self.close_on_payment {
            self.perform_close_with_mode(mode);
        } else {
            self.update_paid_order_payment_mode(mode);
            self.focus = self.focus_return;
        }
    }

    pub fn perform_close_with_mode(&mut self, mode: PaymentMode) {
        if self.orders.is_empty() || self.order().status != OrderStatus::Paid {
            return;
        }

        self.update_paid_order_payment_mode(mode);

        let order = self.order();
        let label = order.label.clone();
        let table_info = order
            .table_number
            .and_then(|num| order.area.as_ref().map(|area| (num, area.clone())));
        let closed_id = order.id;
        let mode_display = mode.display();

        self.orders.remove(self.active_order);
        if self.active_order >= self.orders.len() && !self.orders.is_empty() {
            self.active_order = self.orders.len() - 1;
        }
        self.focus = self.focus_return;

        if let Some(db) = &self.database {
            let _ = db.delete_open_order(closed_id);
        }

        if let Some((table_num, area)) = table_info {
            self.transition_table_status(&area, table_num, TableStatus::Dirty);
            self.notify(format!(
                "Closed {label} via {mode_display}. Table {table_num} cleaning — auto-ready in {CLEANING_MINUTES} min."
            ));
            return;
        }

        self.notify(format!("Closed {label} via {mode_display}."));
    }

    pub fn open_split_payment(&mut self) {
        self.split_cash.clear();
        self.split_upi.clear();
        self.split_card.clear();
        self.split_field = 0;
        self.focus = Focus::SplitPayment;
    }

    pub fn split_payment_totals(&self) -> (f64, f64, f64, f64) {
        let bill_total = if self.orders.is_empty() {
            0.0
        } else {
            self.order().totals().total
        };
        let cash: f64 = self.split_cash.trim().parse().unwrap_or(0.0);
        let upi: f64 = self.split_upi.trim().parse().unwrap_or(0.0);
        let card: f64 = self.split_card.trim().parse().unwrap_or(0.0);
        (bill_total, cash, upi, card)
    }

    pub fn confirm_split_payment(&mut self) {
        let (bill_total, cash, upi, card) = self.split_payment_totals();
        let entered = cash + upi + card;
        if entered < bill_total - 0.01 {
            self.notify(format!(
                "Remaining balance of ₹{:.2} must be paid!",
                bill_total - entered
            ));
            return;
        }
        self.select_payment_mode(PaymentMode::Split);
    }

    pub fn update_customer_crm(&mut self) {
        let phone = self.mobile_buffer.trim();
        if phone.len() >= 4 {
            if let Some(db) = &self.database {
                self.customer_crm = Some(db.get_customer_crm_profile(phone));
            }
        } else {
            self.customer_crm = None;
        }
    }

    pub fn show_upi_qr(&mut self) {
        if self.orders.is_empty() {
            self.notify("No active order for UPI QR.".to_string());
            return;
        }
        self.focus_return = self.focus;
        self.focus = Focus::UpiQr;
    }

    pub fn upi_qr_uri_and_blocks(&self) -> Option<(String, Vec<String>, f64)> {
        if self.orders.is_empty() {
            return None;
        }
        let order = self.order();
        let totals = order.totals();
        let uri = format!(
            "upi://pay?pa={}&pn=DineInTakeOut&am={:.2}&cu=INR&tn=Bill-{}",
            self.upi_id, totals.total, order.id
        );
        match crate::receipts::generate_upi_qr_blocks(&uri) {
            Ok(blocks) => Some((uri, blocks, totals.total)),
            Err(_) => None,
        }
    }
}
