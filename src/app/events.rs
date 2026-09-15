use crossterm::event::KeyCode;

use crate::{
    app::App,
    models::{Focus, PaymentMode, RecentTab},
};

impl App {
    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        let in_protected = matches!(
            self.focus,
            Focus::Search
                | Focus::MobileEntry
                | Focus::PaymentMode
                | Focus::OfferSelect
                | Focus::TableJump
                | Focus::DailyReport
                | Focus::BillSearch
                | Focus::UpiQr
                | Focus::TableMove
                | Focus::ItemNote
                | Focus::KitchenDisplay
                | Focus::SplitPayment
                | Focus::Analytics
        );
        if matches!(key, KeyCode::Char('q')) && !in_protected {
            return true;
        }
        if matches!(key, KeyCode::Esc) && !in_protected {
            return true;
        }

        let in_text_input = matches!(
            self.focus,
            Focus::Search
                | Focus::TableJump
                | Focus::MobileEntry
                | Focus::BillSearch
                | Focus::ItemNote
                | Focus::SplitPayment
        );

        if matches!(key, KeyCode::Char('?')) && !in_text_input {
            self.show_help = !self.show_help;
            return false;
        }

        if !in_text_input {
            if matches!(key, KeyCode::Char(']')) {
                if !self.orders.is_empty() {
                    self.active_order = (self.active_order + 1) % self.orders.len();
                    self.notify(format!("Switched to {}.", self.order().label));
                }
                return false;
            }
            if matches!(key, KeyCode::Char('[')) {
                if !self.orders.is_empty() {
                    self.active_order =
                        self.active_order.saturating_add(self.orders.len() - 1) % self.orders.len();
                    self.notify(format!("Switched to {}.", self.order().label));
                }
                return false;
            }
            if matches!(key, KeyCode::Char('z')) && self.focus != Focus::DailyReport {
                self.open_daily_report();
                return false;
            }

            let in_modal = matches!(
                self.focus,
                Focus::PaymentMode
                    | Focus::OfferSelect
                    | Focus::DailyReport
                    | Focus::TableMove
                    | Focus::UpiQr
                    | Focus::KitchenDisplay
                    | Focus::SplitPayment
                    | Focus::Analytics
            );

            if (matches!(key, KeyCode::Char('a') | KeyCode::Char('A'))
                || matches!(key, KeyCode::F(8)))
                && !in_modal
            {
                self.open_sales_analytics();
                return false;
            }

            if matches!(key, KeyCode::F(7)) && !in_modal {
                self.open_kds();
                return false;
            }

            if !in_modal {
                if let KeyCode::Char(d @ '1'..='7') = key {
                    self.switch_to_box(d as u8 - b'0');
                    return false;
                }
            }
        }

