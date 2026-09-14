//! Receipt rendering logic.

use crate::models::{DailySalesSummary, Order};

pub fn money(value: f64) -> String {
    if value < 0.0 {
        format!("-₹{:.2}", value.abs())
    } else {
        format!("₹{value:.2}")
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
    if !gst_number.trim().is_empty() {
        out.push_str(&center(&format!("GST: {}", gst_number.trim()), 42));
        out.push('\n');
    }
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!("Bill #{:<6} Table: {}\n", order.id, order.label));
    out.push_str(&format!(
        "{}\n",
        chrono::Local::now().format("%d-%m-%Y %H:%M")
    ));
    out.push_str(&format!("Mode : {}\n", order.service.label()));
    if let Some(mode) = order.payment_mode {
        out.push_str(&format!("Payment: {}\n", mode.display().to_uppercase()));
    }
    let mobile = customer_mobile.or(order.customer_mobile.as_deref());
    if let Some(mobile) = mobile {
        out.push_str(&format!("Mobile: {mobile}\n"));
    }
    out.push_str(&"-".repeat(42));
    out.push('\n');

    for line in &order.cart {
        out.push_str(&format!(
            "{:<22}{:>4} × {:<10}\n",
            line.name,
            line.qty,
            money(line.unit_price * line.qty as f64)
        ));
        if let Some(note) = &line.note {
            out.push_str(&format!("  ↳ {}\n", note));
        }
    }

    let totals = order.totals();
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!(
        "{:<22}{:>20}\n",
        "Subtotal",
        money(totals.subtotal)
    ));
    if totals.discount > 0.0 {
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            "Discount",
            money(-totals.discount)
        ));
    }
    if totals.ac_charge > 0.0 {
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            "AC Surcharge",
            money(totals.ac_charge)
        ));
    }
    if totals.gst > 0.0 {
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            format!("GST ({:.1}%)", totals.gst_rate * 100.0),
            money(totals.gst)
        ));
    }
    out.push_str(&format!("{:<22}{:>20}\n", "TOTAL", money(totals.total)));
    if let Some(mode) = order.payment_mode {
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

pub fn render_kot(order: &Order, is_reprint: bool, restaurant_name: &str) -> String {
    let mut out = String::new();
    let r_name = if restaurant_name.trim().is_empty() {
        "SHREE KRISHNA RESTAURANT"
    } else {
        restaurant_name.trim()
    };
    out.push_str(&center(r_name, 42));
    out.push('\n');
    let title = if is_reprint {
        "*** KITCHEN ORDER TICKET [REPRINT] ***"
    } else {
        "*** KITCHEN ORDER TICKET (KOT) ***"
    };
    out.push_str(&center(title, 42));
    out.push('\n');
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!("Table / Order: {}\n", order.label));
    if let Some(area) = &order.area {
        out.push_str(&format!("Area         : {}\n", area));
    }
    out.push_str(&format!("Order ID     : #{}\n", order.id));
    out.push_str(&format!(
        "Time         : {}\n",
        chrono::Local::now().format("%d-%m-%Y %H:%M:%S")
    ));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!("{:<32}{:>10}\n", "ITEM", "QTY"));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    for line in &order.cart {
        out.push_str(&format!("{:<32}{:>10}\n", line.name, line.qty));
        if let Some(note) = &line.note {
            out.push_str(&format!("  ↳ {}\n", note));
        }
    }
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&center("Send to Kitchen", 42));
    out.push_str("\n\n");
    out
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
