//! ESC/POS thermal printer driver, formatting engine, and peripheral routing.

use std::{
    collections::HashMap,
    io::Write,
    net::{SocketAddr, TcpStream},
    path::Path,
    time::Duration,
};

use crate::models::{CartLine, Order};

/// Configured output destination for thermal printing.
#[derive(Clone, Debug, PartialEq)]
pub enum PrinterMode {
    /// Use system CUPS spooler (`lp` command).
    Lpr,
    /// Direct byte write to character device or serial port (e.g. `/dev/usb/lp0`).
    Device(String),
    /// Direct TCP socket to network thermal printer (e.g. `192.168.1.100:9100`).
    Network(String),
    /// Simulation mode that writes raw ESC/POS and text files to `receipts/`.
    Simulated,
}

#[derive(Clone, Debug)]
pub struct PrinterConfig {
    pub mode: PrinterMode,
    pub bill_printer: String,
    pub kot_printer: String,
    pub cash_drawer_enabled: bool,
}

impl Default for PrinterConfig {
    fn default() -> Self {
        Self {
            mode: PrinterMode::Lpr,
            bill_printer: String::new(),
            kot_printer: String::new(),
            cash_drawer_enabled: true,
        }
    }
}

impl PrinterConfig {
    /// Sourced from key-value pairs in `config.csv` or DB settings.
    pub fn from_map(map: &HashMap<String, String>) -> Self {
        let mode_str = map
            .get("PrinterMode")
            .map(|s| s.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "lpr".to_string());

        let bill_printer = map.get("BillPrinter").cloned().unwrap_or_default();

        let kot_printer = map.get("KotPrinter").cloned().unwrap_or_default();

        let cash_drawer_enabled = map
            .get("CashDrawerEnabled")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(true);

        let mode = match mode_str.as_str() {
            "device" | "raw_device" | "usb" => {
                let dev = if bill_printer.is_empty() {
                    "/dev/usb/lp0".to_string()
                } else {
                    bill_printer.clone()
                };
                PrinterMode::Device(dev)
            }
            "network" | "tcp" | "socket" => {
                let addr = if bill_printer.is_empty() {
                    "127.0.0.1:9100".to_string()
                } else {
                    bill_printer.clone()
                };
                PrinterMode::Network(addr)
            }
            "simulated" | "file" | "test" => PrinterMode::Simulated,
            _ => PrinterMode::Lpr,
        };

        Self {
            mode,
            bill_printer,
            kot_printer,
            cash_drawer_enabled,
        }
    }
}

// -- ESC/POS Command Byte Constants -------------------------------------------

pub const ESC_INIT: &[u8] = &[0x1B, 0x40]; // Initialize printer
pub const ESC_ALIGN_LEFT: &[u8] = &[0x1B, 0x61, 0x00];
pub const ESC_ALIGN_CENTER: &[u8] = &[0x1B, 0x61, 0x01];
pub const ESC_ALIGN_RIGHT: &[u8] = &[0x1B, 0x61, 0x02];
pub const ESC_BOLD_ON: &[u8] = &[0x1B, 0x45, 0x01];
pub const ESC_BOLD_OFF: &[u8] = &[0x1B, 0x45, 0x00];
pub const ESC_DOUBLE_SIZE: &[u8] = &[0x1D, 0x21, 0x11]; // Double height & width
pub const ESC_NORMAL_SIZE: &[u8] = &[0x1D, 0x21, 0x00];
pub const ESC_CUT_PAPER: &[u8] = &[0x1D, 0x56, 0x42, 0x00]; // GS V 66 0
pub const ESC_DRAWER_KICK: &[u8] = &[0x1B, 0x70, 0x00, 0x19, 0xFA]; // Pulse pin 2 for 50ms

