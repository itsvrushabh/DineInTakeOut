//! Receipt rendering logic.

use crate::models::{DailySalesSummary, Order};

pub fn money(value: f64) -> String {
    if value < 0.0 {
        format!("-₹{:.2}", value.abs())
    } else {
        format!("₹{value:.2}")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReceiptItem {
    pub name: String,
    pub tag: &'static str,
    pub qty: u32,
    pub line_total: f64,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReceiptDocument {
    pub restaurant_name: String,
    pub address: String,
    pub contact: String,
    pub gst_number: String,
    pub bill_id: u32,
    pub table_label: String,
    pub service_label: &'static str,
    pub payment_mode: Option<crate::models::PaymentMode>,
    pub customer_mobile: Option<String>,
    pub items: Vec<ReceiptItem>,
    pub totals: crate::models::BillTotals,
    pub timestamp_str: String,
}

impl ReceiptDocument {
    pub fn from_order(
        order: &Order,
        customer_mobile: Option<&str>,
        gst_number: &str,
        restaurant_name: &str,
        address: &str,
        contact: &str,
    ) -> Self {
        let r_name = if restaurant_name.trim().is_empty() {
            "SHREE KRISHNA RESTAURANT".to_string()
        } else {
            restaurant_name.trim().to_string()
        };

        let items = order
            .cart
            .iter()
            .map(|line| {
                let tag = if line.is_complimentary {
                    " [NC]"
                } else if line.discount_percent > 0.0 {
                    " [DISC]"
                } else {
                    ""
                };
                ReceiptItem {
                    name: line.name.clone(),
                    tag,
                    qty: line.qty,
                    line_total: line.total(),
                    note: line.note.clone(),
                }
            })
            .collect();

        Self {
            restaurant_name: r_name,
            address: address.trim().to_string(),
            contact: contact.trim().to_string(),
            gst_number: gst_number.trim().to_string(),
            bill_id: order.id,
            table_label: order.label.clone(),
            service_label: order.service.label(),
            payment_mode: order.payment_mode,
            customer_mobile: customer_mobile
                .map(|s| s.to_string())
                .or_else(|| order.customer_mobile.clone()),
            items,
            totals: order.totals(),
            timestamp_str: chrono::Local::now().format("%d-%m-%Y %H:%M").to_string(),
        }
    }

    pub fn render_ascii(&self) -> String {
        let mut out = String::new();
        out.push_str(&center(&self.restaurant_name, 42));
        out.push('\n');
        if !self.address.is_empty() {
            out.push_str(&center(&self.address, 42));
            out.push('\n');
        }
        if !self.contact.is_empty() {
            out.push_str(&center(&format!("Contact: {}", self.contact), 42));
            out.push('\n');
        }
        if !self.gst_number.is_empty() {
            out.push_str(&center(&format!("GST: {}", self.gst_number), 42));
            out.push('\n');
        }
        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&format!(
            "Bill #{:<6} Table: {}\n",
            self.bill_id, self.table_label
        ));
        out.push_str(&format!("{}\n", self.timestamp_str));
        out.push_str(&format!("Mode : {}\n", self.service_label));
        if let Some(mode) = self.payment_mode {
            out.push_str(&format!("Payment: {}\n", mode.display().to_uppercase()));
        }
        if let Some(ref mobile) = self.customer_mobile {
            out.push_str(&format!("Mobile: {mobile}\n"));
        }
        out.push_str(&"-".repeat(42));
        out.push('\n');

        for item in &self.items {
            let display_name = format!("{}{}", item.name, item.tag);
            out.push_str(&format!(
                "{:<22}{:>4} × {:<10}\n",
                display_name,
                item.qty,
                money(item.line_total)
            ));
            if let Some(ref note) = item.note {
                out.push_str(&format!("  ↳ {}\n", note));
            }
        }

        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            "Subtotal",
            money(self.totals.subtotal)
        ));
        if self.totals.discount > 0.0 {
            out.push_str(&format!(
                "{:<22}{:>20}\n",
                "Discount",
                money(-self.totals.discount)
            ));
        }
        if self.totals.ac_charge > 0.0 {
            out.push_str(&format!(
                "{:<22}{:>20}\n",
                "AC Surcharge",
                money(self.totals.ac_charge)
            ));
        }
        if self.totals.gst > 0.0 {
            out.push_str(&format!(
                "{:<22}{:>20}\n",
                format!("GST ({:.1}%)", self.totals.gst_rate * 100.0),
                money(self.totals.gst)
            ));
        }
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            "TOTAL",
            money(self.totals.total)
        ));
        if let Some(mode) = self.payment_mode {
            out.push_str(&format!(
                "{:<22}{:>20}\n",
                "Paid via",
                mode.display().to_uppercase()
            ));
        }
        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&center("Thank you! Visit again!", 42));
        out.push_str("\n\n");
        out
    }
}

