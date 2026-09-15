use fuzzy_matcher::FuzzyMatcher;

use crate::{
    app::App,
    models::{CartLine, Focus, OrderStatus},
};

impl App {
    pub fn visible_items(&self) -> Vec<usize> {
        let q = self.search.trim();
        let cat_filter = self.selected_category_index > 0
            && self.selected_category_index < self.categories.len();
        let target_cat = if cat_filter {
            Some(&self.categories[self.selected_category_index])
        } else {
            None
        };

        if q.is_empty() {
            return self
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, it)| {
                    if let Some(cat) = target_cat {
                        if &it.category == cat {
                            Some(i)
                        } else {
                            None
                        }
                    } else {
                        Some(i)
                    }
                })
                .collect();
        }

        let mut scored: Vec<(i64, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                if let Some(cat) = target_cat {
                    if &it.category != cat {
                        return None;
                    }
                }
                let name_score = self.matcher.fuzzy_match(&it.name, q);
                let cat_score = self.matcher.fuzzy_match(&it.category, q).map(|s| s / 2);
                name_score.or(cat_score).map(|s| (s, i))
            })
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        scored.into_iter().map(|(_, i)| i).collect()
    }

    pub fn add_selected_to_cart(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }

        let vis = self.visible_items();
        if let Some(&idx) = vis.get(self.menu_index) {
            let item = self.items[idx].clone();
            if !item.is_available {
                self.notify(format!("'{}' is currently OUT OF STOCK (86)!", item.name));
                return;
            }
            let order = self.order_mut();
            if let Some(line) = order.cart.iter_mut().find(|l| l.name == item.name) {
                line.qty += 1;
            } else {
                order.cart.push(CartLine::new(&item.name, item.price, 1));
            }
            self.notify(format!("Added {} to {}.", item.name, self.order().label));
            self.persist_active_order();
        }
    }

    pub fn toggle_selected_menu_item_stock(&mut self) {
        let vis = self.visible_items();
        if let Some(&idx) = vis.get(self.menu_index) {
            let item = &mut self.items[idx];
            item.is_available = !item.is_available;
            let name = item.name.clone();
            let is_avail = item.is_available;
            if let Some(db) = &self.database {
                let _ = db.update_menu_item_availability(&name, is_avail);
            }
            let status = if is_avail {
                "AVAILABLE"
            } else {
                "OUT OF STOCK (86)"
            };
            self.notify(format!("Marked '{name}' as {status}."));
        }
    }

    pub fn open_item_note_prompt(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }
        let order = self.order();
        if order.cart.is_empty() || order.cart_index >= order.cart.len() {
            self.notify("No cart line selected to add note.".to_string());
            return;
        }
        self.item_note_buffer = order.cart[order.cart_index]
            .note
            .clone()
            .unwrap_or_default();
        self.focus_return = self.focus;
        self.focus = Focus::ItemNote;
    }

    pub fn save_item_note(&mut self) {
        let note = self.item_note_buffer.trim().to_string();
        let order = self.order_mut();
        if order.cart_index < order.cart.len() {
            order.cart[order.cart_index].note = if note.is_empty() { None } else { Some(note) };
            self.persist_active_order();
            self.notify("Item note updated.".to_string());
        }
        self.focus = self.focus_return;
    }

    pub fn toggle_selected_line_complimentary(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }
        let order = self.order_mut();
        if order.cart.is_empty() || order.cart_index >= order.cart.len() {
            return;
        }
        let line = &mut order.cart[order.cart_index];
        line.is_complimentary = !line.is_complimentary;
        let is_nc = line.is_complimentary;
        let name = line.name.clone();
        if is_nc {
            line.discount_percent = 0.0;
            self.notify(format!("Marked '{name}' as Complimentary (NC)."));
        } else {
            self.notify(format!("Removed Complimentary (NC) flag from '{name}'."));
        }
        self.persist_active_order();
    }

    pub fn cycle_selected_line_discount(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }
        let order = self.order_mut();
        if order.cart.is_empty() || order.cart_index >= order.cart.len() {
            return;
        }
        let line = &mut order.cart[order.cart_index];
        if line.is_complimentary {
            line.is_complimentary = false;
        }
        let next_disc = match line.discount_percent as u32 {
            0 => 10.0,
            10 => 20.0,
            20 => 50.0,
            50 => 100.0,
            _ => 0.0,
        };
        line.discount_percent = next_disc;
        let name = line.name.clone();
        self.notify(format!("Set '{name}' discount to {:.0}%.", next_disc));
        self.persist_active_order();
    }

    pub fn remove_selected_line(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }

        let order = self.order_mut();
        if order.cart_index < order.cart.len() {
            let name = order.cart[order.cart_index].name.clone();
            order.cart.remove(order.cart_index);
            if order.cart_index >= order.cart.len() && !order.cart.is_empty() {
                order.cart_index = order.cart.len() - 1;
            }
            self.notify(format!("Removed {name}."));
            self.persist_active_order();
        }
    }

    pub fn adjust_selected_line_quantity(&mut self, change: i32) {
        if !self.ensure_editable_order() {
            return;
        }

        let message = {
            let order = self.order_mut();
            if order.cart_index >= order.cart.len() {
                String::from("No item selected in the bill.")
            } else if change > 0 {
                order.cart[order.cart_index].qty += change as u32;
                let name = order.cart[order.cart_index].name.clone();
                format!("Increased {name}.")
            } else if order.cart[order.cart_index].qty > 1 {
                order.cart[order.cart_index].qty -= (-change) as u32;
                let name = order.cart[order.cart_index].name.clone();
                format!("Decreased {name}.")
            } else {
                String::from("Cannot decrease below 1.")
            }
        };

        self.notify(message);
        self.persist_active_order();
    }

    pub fn clear_active_cart(&mut self) {
        if !self.ensure_editable_order() {
            return;
        }
        self.order_mut().cart.clear();
        let order = self.order_mut();
        order.cart_index = 0;
        self.persist_active_order();
        self.notify("Cart cleared.");
    }

    pub fn ensure_editable_order(&mut self) -> bool {
        match self.orders.get(self.active_order) {
            None => {
                self.notify("No active order. Open a table or take-out order first.");
                false
            }
            Some(order) if order.status == OrderStatus::Paid => {
                self.notify(format!("Bill #{} is paid and cannot be changed.", order.id));
                false
            }
            Some(_) => true,
        }
    }
}
