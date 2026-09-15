use fuzzy_matcher::FuzzyMatcher;

use crate::{
    app::{App, TableDestination, TableSearchResult},
    models::{Focus, Service, TableStatus},
};

impl App {
    pub fn matching_tables(&self) -> Vec<TableSearchResult> {
        let mut all = Vec::new();
        for (a_idx, area) in self.areas.iter().enumerate() {
            for num in 1..=area.table_count {
                let pt = self
                    .physical_tables
                    .iter()
                    .find(|t| t.area == area.name && t.number == num);
                let status = pt.map_or(TableStatus::Ready, |t| t.status);
                let dirty_since = pt.and_then(|t| t.dirty_since);
                let order = self.orders.iter().find(|o| {
                    o.service == Service::DineIn
                        && o.area.as_deref() == Some(&area.name)
                        && o.table_number == Some(num)
                });
                all.push(TableSearchResult {
                    area_index: a_idx,
                    area_name: area.name.clone(),
                    table_number: num,
                    is_ac: area.is_ac,
                    status,
                    dirty_since,
                    order_id: order.map(|o| o.id),
                    order_label: order.map(|o| o.label.clone()),
                    order_total: order.map(|o| o.totals().total),
                });
            }
        }

        let query = self.table_input.trim().to_lowercase();
        if query.is_empty() {
            let current_area = self.selected_area_index;
            all.sort_by_key(|t| (t.area_index != current_area, t.area_index, t.table_number));
            return all;
        }

        let parsed_num: Option<usize> = if let Some(stripped) = query.strip_prefix('t') {
            stripped.trim().parse().ok()
        } else {
            query.parse().ok()
        };

        let mut scored: Vec<(i64, TableSearchResult)> = Vec::new();
        for item in all {
            let mut score = 0i64;
            let area_lower = item.area_name.to_lowercase();
            let status_title = item.status.title().to_lowercase();
            let status_label = item.status.label().to_lowercase();

            if let Some(num) = parsed_num {
                if item.table_number == num {
                    score += 100;
                    if item.area_index == self.selected_area_index {
                        score += 50;
                    }
                }
            }

            let tokens: Vec<&str> = query.split_whitespace().collect();
            if tokens.len() >= 2 {
                let mut matches_tokens = true;
                for tok in &tokens {
                    if let Ok(num) = tok.parse::<usize>() {
                        if item.table_number != num {
                            matches_tokens = false;
                        }
                    } else if !area_lower.contains(tok) && !status_title.contains(tok) {
                        matches_tokens = false;
                    }
                }
                if matches_tokens {
                    score += 80;
                }
            }

            if query.len() >= 2 {
                let (first, rest) = query.split_at(1);
                if let Ok(num) = rest.parse::<usize>() {
                    if item.table_number == num && area_lower.starts_with(first) {
                        score += 90;
                    }
                }
                if query.len() >= 3 {
                    let (prefix, rest) = query.split_at(2);
                    if let Ok(num) = rest.parse::<usize>() {
                        if item.table_number == num && area_lower.starts_with(prefix) {
                            score += 95;
                        }
                    }
                }
            }

            if area_lower.contains(&query) {
                score += 40;
            }

            if status_title.contains(&query) || status_label.contains(&query) {
                score += 35;
            }

            if let Some(ref label) = item.order_label {
                if label.to_lowercase().contains(&query) {
                    score += 60;
                }
            }

            let haystack = format!(
                "{} T{} {} {}",
                item.area_name,
                item.table_number,
                item.status.title(),
                item.status.label()
            );
            if let Some(fuzzy_score) = self.matcher.fuzzy_match(&haystack, &query) {
                score += fuzzy_score;
            }

            if score > 0 {
                scored.push((score, item));
            }
        }

        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.area_index.cmp(&b.1.area_index))
                .then_with(|| a.1.table_number.cmp(&b.1.table_number))
        });

        scored.into_iter().map(|(_, item)| item).collect()
    }

    pub fn table_move_targets(&self) -> Vec<TableDestination> {
        let current_area = self.selected_area_name();
        let current_num = self.selected_table_index + 1;
        let mut list = Vec::new();
        for area in &self.areas {
            for num in 1..=area.table_count {
                if area.name == current_area && num == current_num {
                    continue;
                }
                let pt = self
                    .physical_tables
                    .iter()
                    .find(|t| t.area == area.name && t.number == num);
                let status = pt.map_or(TableStatus::Ready, |t| t.status);
                let order = self.orders.iter().find(|o| {
                    o.service == Service::DineIn
                        && o.area.as_deref() == Some(&area.name)
                        && o.table_number == Some(num)
                });
                list.push(TableDestination {
                    area_name: area.name.clone(),
                    table_number: num,
                    is_ac: area.is_ac,
                    status,
                    order_id: order.map(|o| o.id),
                    order_label: order.map(|o| o.label.clone()),
                });
            }
        }
        list
    }

    pub fn open_table_move(&mut self) {
        let source_order = self.selected_table_order();
        if source_order.is_none() {
            self.notify("Selected table does not have an active order to move.".to_string());
            return;
        }
        self.table_move_target_index = 0;
        self.focus_return = self.focus;
        self.focus = Focus::TableMove;
    }

    pub fn execute_table_move_or_merge(&mut self) {
        let targets = self.table_move_targets();
        if targets.is_empty() {
            self.focus = Focus::Tables;
            return;
        }
        let target_idx = self.table_move_target_index.min(targets.len() - 1);
        let target = targets[target_idx].clone();

        let source_area = self.selected_area_name();
        let source_number = self.selected_table_index + 1;
        let Some(source_order_pos) = self.orders.iter().position(|o| {
            o.service == Service::DineIn
                && o.area.as_deref() == Some(&source_area)
                && o.table_number == Some(source_number)
        }) else {
            self.focus = Focus::Tables;
            return;
        };

        if let Some(target_order_id) = target.order_id {
            // MERGE into target order
            let source_order = self.orders.remove(source_order_pos);
            if let Some(target_order) = self.orders.iter_mut().find(|o| o.id == target_order_id) {
                for src_line in source_order.cart {
                    if let Some(existing) = target_order
                        .cart
                        .iter_mut()
                        .find(|l| l.name == src_line.name && l.note == src_line.note)
                    {
                        existing.qty += src_line.qty;
                    } else {
                        target_order.cart.push(src_line);
                    }
                }
            }
            if let Some(db) = &self.database {
                let _ = db.delete_open_order(source_order.id);
            }
            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == source_area && t.number == source_number)
            {
                pt.status = TableStatus::Ready;
                pt.order_id = None;
                self.persist_table(&source_area, source_number);
            }
            if let Some(target_order) = self.orders.iter().find(|o| o.id == target_order_id) {
                if let Some(db) = &self.database {
                    let _ = db.save_open_order(target_order);
                }
            }
            if let Some(idx) = self.orders.iter().position(|o| o.id == target_order_id) {
                self.active_order = idx;
            } else if self.active_order >= self.orders.len() && !self.orders.is_empty() {
                self.active_order = self.orders.len() - 1;
            }
            self.notify(format!(
                "Merged Table {} into {} Table {}!",
                source_number, target.area_name, target.table_number
            ));
        } else {
            // TRANSFER to empty table
            let source_order = &mut self.orders[source_order_pos];
            let order_id = source_order.id;
            source_order.area = Some(target.area_name.clone());
            source_order.table_number = Some(target.table_number);
            source_order.label = format!(
                "{}-T{}",
                target.area_name.split_whitespace().next().unwrap_or("T"),
                target.table_number
            );
            source_order.is_ac = target.is_ac;
            if target.is_ac {
                source_order.ac_rate = self.ac_rate;
            } else {
                source_order.ac_rate = 0.0;
            }

            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == source_area && t.number == source_number)
            {
                pt.status = TableStatus::Ready;
                pt.order_id = None;
                self.persist_table(&source_area, source_number);
            }
            if let Some(pt) = self
                .physical_tables
                .iter_mut()
                .find(|t| t.area == target.area_name && t.number == target.table_number)
            {
                pt.status = TableStatus::Ordering;
                pt.order_id = Some(order_id);
                self.persist_table(&target.area_name, target.table_number);
            }
            self.persist_active_order();
            self.notify(format!(
                "Moved Table {} to {} Table {}!",
                source_number, target.area_name, target.table_number
            ));
        }
        self.focus = Focus::Tables;
    }
}