pub fn render_receipt(
    order: &Order,
    customer_mobile: Option<&str>,
    gst_number: &str,
    restaurant_name: &str,
    address: &str,
    contact: &str,
) -> String {
    ReceiptDocument::from_order(
        order,
        customer_mobile,
        gst_number,
        restaurant_name,
        address,
        contact,
    )
    .render_ascii()
}

#[derive(Clone, Debug, PartialEq)]
pub struct KotItem {
    pub name: String,
    pub qty: u32,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KotDocument {
    pub restaurant_name: String,
    pub order_id: u32,
    pub table_label: String,
    pub area_name: Option<String>,
    pub is_reprint: bool,
    pub is_delta: bool,
    pub items: Vec<KotItem>,
    pub timestamp_str: String,
}

impl KotDocument {
    pub fn from_order_items(
        order: &Order,
        items: &[crate::models::CartLine],
        is_reprint: bool,
        is_delta: bool,
        restaurant_name: &str,
    ) -> Self {
        let r_name = if restaurant_name.trim().is_empty() {
            "SHREE KRISHNA RESTAURANT".to_string()
        } else {
            restaurant_name.trim().to_string()
        };

        let kot_items = items
            .iter()
            .map(|l| KotItem {
                name: l.name.clone(),
                qty: l.qty,
                note: l.note.clone(),
            })
            .collect();

        Self {
            restaurant_name: r_name,
            order_id: order.id,
            table_label: order.label.clone(),
            area_name: order.area.clone(),
            is_reprint,
            is_delta,
            items: kot_items,
            timestamp_str: chrono::Local::now().format("%d-%m-%Y %H:%M:%S").to_string(),
        }
    }

    pub fn render_ascii(&self) -> String {
        let mut out = String::new();
        out.push_str(&center(&self.restaurant_name, 42));
        out.push('\n');
        let title = if self.is_reprint {
            "*** KITCHEN ORDER TICKET [REPRINT] ***"
        } else if self.is_delta {
            "*** KITCHEN ORDER TICKET (ADD-ON / DELTA) ***"
        } else {
            "*** KITCHEN ORDER TICKET (KOT) ***"
        };
        out.push_str(&center(title, 42));
        out.push('\n');
        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&format!("Table / Order: {}\n", self.table_label));
        if let Some(ref area) = self.area_name {
            out.push_str(&format!("Area         : {}\n", area));
        }
        out.push_str(&format!("Order ID     : #{}\n", self.order_id));
        out.push_str(&format!("Time         : {}\n", self.timestamp_str));
        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&format!("{:<32}{:>10}\n", "ITEM", "QTY"));
        out.push_str(&"-".repeat(42));
        out.push('\n');
        for line in &self.items {
            out.push_str(&format!("{:<32}{:>10}\n", line.name, line.qty));
            if let Some(ref note) = line.note {
                out.push_str(&format!("  ↳ {}\n", note));
            }
        }
        out.push_str(&"-".repeat(42));
        out.push('\n');
        out.push_str(&center("Send to Kitchen", 42));
        out.push_str("\n\n");
        out
    }
}

pub fn render_kot(order: &Order, is_reprint: bool, restaurant_name: &str) -> String {
    render_kot_items(order, &order.cart, is_reprint, false, restaurant_name)
}

