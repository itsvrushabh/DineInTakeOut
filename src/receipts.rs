//! Receipt rendering, persistence, printing, and recent-receipt loading.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Local;

use crate::models::{BillSummary, Order, Service};

pub const BILLS_DIR: &str = "bills";

pub fn money(value: f64) -> String {
    format!("₹{value:.2}")
}

pub fn render_receipt(order: &Order, customer_mobile: Option<&str>, gst_number: &str) -> String {
    let mut out = String::new();
    out.push_str(&center("SHREE KRISHNA RESTAURANT", 42));
    out.push('\n');
    out.push_str(&center("Dine-In & Take-Out", 42));
    out.push('\n');
    if !gst_number.trim().is_empty() {
        out.push_str(&center(&format!("GSTIN: {gst_number}"), 42));
        out.push('\n');
    }
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&format!("Bill #{:<6} Table: {}\n", order.id, order.label));
    out.push_str(&format!("{}\n", Local::now().format("%d-%m-%Y %H:%M")));
    out.push_str(&format!("Mode : {}\n", order.service.label()));
    if let Some(mobile) = customer_mobile {
        out.push_str(&format!("Mobile: {mobile}\n"));
    }
    out.push_str(&"-".repeat(42));
    out.push('\n');

    for line in &order.cart {
        out.push_str(&line.name);
        out.push('\n');
        out.push_str(&format!(
            "  {:>3} x {:>9} {:>14}\n",
            line.qty,
            money(line.unit_price),
            money(line.total())
        ));
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
            format!("Discount ({:.0}%)", order.discount_percent),
            money(-totals.discount)
        ));
    }
    if totals.ac_charge > 0.0 {
        out.push_str(&format!(
            "{:<22}{:>20}\n",
            format!("AC charge ({:.0}%)", order.ac_surcharge() * 100.0),
            money(totals.ac_charge)
        ));
    }
    out.push_str(&format!(
        "{:<22}{:>20}\n",
        format!("GST ({:.0}%)", totals.gst_rate * 100.0),
        money(totals.gst)
    ));
    out.push_str(&format!("{:<22}{:>20}\n", "TOTAL", money(totals.total)));
    out.push_str(&"-".repeat(42));
    out.push('\n');
    out.push_str(&center("Thank you! Visit again!", 42));
    out.push_str("\n\n");
    out
}

pub fn save_and_print(text: &str, order_number: u32) -> Result<String, String> {
    let stamp = Local::now().format("%Y%m%d_%H%M%S");
    fs::create_dir_all(BILLS_DIR).map_err(|error| error.to_string())?;
    let path = PathBuf::from(BILLS_DIR).join(format!("bill_{order_number}_{stamp}.txt"));
    fs::write(&path, text).map_err(|error| error.to_string())?;

    match Command::new("lp").arg(&path).output() {
        Ok(output) if output.status.success() => Ok(format!(
            "Bill #{order_number} printed and saved to {}.",
            path.display()
        )),
        Ok(output) => Ok(format!(
            "Saved to {} (printer not available: {}).",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(_) => Ok(format!(
            "Saved to {} (no printer found on this system).",
            path.display()
        )),
    }
}

/// Reads recent receipts from both the current `bills/` folder and the legacy
/// project-root location, so older receipts remain reviewable after upgrade.
pub fn load_recent() -> Vec<BillSummary> {
    let mut paths = receipt_paths(Path::new(BILLS_DIR));
    paths.extend(receipt_paths(Path::new(".")));
    paths.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    paths.reverse();
    paths
        .into_iter()
        .filter_map(|path| parse_summary(&path))
        .take(5)
        .collect()
}

pub fn next_bill_number(recent: &[BillSummary]) -> u32 {
    recent
        .iter()
        .map(|bill| bill.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn receipt_paths(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            (name.starts_with("bill_") && name.ends_with(".txt")).then_some(path)
        })
        .collect()
}

fn parse_summary(path: &Path) -> Option<BillSummary> {
    let receipt = fs::read_to_string(path).ok()?;
    let bill_line = receipt
        .lines()
        .find(|line| line.trim_start().starts_with("Bill #"))?;
    let id = bill_line
        .trim_start()
        .strip_prefix("Bill #")?
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    let label = bill_line
        .split_once("Table:")
        .map(|(_, label)| label.trim().to_string())
        .unwrap_or_else(|| "Saved receipt".to_string());
    let service = if receipt.contains("Mode : TAKE-OUT") {
        Service::TakeOut
    } else {
        Service::DineIn
    };
    let total_line = receipt
        .lines()
        .find(|line| line.trim_start().starts_with("TOTAL"))?;
    let total = total_line.split('₹').nth(1)?.trim().parse().ok()?;
    Some(BillSummary {
        id,
        label,
        service,
        total,
        receipt,
    })
}

fn center(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(text.chars().count()) / 2;
    format!("{}{}", " ".repeat(padding), text)
}
