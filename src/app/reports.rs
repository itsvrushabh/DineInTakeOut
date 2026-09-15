use chrono::Local;

use crate::{
    app::App,
    models::{DailySalesSummary, Focus, SalesAnalytics},
    receipts::render_receipt,
};

impl App {
    pub fn open_sales_analytics(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        if let Some(db) = &self.database {
            self.sales_analytics = Some(db.get_sales_analytics(&today));
        } else {
            self.sales_analytics = Some(SalesAnalytics {
                date: today,
                ..Default::default()
            });
        }
        self.focus_return = self.focus;
        self.focus = Focus::Analytics;
    }

    pub fn open_daily_report(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let summary = if let Some(db) = &self.database {
            db.get_daily_sales_summary(&today)
        } else {
            DailySalesSummary {
                date: today,
                ..Default::default()
            }
        };
        self.daily_report_summary = Some(summary);
        self.focus_return = self.focus;
        self.focus = Focus::DailyReport;
    }

    pub fn print_daily_report(&mut self) {
        let summary = match &self.daily_report_summary {
            Some(s) => s.clone(),
            None => return,
        };
        let text = crate::receipts::render_z_report(
            &summary,
            &self.restaurant_name,
            &self.restaurant_address,
            &self.restaurant_contact,
            &self.gst_number,
        );
        if let Some(db) = &self.database {
            if let Err(e) = db.save_z_report(
                &summary.date,
                summary.subtotal + summary.ac_charge + summary.tax,
                summary.total_sales,
                summary.total_orders as u32,
                &text,
            ) {
                self.notify(format!("DB Z-Report save error: {e}"));
            }
        }
        let _ = crate::receipts::print_receipt_text(&text);
        self.notify(format!(
            "Z-Report for {} saved to DB & printed!",
            summary.date
        ));
    }

    pub fn open_bill_search(&mut self) {
        self.bill_search_query.clear();
        self.bill_search_index = 0;
        self.bill_search_results = if let Some(db) = &self.database {
            db.search_bills("")
        } else {
            Vec::new()
        };
        self.focus_return = self.focus;
        self.focus = Focus::BillSearch;
    }

    pub fn update_bill_search(&mut self) {
        self.bill_search_results = if let Some(db) = &self.database {
            db.search_bills(&self.bill_search_query)
        } else {
            Vec::new()
        };
        self.bill_search_index = 0;
    }

    pub fn reprint_selected_historical_bill(&mut self) {
        if let Some(bill) = self.bill_search_results.get(self.bill_search_index) {
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
            self.notify(format!("Could not load details for Bill #{bill_id}."));
        }
    }
}