pub fn render_kot_items(
    order: &Order,
    items: &[crate::models::CartLine],
    is_reprint: bool,
    is_delta: bool,
    restaurant_name: &str,
) -> String {
    KotDocument::from_order_items(order, items, is_reprint, is_delta, restaurant_name)
        .render_ascii()
}

pub fn render_z_report(
    summary: &DailySalesSummary,
    restaurant_name: &str,
    address: &str,
    contact: &str,
    gst_number: &str,
) -> String {
    let mut out = String::new();
    let r_name = if restaurant_name.trim().is_empty() {
        "SHREE KRISHNA RESTAURANT"
    } else {
        restaurant_name.trim()
    };
    out.push_str(&center(r_name, 42));
    out.push('\n');
    if !address.trim().is_empty() {
        out.push_str(&center(address.trim(), 42));
        out.push('\n');
    }
    if !contact.trim().is_empty() {
        out.push_str(&center(&format!("Contact: {}", contact.trim()), 42));
        out.push('\n');
    }
    out.push_str(&center("DAILY SALES & SETTLEMENT REPORT", 42));
    out.push('\n');
    if !gst_number.trim().is_empty() {
        out.push_str(&center(&format!("GST: {}", gst_number), 42));
        out.push('\n');
    }
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!("Report Date  : {}\n", summary.date));
    out.push_str(&format!(
        "Generated At : {}\n",
        chrono::Local::now().format("%d-%m-%Y %H:%M")
    ));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!(
        "{:<26}{:>16}\n",
        "Total Orders Billed", summary.total_orders
    ));
    out.push_str(&format!(
        "{:<26}{:>16}\n",
        "  - Dine-In Orders", summary.dine_in_orders
    ));
    out.push_str(&format!(
        "{:<26}{:>16}\n",
        "  - Take-Out Orders", summary.takeout_orders
    ));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!(
        "{:<22}{:>20}\n",
        "Gross Subtotal",
        money(summary.subtotal)
    ));
    if summary.discount > 0.0 {
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            "Discounts Given",
            money(-summary.discount)
        ));
    }
    if summary.ac_charge > 0.0 {
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            "AC Surcharges",
            money(summary.ac_charge)
        ));
    }
    out.push_str(&format!(
        "{:<22}{:>20}\n",
        "GST Collected",
        money(summary.tax)
    ));
    let cgst = summary.tax / 2.0;
    out.push_str(&format!("{:<22}{:>20}\n", "  - CGST", money(cgst)));
    out.push_str(&format!("{:<22}{:>20}\n", "  - SGST", money(cgst)));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!(
        "{:<22}{:>20}\n",
        "TOTAL NET SALES",
        money(summary.total_sales)
    ));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&center("PAYMENT MODE SETTLEMENTS", 42));
    out.push('\n');
    out.push_str(&format!("{:<16}{:>6}{:>20}\n", "MODE", "COUNT", "TOTAL"));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!(
        "{:<16}{:>6}{:>20}\n",
        "UPI",
        summary.upi_count,
        money(summary.upi_total)
    ));
    out.push_str(&format!(
        "{:<16}{:>6}{:>20}\n",
        "Cash",
        summary.cash_count,
        money(summary.cash_total)
    ));
    out.push_str(&format!(
        "{:<16}{:>6}{:>20}\n",
        "Card",
        summary.card_count,
        money(summary.card_total)
    ));
    if summary.person_credit_count > 0 {
        out.push_str(&format!(
            "{:<16}{:>6}{:>20}\n",
            "Person Credit",
            summary.person_credit_count,
            money(summary.person_credit_total)
        ));
    }
    if summary.have_it_on_hotel_count > 0 {
        out.push_str(&format!(
            "{:<16}{:>6}{:>20}\n",
            "On Hotel",
            summary.have_it_on_hotel_count,
            money(summary.have_it_on_hotel_total)
        ));
    }
    if summary.other_count > 0 {
        out.push_str(&format!(
            "{:<16}{:>6}{:>20}\n",
            "Unsettled",
            summary.other_count,
            money(summary.other_total)
        ));
    }
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&center("End of Shift Report", 42));
    out.push_str("\n\n");
    out
}

