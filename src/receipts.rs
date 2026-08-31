//! Receipt rendering logic.

use crate::models::{Order};

pub fn money(value: f64) -> String {
    if value < 0.0 {
        format!("-₹{:.2}", value.abs())
    } else {
        format!("₹{value:.2}")
    }
}

pub fn render_receipt(order: &Order, customer_mobile: Option<&str>, gst_number: &str) -> String {
    // ... [existing logic here] ...
    // Actually, I should just copy the render_receipt logic here.
    let mut out = String::new();
    out.push_str(&center("SHREE KRISHNA RESTAURANT", 42));
    out.push('\n');
    out.push_str(&center("Dine-In & Take-Out", 42));
    out.push('\n');
    if !gst_number.trim().is_empty() {
        out.push_str(&center(&format!("GST: {}", gst_number), 42));
        out.push('\n');
    }
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!("Bill #{:<6} Table: {}\n", order.id, order.label));
    out.push_str(&format!("{}\n", chrono::Local::now().format("%d-%m-%Y %H:%M")));
    out.push_str(&format!("Mode : {}\n", order.service.label()));
    if let Some(mobile) = customer_mobile {
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
    }

    let totals = order.totals();
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!("{:<22}{:>20}\n", "Subtotal", money(totals.subtotal)));
    if totals.discount > 0.0 {
        out.push_str(&format!("{:<22}{:>20}\n", "Discount", money(-totals.discount)));
    }
    if totals.ac_charge > 0.0 {
        out.push_str(&format!("{:<22}{:>20}\n", "AC Surcharge", money(totals.ac_charge)));
    }
    if totals.gst > 0.0 {
        out.push_str(&format!("{:<22}{:>20}\n", format!("GST ({:.1}%)", totals.gst_rate * 100.0), money(totals.gst)));
    }
    out.push_str(&format!("{:<22}{:>20}\n", "TOTAL", money(totals.total)));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&center("Thank you! Visit again!", 42));
    out.push_str("\n\n");
    out
}

fn center(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(text.chars().count()) / 2;
    format!("{}{}", " ".repeat(padding), text)
}