/// Builds raw ESC/POS byte sequence for customer tax invoice.
pub fn build_escpos_receipt(
    order: &Order,
    restaurant_name: &str,
    address: &str,
    contact: &str,
    gst_number: &str,
    customer_mobile: Option<&str>,
    kick_drawer: bool,
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(ESC_INIT);

    // Kick cash drawer if applicable
    if kick_drawer {
        buf.extend_from_slice(ESC_DRAWER_KICK);
    }

    // Header (Centered, Double Size)
    buf.extend_from_slice(ESC_ALIGN_CENTER);
    buf.extend_from_slice(ESC_DOUBLE_SIZE);
    buf.extend_from_slice(ESC_BOLD_ON);
    let r_name = if restaurant_name.trim().is_empty() {
        "SHREE KRISHNA RESTAURANT"
    } else {
        restaurant_name.trim()
    };
    buf.extend_from_slice(r_name.as_bytes());
    buf.push(b'\n');

    // Sub-header (Normal size)
    buf.extend_from_slice(ESC_NORMAL_SIZE);
    buf.extend_from_slice(ESC_BOLD_OFF);
    if !address.trim().is_empty() {
        buf.extend_from_slice(address.trim().as_bytes());
        buf.push(b'\n');
    }
    if !contact.trim().is_empty() {
        buf.extend_from_slice(format!("Contact: {}\n", contact.trim()).as_bytes());
    }
    if !gst_number.trim().is_empty() {
        buf.extend_from_slice(format!("GSTIN: {}\n", gst_number.trim()).as_bytes());
    }

    // Invoice Metadata (Left Aligned)
    buf.extend_from_slice(ESC_ALIGN_LEFT);
    buf.extend_from_slice(b"------------------------------------------\n");
    buf.extend_from_slice(
        format!(
            "Bill #{:<6} Table: {:<14} {}\n",
            order.id,
            order.label,
            order.service.label()
        )
        .as_bytes(),
    );
    buf.extend_from_slice(
        format!(
            "Date: {}\n",
            chrono::Local::now().format("%d-%m-%Y %H:%M:%S")
        )
        .as_bytes(),
    );
    if let Some(mobile) = customer_mobile.or(order.customer_mobile.as_deref()) {
        buf.extend_from_slice(format!("Customer: {}\n", mobile).as_bytes());
    }
    buf.extend_from_slice(b"------------------------------------------\n");

    // Item List
    buf.extend_from_slice(b"ITEM                       QTY       TOTAL\n");
    buf.extend_from_slice(b"------------------------------------------\n");
    for line in &order.cart {
        let tag = if line.is_complimentary {
            " [NC]"
        } else if line.discount_percent > 0.0 {
            " [DISC]"
        } else {
            ""
        };
        let item_title = format!("{}{}", line.name, tag);
        let total_str = format!("Rs.{:.2}", line.total());
        buf.extend_from_slice(
            format!("{:<26}{:>4}{:>12}\n", item_title, line.qty, total_str).as_bytes(),
        );
        if let Some(note) = &line.note {
            buf.extend_from_slice(format!("  >> {}\n", note).as_bytes());
        }
    }
    buf.extend_from_slice(b"------------------------------------------\n");

    // Totals Breakdown
    let totals = order.totals();
    buf.extend_from_slice(format!("{:<26}{:>16.2}\n", "Subtotal", totals.subtotal).as_bytes());
    if totals.discount > 0.0 {
        buf.extend_from_slice(format!("{:<26}{:>16.2}\n", "Discount", -totals.discount).as_bytes());
    }
    if totals.ac_charge > 0.0 {
        buf.extend_from_slice(
            format!("{:<26}{:>16.2}\n", "AC Charge", totals.ac_charge).as_bytes(),
        );
    }
    if totals.gst > 0.0 {
        buf.extend_from_slice(
            format!(
                "{:<26}{:>16.2}\n",
                format!("GST ({:.1}%)", totals.gst_rate * 100.0),
                totals.gst
            )
            .as_bytes(),
        );
    }

    // Grand Total (Bold)
    buf.extend_from_slice(ESC_BOLD_ON);
    buf.extend_from_slice(format!("{:<26}{:>16.2}\n", "GRAND TOTAL", totals.total).as_bytes());
    if let Some(mode) = order.payment_mode {
        buf.extend_from_slice(format!("{:<26}{:>16}\n", "Settled Via", mode.display()).as_bytes());
    }
    buf.extend_from_slice(ESC_BOLD_OFF);
    buf.extend_from_slice(b"------------------------------------------\n");

    // Footer & Auto Cut
    buf.extend_from_slice(ESC_ALIGN_CENTER);
    buf.extend_from_slice(b"Thank you! Visit again!\n\n\n");
    buf.extend_from_slice(ESC_CUT_PAPER);

    buf
}