pub fn print_receipt_text(text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut child = std::process::Command::new("lp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("lp returned error status".to_string())
    }
}

pub fn generate_upi_qr_blocks(upi_uri: &str) -> Result<Vec<String>, String> {
    use qrcode::{Color, EcLevel, QrCode, Version};
    let code = QrCode::with_version(upi_uri, Version::Normal(4), EcLevel::M)
        .or_else(|_| QrCode::new(upi_uri))
        .map_err(|e| e.to_string())?;
    let width = code.width();
    let colors = code.to_colors();
    let mut lines = Vec::new();
    let quiet_zone = 1;
    let total_width = width + 2 * quiet_zone;

    // Top quiet zone
    lines.push(" ".repeat(total_width));

    for y in (0..width).step_by(2) {
        let mut line = String::new();
        line.push(' '); // Left quiet zone
        for x in 0..width {
            let top_dark = colors[y * width + x] == Color::Dark;
            let bottom_dark = if y + 1 < width {
                colors[(y + 1) * width + x] == Color::Dark
            } else {
                false
            };
            match (top_dark, bottom_dark) {
                (true, true) => line.push('█'),
                (true, false) => line.push('▀'),
                (false, true) => line.push('▄'),
                (false, false) => line.push(' '),
            }
        }
        line.push(' '); // Right quiet zone
        lines.push(line);
    }

    // Bottom quiet zone
    lines.push(" ".repeat(total_width));
    Ok(lines)
}