        match self.focus {
            Focus::Search => match key {
                KeyCode::Char(c) => {
                    self.search.push(c);
                    self.menu_index = 0;
                }
                KeyCode::Backspace => {
                    self.search.pop();
                    self.menu_index = 0;
                }
                KeyCode::Esc | KeyCode::Down => self.focus = Focus::Menu,
                KeyCode::Enter => self.add_selected_to_cart(),
                KeyCode::Tab => self.focus = Focus::Menu,
                KeyCode::BackTab => self.focus = Focus::Tables,
                _ => {}
            },
            Focus::Menu => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.menu_index = self.menu_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let n = self.visible_items().len();
                    if self.menu_index + 1 < n {
                        self.menu_index += 1;
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if !self.categories.is_empty() {
                        let n = self.categories.len();
                        self.selected_category_index = (self.selected_category_index + n - 1) % n;
                        self.menu_index = 0;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if !self.categories.is_empty() {
                        let n = self.categories.len();
                        self.selected_category_index = (self.selected_category_index + 1) % n;
                        self.menu_index = 0;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => self.add_selected_to_cart(),
                KeyCode::Tab => self.focus = Focus::Cart,
                KeyCode::BackTab => self.focus = Focus::Search,
                KeyCode::Char('/') => {
                    self.focus = Focus::Search;
                }
                KeyCode::Char('o') => self.toggle_selected_menu_item_stock(),
                KeyCode::Char('c') => self.clear_active_cart(),
                KeyCode::Char('p') => self.begin_billing(),
                KeyCode::Char('e') => self.export_config(),
                KeyCode::Char('i') => self.import_config(),
                KeyCode::Char('K') => self.generate_kot(),
                KeyCode::Char('g') => {
                    self.focus = Focus::TableJump;
                    self.table_input.clear();
                    self.table_search_index = 0;
                }
                _ => {}
            },
            Focus::Cart => match key {
                KeyCode::Up | KeyCode::Char('w') => {
                    if !self.orders.is_empty() {
                        let o = self.order_mut();
                        o.cart_index = o.cart_index.saturating_sub(1);
                    }
                }
                KeyCode::Down | KeyCode::Char('s') => {
                    if !self.orders.is_empty() {
                        let o = self.order_mut();
                        if o.cart_index + 1 < o.cart.len() {
                            o.cart_index += 1;
                        }
                    }
                }
                KeyCode::Char('k') => self.generate_kot(),
                KeyCode::Char('K') => self.reprint_full_kot(),
                KeyCode::Char('c') => self.toggle_selected_line_complimentary(),
                KeyCode::Char('d') => self.cycle_selected_line_discount(),
                KeyCode::Char('C') => self.clear_active_cart(),
                KeyCode::Char('n') => self.open_item_note_prompt(),
                KeyCode::Char('=') | KeyCode::Char('+') => self.adjust_selected_line_quantity(1),
                KeyCode::Char('-') => self.adjust_selected_line_quantity(-1),
                KeyCode::Delete | KeyCode::Char('x') => self.remove_selected_line(),
                KeyCode::Char('g') => {
                    self.focus = Focus::TableJump;
                    self.table_input.clear();
                    self.table_search_index = 0;
                }
                KeyCode::Enter | KeyCode::Char('p') => self.begin_billing(),
                KeyCode::Tab => {
                    self.focus = Focus::RecentBills;
                    self.recent_tab = RecentTab::Bills;
                }
                KeyCode::BackTab => self.focus = Focus::Menu,
                _ => {}
            },
            Focus::Tables => match key {
                KeyCode::Left | KeyCode::Char('h') => {
                    self.selected_table_index = self.selected_table_index.saturating_sub(1);
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    let max = self.selected_area().map_or(0, |a| a.table_count);
                    if self.selected_table_index + 1 < max {
                        self.selected_table_index += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !self.areas.is_empty() {
                        let n = self.areas.len();
                        self.selected_area_index = (self.selected_area_index + n - 1) % n;
                        let max = self.selected_area().map_or(0, |a| a.table_count);
                        if self.selected_table_index >= max {
                            self.selected_table_index = max.saturating_sub(1);
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.areas.is_empty() {
                        let n = self.areas.len();
                        self.selected_area_index = (self.selected_area_index + 1) % n;
                        let max = self.selected_area().map_or(0, |a| a.table_count);
                        if self.selected_table_index >= max {
                            self.selected_table_index = max.saturating_sub(1);
                        }
                    }
                }
                KeyCode::Enter => {
                    self.open_table_order();
                }
                KeyCode::Char('m') => {
                    self.open_table_move();
                }
                KeyCode::Char('t') => {
                    self.open_takeout_order();
                }
                KeyCode::Char('c') => {
                    self.close_order();
                }
                KeyCode::Char('s') => {
                    self.advance_stage();
                }
                KeyCode::Char('r') => {
                    self.clean_selected_table();
                }
                KeyCode::Char('K') => {
                    self.generate_kot();
                }
                KeyCode::Char('g') => {
                    self.focus = Focus::TableJump;
                    self.table_input.clear();
                    self.table_search_index = 0;
                }
                KeyCode::Char('b') | KeyCode::Char('p') => self.begin_billing(),
                KeyCode::BackTab => {
                    self.focus = Focus::RecentBills;
                    self.recent_tab = RecentTab::Kots;
                }
                KeyCode::Tab => self.focus = Focus::Search,
                _ => {}
            },
            Focus::RecentBills => match key {
                KeyCode::Left | KeyCode::Char('h') => {
                    self.recent_tab = RecentTab::Bills;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.recent_tab = RecentTab::Kots;
                }
                KeyCode::Up | KeyCode::Char('k') => match self.recent_tab {
                    RecentTab::Bills => {
                        self.recent_bill_index = self.recent_bill_index.saturating_sub(1);
                    }
                    RecentTab::Kots => {
                        self.recent_kot_index = self.recent_kot_index.saturating_sub(1);
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => match self.recent_tab {
                    RecentTab::Bills => {
                        if self.recent_bill_index + 1 < self.recent_bills.len() {
                            self.recent_bill_index += 1;
                        }
                    }
                    RecentTab::Kots => {
                        if self.recent_kot_index + 1 < self.recent_kots.len() {
                            self.recent_kot_index += 1;
                        }
                    }
                },
                KeyCode::Char('/') | KeyCode::Char('s') => {
                    self.open_bill_search();
                }
                KeyCode::Char('p') | KeyCode::Char('r') | KeyCode::Enter => match self.recent_tab {
                    RecentTab::Bills => {
                        self.reprint_selected_recent_bill();
                    }
                    RecentTab::Kots => {
                        self.reprint_selected_recent_kot();
                    }
                },
                KeyCode::Char('K') => {
                    self.open_kds();
                }
                KeyCode::Tab => match self.recent_tab {
                    RecentTab::Bills => self.recent_tab = RecentTab::Kots,
                    RecentTab::Kots => self.focus = Focus::Tables,
                },
                KeyCode::BackTab => match self.recent_tab {
                    RecentTab::Bills => self.focus = Focus::Cart,
                    RecentTab::Kots => self.recent_tab = RecentTab::Bills,
                },
                _ => {}
            },
            Focus::MobileEntry => match key {
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    if self.mobile_buffer.len() < 10 {
                        self.mobile_buffer.push(c);
                        self.update_customer_crm();
                    }
                }
                KeyCode::Backspace => {
                    self.mobile_buffer.pop();
                    self.update_customer_crm();
                }
                KeyCode::Enter => {
                    if self.mobile_buffer.len() == 10 || self.mobile_buffer.is_empty() {
                        let mobile = std::mem::take(&mut self.mobile_buffer);
                        if self.offers.is_empty() {
                            self.complete_billing(&mobile, None);
                        } else {
                            self.offer_index = 0;
                            self.focus = Focus::OfferSelect;
                            self.pending_mobile = mobile;
                        }
                    } else {
                        self.notify(format!(
                            "Mobile number needs 10 digits ({} so far, or press Enter on empty to skip).",
                            self.mobile_buffer.len()
                        ));
                    }
                }
                KeyCode::Esc => {
                    self.focus = self.focus_return;
                }
                _ => {}
            },
            Focus::PaymentMode => match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.payment_mode_index = self.payment_mode_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let last = PaymentMode::all().len() - 1;
                    if self.payment_mode_index < last {
                        self.payment_mode_index += 1;
                    }
                }
                KeyCode::Char(d @ '1'..='6') => {
                    let idx = (d as u8 - b'1') as usize;
                    if let Some(mode) = PaymentMode::all().get(idx) {
                        if *mode == PaymentMode::Split {
                            self.open_split_payment();
                        } else {
                            self.select_payment_mode(*mode);
                        }
                    }
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.select_payment_mode(PaymentMode::Cash);
                }
                KeyCode::Char('u') | KeyCode::Char('U') => {
                    self.select_payment_mode(PaymentMode::Upi);
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.select_payment_mode(PaymentMode::Card);
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.open_split_payment();
                }
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.show_upi_qr();
                }
                KeyCode::Enter => {
                    if let Some(mode) = PaymentMode::all().get(self.payment_mode_index) {
                        if *mode == PaymentMode::Split {
                            self.open_split_payment();
                        } else {
                            self.select_payment_mode(*mode);
                        }
                    }
                }
                KeyCode::Esc => {
                    self.focus = self.focus_return;
                }
                _ => {}
            },
            Focus::OfferSelect => {
                let count = self.offers.len() + 1;
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.offer_index = self.offer_index.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.offer_index + 1 < count {
                            self.offer_index += 1;
                        }
                    }
                    KeyCode::Char(d @ '0'..='9') => {
                        let idx = (d as u8 - b'0') as usize;
                        if idx < count {
                            self.apply_offer_and_bill(idx);
                        }
                    }

                    KeyCode::Enter => self.apply_offer_and_bill(self.offer_index),
                    KeyCode::Esc => {
                        self.pending_mobile.clear();
                        self.focus = self.focus_return;
                    }
                    _ => {}
                }
            }
            Focus::TableJump => match key {
                KeyCode::Char(c) => {
                    self.table_input.push(c);
                    self.table_search_index = 0;
                }
                KeyCode::Backspace => {
                    self.table_input.pop();
                    self.table_search_index = 0;
                }
                KeyCode::Up | KeyCode::BackTab => {
                    self.table_search_index = self.table_search_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Tab => {
                    let n = self.matching_tables().len();
                    if self.table_search_index + 1 < n {
                        self.table_search_index += 1;
                    }
                }
                KeyCode::Enter => {
                    let matches = self.matching_tables();
                    if !matches.is_empty() {
                        let idx = self.table_search_index.min(matches.len() - 1);
                        let target = &matches[idx];
                        self.selected_area_index = target.area_index;
                        self.selected_table_index = target.table_number.saturating_sub(1);
                        if let Some(order_id) = target.order_id {
                            if let Some(order_idx) =
                                self.orders.iter().position(|o| o.id == order_id)
                            {
                                self.active_order = order_idx;
                            }
                        }
                        self.notify(format!(
                            "Jumped to {} Table {} ({}).",
                            target.area_name,
                            target.table_number,
                            target.status.title()
                        ));
                        self.table_input.clear();
                        self.table_search_index = 0;
                        self.focus = Focus::Tables;
                    } else if !self.table_input.is_empty() {
                        self.notify(format!("No tables match '{}'.", self.table_input));
                    } else {
                        self.focus = Focus::Tables;
                    }
                }
                KeyCode::Esc => {
                    self.table_input.clear();
                    self.table_search_index = 0;
                    self.focus = Focus::Tables;
                }
                _ => {}
            },
            Focus::DailyReport => match key {
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.print_daily_report();
                }
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('z') => {
                    self.focus = self.focus_return;
                }
                _ => {}
            },
            Focus::BillSearch => match key {
                KeyCode::Char(c) => {
                    self.bill_search_query.push(c);
                    self.update_bill_search();
                }
                KeyCode::Backspace => {
                    self.bill_search_query.pop();
                    self.update_bill_search();
                }
                KeyCode::Up | KeyCode::BackTab => {
                    self.bill_search_index = self.bill_search_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Tab => {
                    if self.bill_search_index + 1 < self.bill_search_results.len() {
                        self.bill_search_index += 1;
                    }
                }
                KeyCode::Enter => {
                    self.reprint_selected_historical_bill();
                }
                KeyCode::Esc => {
                    self.focus = self.focus_return;
                }
                _ => {}
            },
            Focus::UpiQr => match key {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.focus = self.focus_return;
                }
                _ => {}
            },
            Focus::TableMove => match key {
                KeyCode::Up | KeyCode::Left | KeyCode::Char('k') | KeyCode::Char('h') => {
                    self.table_move_target_index = self.table_move_target_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Right | KeyCode::Char('j') | KeyCode::Char('l') => {
                    let n = self.table_move_targets().len();
                    if self.table_move_target_index + 1 < n {
                        self.table_move_target_index += 1;
                    }
                }
                KeyCode::Enter => {
                    self.execute_table_move_or_merge();
                }
                KeyCode::Esc => {
                    self.focus = Focus::Tables;
                }
                _ => {}
            },
            Focus::ItemNote => match key {
                KeyCode::Char(c) => {
                    self.item_note_buffer.push(c);
                }
                KeyCode::Backspace => {
                    self.item_note_buffer.pop();
                }
                KeyCode::Enter => {
                    self.save_item_note();
                }
                KeyCode::Esc => {
                    self.focus = self.focus_return;
                }
                _ => {}
            },
            Focus::KitchenDisplay => match key {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('k') | KeyCode::Char('K') => {
                    self.focus = self.focus_return;
                }
                KeyCode::Up | KeyCode::Char('w') => {
                    self.kds_index = self.kds_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('s') => {
                    if !self.kds_kots.is_empty() && self.kds_index + 1 < self.kds_kots.len() {
                        self.kds_index += 1;
                    }
                }
                KeyCode::Char(' ') | KeyCode::Enter => {
                    self.kds_bump_status();
                }
                KeyCode::Char('r') | KeyCode::F(5) => {
                    if let Some(db) = &self.database {
                        self.kds_kots = db.load_active_kots();
                        self.notify("KDS refreshed from database.".to_string());
                    }
                }
                _ => {}
            },
            Focus::SplitPayment => match key {
                KeyCode::Esc => {
                    self.focus = Focus::PaymentMode;
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.split_field = (self.split_field + 1) % 3;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.split_field = (self.split_field + 2) % 3;
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    let (bill_total, cash, upi, card) = self.split_payment_totals();
                    let current_val = match self.split_field {
                        0 => cash,
                        1 => upi,
                        2 => card,
                        _ => 0.0,
                    };
                    let other_paid = (cash + upi + card) - current_val;
                    let remaining = (bill_total - other_paid).max(0.0);
                    let rem_str = format!("{remaining:.2}");
                    match self.split_field {
                        0 => self.split_cash = rem_str,
                        1 => self.split_upi = rem_str,
                        2 => self.split_card = rem_str,
                        _ => {}
                    }
                }
                KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => match self.split_field {
                    0 => self.split_cash.push(c),
                    1 => self.split_upi.push(c),
                    2 => self.split_card.push(c),
                    _ => {}
                },
                KeyCode::Backspace => match self.split_field {
                    0 => {
                        self.split_cash.pop();
                    }
                    1 => {
                        self.split_upi.pop();
                    }
                    2 => {
                        self.split_card.pop();
                    }
                    _ => {}
                },
                KeyCode::Enter => {
                    self.confirm_split_payment();
                }
                _ => {}
            },
            Focus::Analytics => match key {
                KeyCode::Esc
                | KeyCode::Char('q')
                | KeyCode::Char('a')
                | KeyCode::Char('A')
                | KeyCode::Enter => {
                    self.focus = self.focus_return;
                }
                _ => {}
            },
        }
        false
    }
}