/// Builds raw ESC/POS byte sequence for Kitchen Order Ticket (KOT), supports delta items.
pub fn build_escpos_kot(
    order: &Order,
    items_to_print: &[CartLine],
    is_reprint: bool,
    is_delta: bool,
    restaurant_name: &str,
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(ESC_INIT);

    // KOT Header
    buf.extend_from_slice(ESC_ALIGN_CENTER);
    buf.extend_from_slice(ESC_BOLD_ON);
    let r_name = if restaurant_name.trim().is_empty() {
        "SHREE KRISHNA RESTAURANT"
    } else {
        restaurant_name.trim()
    };
    buf.extend_from_slice(format!("{}\n", r_name).as_bytes());

    buf.extend_from_slice(ESC_DOUBLE_SIZE);
    let title = if is_reprint {
        "*** KOT [REPRINT] ***"
    } else if is_delta {
        "*** KOT [ADD-ON / DELTA] ***"
    } else {
        "*** KOT (KITCHEN ORDER) ***"
    };
    buf.extend_from_slice(format!("{}\n", title).as_bytes());
    buf.extend_from_slice(ESC_NORMAL_SIZE);
    buf.extend_from_slice(ESC_BOLD_OFF);

    buf.extend_from_slice(b"------------------------------------------\n");
    buf.extend_from_slice(ESC_ALIGN_LEFT);
    buf.extend_from_slice(ESC_BOLD_ON);
    buf.extend_from_slice(format!("TABLE: {:<14} ORDER #{}\n", order.label, order.id).as_bytes());
    if let Some(area) = &order.area {
        buf.extend_from_slice(format!("AREA : {}\n", area).as_bytes());
    }
    buf.extend_from_slice(ESC_BOLD_OFF);
    buf.extend_from_slice(
        format!(
            "TIME : {}\n",
            chrono::Local::now().format("%d-%m-%Y %H:%M:%S")
        )
        .as_bytes(),
    );
    buf.extend_from_slice(b"------------------------------------------\n");

    // Items list (with emphasis on quantity and notes)
    buf.extend_from_slice(b"QTY    DISH NAME & CUSTOMIZATION\n");
    buf.extend_from_slice(b"------------------------------------------\n");
    for line in items_to_print {
        buf.extend_from_slice(ESC_BOLD_ON);
        buf.extend_from_slice(format!("{:>3} x  {}\n", line.qty, line.name).as_bytes());
        buf.extend_from_slice(ESC_BOLD_OFF);
        if let Some(note) = &line.note {
            buf.extend_from_slice(format!("       >> NOTE: {}\n", note).as_bytes());
        }
    }
    buf.extend_from_slice(b"------------------------------------------\n");

    buf.extend_from_slice(ESC_ALIGN_CENTER);
    buf.extend_from_slice(b"-- Kitchen Copy --\n\n\n");
    buf.extend_from_slice(ESC_CUT_PAPER);

    buf
}

/// Dispatches raw bytes to the destination printer according to configuration.
pub fn send_bytes(
    config: &PrinterConfig,
    target_override: Option<&str>,
    data: &[u8],
) -> Result<(), String> {
    match &config.mode {
        PrinterMode::Lpr => {
            let mut cmd = std::process::Command::new("lp");
            let target = target_override.unwrap_or(&config.bill_printer);
            if !target.trim().is_empty() {
                cmd.arg("-d").arg(target.trim());
            }
            cmd.stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to spawn lp: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(data);
            }
            let status = child.wait().map_err(|e| format!("lp wait failed: {e}"))?;
            if status.success() {
                Ok(())
            } else {
                Err("lp returned non-zero exit status".to_string())
            }
        }
        PrinterMode::Device(dev_path) => {
            let path = target_override.unwrap_or(dev_path);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|e| format!("Failed to open printer device {path}: {e}"))?;
            file.write_all(data)
                .map_err(|e| format!("Failed to write to printer device: {e}"))?;
            file.flush().map_err(|e| e.to_string())?;
            Ok(())
        }
        PrinterMode::Network(addr_str) => {
            let addr = target_override.unwrap_or(addr_str);
            let socket_addr: SocketAddr = addr
                .parse()
                .map_err(|e| format!("Invalid printer socket address {addr}: {e}"))?;
            let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(2))
                .map_err(|e| format!("Failed to connect to network printer at {addr}: {e}"))?;
            stream
                .write_all(data)
                .map_err(|e| format!("Failed to write to network printer: {e}"))?;
            stream.flush().map_err(|e| e.to_string())?;
            Ok(())
        }
        PrinterMode::Simulated => {
            let dir = Path::new("receipts");
            let _ = std::fs::create_dir_all(dir);
            let file_name = format!(
                "print_{}.bin",
                chrono::Local::now().format("%Y%m%d_%H%M%S_%3f")
            );
            let _ = std::fs::write(dir.join(file_name), data);
            Ok(())
        }
    }
}