fn center(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(text.chars().count()) / 2;
    format!("{}{}", " ".repeat(padding), text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CartLine, DailySalesSummary, Order, OrderStatus, PaymentMode, Service};

    #[test]
    fn test_money_and_center() {
        assert_eq!(money(150.0), "₹150.00");
        assert_eq!(money(-42.5), "-₹42.50");
        assert_eq!(money(0.0), "₹0.00");

        let centered = center("HELLO", 11);
        assert_eq!(centered, "   HELLO");

        let exact = center("EXACT", 5);
        assert_eq!(exact, "EXACT");

        let overflow = center("LONGSTRING", 4);
        assert_eq!(overflow, "LONGSTRING");
    }

    #[test]
    fn test_generate_upi_qr_blocks() {
        let uri = "upi://pay?pa=test@upi&pn=Rest&am=100.00&cu=INR";
        let blocks = generate_upi_qr_blocks(uri).unwrap();
        assert!(!blocks.is_empty());
        assert!(blocks
            .iter()
            .any(|line| line.contains('█') || line.contains('▀') || line.contains('▄')));
    }

    #[test]
    fn test_render_receipt_variations() {
        let mut order = Order {
            id: 101,
            label: "Table 3".into(),
            service: Service::DineIn,
            table_number: Some(3),
            area: Some("AC Hall".into()),
            is_ac: true,
            ac_rate: 0.05,
            discount_percent: 10.0,
            cart: vec![
                {
                    let mut item = CartLine::new("Special Thali", 200.0, 2);
                    item.note = Some("No onions".into());
                    item
                },
                {
                    let mut item = CartLine::new("Papad", 15.0, 1);
                    item.is_complimentary = true;
                    item
                },
                {
                    let mut item = CartLine::new("Dessert", 80.0, 1);
                    item.discount_percent = 25.0;
                    item
                },
            ],
            cart_index: 0,
            status: OrderStatus::Paid,
            customer_mobile: Some("9876543210".into()),
            payment_mode: Some(PaymentMode::Cash),
            kot_sent_count: 3,
        };

        // Render with full header details
        let r1 = render_receipt(
            &order,
            None,
            "27TESTGSTIN",
            "KRISHNA BHOJ",
            "Market Road",
            "1234567890",
        );
        assert!(r1.contains("KRISHNA BHOJ"));
        assert!(r1.contains("Market Road"));
        assert!(r1.contains("Contact: 1234567890"));
        assert!(r1.contains("GST: 27TESTGSTIN"));
        assert!(r1.contains("Bill #101"));
        assert!(r1.contains("Mobile: 9876543210"));
        assert!(r1.contains("Special Thali"));
        assert!(r1.contains("↳ No onions"));
        assert!(r1.contains("[NC]"));
        assert!(r1.contains("[DISC]"));
        assert!(r1.contains("Discount"));
        assert!(r1.contains("AC Surcharge"));
        assert!(r1.contains("Payment: CASH"));

        // Render default restaurant name with no optional lines
        order.service = Service::TakeOut;
        order.payment_mode = None;
        order.customer_mobile = None;
        let r2 = render_receipt(&order, Some("9999988888"), "", "", "", "");
        assert!(r2.contains("SHREE KRISHNA RESTAURANT"));
        assert!(r2.contains("Mobile: 9999988888"));
        assert!(r2.contains("Mode : TAKE-OUT"));
    }

    #[test]
    fn test_render_kot_variations() {
        let order = Order {
            id: 202,
            label: "T-5".into(),
            service: Service::DineIn,
            table_number: Some(5),
            area: Some("Patio".into()),
            is_ac: false,
            ac_rate: 0.0,
            discount_percent: 0.0,
            cart: vec![{
                let mut line = CartLine::new("Masala Dosa", 90.0, 2);
                line.note = Some("Crispy".into());
                line
            }],
            cart_index: 0,
            status: OrderStatus::Ordering,
            customer_mobile: None,
            payment_mode: None,
            kot_sent_count: 0,
        };

        let kot1 = render_kot(&order, false, "SOUTH INDIAN CAFE");
        assert!(kot1.contains("SOUTH INDIAN CAFE"));
        assert!(kot1.contains("*** KITCHEN ORDER TICKET (KOT) ***"));
        assert!(kot1.contains("Area         : Patio"));
        assert!(kot1.contains("Masala Dosa"));
        assert!(kot1.contains("↳ Crispy"));

        let kot2 = render_kot(&order, true, "");
        assert!(kot2.contains("*** KITCHEN ORDER TICKET [REPRINT] ***"));
        assert!(kot2.contains("SHREE KRISHNA RESTAURANT"));

        let kot_delta = render_kot_items(&order, &order.cart, false, true, "TEST");
        assert!(kot_delta.contains("*** KITCHEN ORDER TICKET (ADD-ON / DELTA) ***"));
    }

    #[test]
    fn test_render_daily_sales_report() {
        let summary = DailySalesSummary {
            date: "2026-09-14".into(),
            total_orders: 15,
            dine_in_orders: 10,
            takeout_orders: 5,
            subtotal: 5000.0,
            discount: 200.0,
            ac_charge: 150.0,
            tax: 250.0,
            total_sales: 5200.0,
            cash_count: 5,
            cash_total: 2000.0,
            upi_count: 6,
            upi_total: 2200.0,
            card_count: 2,
            card_total: 600.0,
            split_count: 1,
            split_total: 200.0,
            person_credit_count: 1,
            person_credit_total: 200.0,
            have_it_on_hotel_count: 1,
            have_it_on_hotel_total: 200.0,
            other_count: 0,
            other_total: 0.0,
        };

        let report = render_z_report(
            &summary,
            "KRISHNA",
            "Highway Stop",
            "9876543210",
            "27GSTIN123",
        );
        assert!(report.contains("DAILY SALES & SETTLEMENT REPORT"));
        assert!(report.contains("Report Date  : 2026-09-14"));
        assert!(report.contains("Discounts Given"));
        assert!(report.contains("AC Surcharges"));
        assert!(report.contains("GST Collected"));
        assert!(report.contains("Person Credit"));
        assert!(report.contains("On Hotel"));
        assert!(report.contains("TOTAL NET SALES"));

        // With zero discounts, zero AC, empty GST
        let mut s2 = summary.clone();
        s2.discount = 0.0;
        s2.ac_charge = 0.0;
        s2.person_credit_count = 0;
        s2.have_it_on_hotel_count = 0;
        s2.other_count = 2;
        s2.other_total = 400.0;
        let r2 = render_z_report(&s2, "", "", "", "");
        assert!(r2.contains("SHREE KRISHNA RESTAURANT"));
        assert!(r2.contains("Unsettled"));
        assert!(!r2.contains("Discounts Given"));
    }
}
