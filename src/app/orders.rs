use crate::{
    app::App,
    models::{OrderStatus, PaymentMode, Service, TableStatus},
    receipts::render_receipt,
};

impl App {
    pub fn open_table_order(&mut self) {
        let area = self.selected_area_name();
        if area.is_empty() {
            self.notify(String::from("No dining areas configured."));
            return;
        }

        let table_info = self
            .physical_tables
            .iter()
            .find(|t| t.area == area && t.number == self.selected_table_index + 1)
            .map(|t| (t.area.clone(), t.number, t.status, t.order_id));

        if let Some((area, table_num, status, order_id)) = table_info {
            if status == TableStatus::Dirty {
                self.clean_selected_table();
                return;
            }

            if status != TableStatus::Ready {
                if let Some(order_id) = order_id {
                    if let Some(order_index) = self.orders.iter().position(|o| o.id == order_id) {
                        self.active_order = order_index;
                        self.notify(format!("Switched to {}.", self.order().label));
                    }
                }
                return;
            }

            let id = self.next_order_id;
            self.next_order_id += 1;
            let label = format!(
                "{}-T{}",
                area.split_whitespace().next().unwrap_or("T"),
                table_num
            );

            let (is_ac, ac_rate) = self
                .selected_area()
                .map(|a| (a.is_ac, self.ac_rate))
                .unwrap_or((false, 0.0));

            let order = crate::models::Order {
                id,
                label,
                service: Service::DineIn,
                table_number: Some(table_num),
                area: Some(area.clone()),
                is_ac,
                ac_rate,
                discount_percent: 0.0,
                cart: Vec::new(),
                cart_index: 0,
                status: OrderStatus::Ordering,
                customer_mobile: None,
                payment_mode: None,
                kot_sent_count: 0,
            };

            self.orders.push(order);
            self.active_order = self.orders.len() - 1;

            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == area && t.number == table_num)
            {
                pt.status = TableStatus::Ordering;
                pt.order_id = Some(id);
            }

            self.persist_active_order();
            self.persist_table(&area, table_num);
            self.notify(format!("Opened order for Table {}", table_num));
        }
    }

    pub fn open_takeout_order(&mut self) {
        let id = self.next_order_id;
        self.next_order_id += 1;
        let tk_count = self.next_takeout_id;
        self.next_takeout_id += 1;
        let order = crate::models::Order {
            id,
            label: format!("TK{}", tk_count),
            service: Service::TakeOut,
            table_number: None,
            area: None,
            is_ac: false,
            ac_rate: 0.0,
            discount_percent: 0.0,
            cart: Vec::new(),
            cart_index: 0,
            status: OrderStatus::Ordering,
            customer_mobile: None,
            payment_mode: None,
            kot_sent_count: 0,
        };

        self.orders.push(order);
        self.active_order = self.orders.len() - 1;
        self.persist_active_order();
        self.notify(format!("Opened take-out order TK{}.", tk_count));
    }

    pub fn cancel_order(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No active order to cancel."));
            return;
        }
        let order = self.order();
        let table_info = order
            .table_number
            .and_then(|num| order.area.as_ref().map(|area| (num, area.clone())));
        let closed_id = order.id;

        self.orders.remove(self.active_order);
        if self.active_order >= self.orders.len() && !self.orders.is_empty() {
            self.active_order = self.orders.len() - 1;
        }

        if let Some(db) = &self.database {
            let _ = db.delete_open_order(closed_id);
        }

        if let Some((table_num, area)) = table_info {
            self.transition_table_status(&area, table_num, TableStatus::Ready);
        }
        self.notify(format!("Order #{} cancelled.", closed_id));
    }

    pub fn close_order(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No orders to close."));
            return;
        }

        let order = self.order();
        if order.status == OrderStatus::Paid {
            self.payment_mode_index = order
                .payment_mode
                .and_then(|m| PaymentMode::all().iter().position(|&x| x == m))
                .unwrap_or(0);
            self.close_on_payment = true;
            self.focus_return = self.focus;
            self.focus = crate::models::Focus::PaymentMode;
        } else if order.cart.is_empty() {
            self.cancel_order();
        } else {
            self.notify(String::from(
                "Cannot close order with items. Generate bill first.",
            ));
        }
    }

    pub fn update_paid_order_payment_mode(&mut self, mode: PaymentMode) {
        if self.orders.is_empty() || self.order().status != OrderStatus::Paid {
            return;
        }
        let order_id = self.order().id;
        self.order_mut().payment_mode = Some(mode);

        if let Some(db) = &self.database {
            if let Err(error) = db.update_payment_mode(order_id, mode.label()) {
                self.notify(format!("Could not save payment mode: {error}"));
            }
        }

        let gst_number = self.gst_number.clone();
        let restaurant_name = self.restaurant_name.clone();
        let restaurant_address = self.restaurant_address.clone();
        let restaurant_contact = self.restaurant_contact.clone();
        let order = self.order();
        let updated_receipt = render_receipt(
            order,
            order.customer_mobile.as_deref(),
            &gst_number,
            &restaurant_name,
            &restaurant_address,
            &restaurant_contact,
        );

        if let Some(summary) = self.recent_bills.iter_mut().find(|b| b.id == order_id) {
            summary.payment_mode = Some(mode);
            summary.receipt = updated_receipt;
        }

        self.notify(format!(
            "Bill #{} updated with payment type {}.",
            order_id,
            mode.display()
        ));
    }

    pub fn advance_stage(&mut self) {
        if self.orders.is_empty() {
            self.notify(String::from("No active order."));
            return;
        }

        let (current, table_number, area) = {
            let order = self.order();
            (order.status, order.table_number, order.area.clone())
        };

        match current.advance() {
            Some(next) => {
                self.order_mut().status = next;
                let label = self.order().label.clone();

                if let (Some(table_num), Some(area)) = (table_number, area) {
                    self.transition_table_status(&area, table_num, next.table_status());
                }
                self.persist_active_order();
                let stage = match next {
                    OrderStatus::Serving => "now being served",
                    OrderStatus::BillRequested => "ready for bill",
                    _ => "advanced",
                };
                self.notify(format!("Table {label} — {stage}."));
            }
            None => {
                self.notify(String::from(
                    "Order already at final stage (use p to bill, c to close).",
                ));
            }
        }
    }
}
