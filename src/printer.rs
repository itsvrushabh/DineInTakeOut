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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CartLine, Order, OrderStatus, PaymentMode, Service};
    use std::collections::HashMap;

    #[test]
    fn test_printer_config_default_and_from_map() {
        let def = PrinterConfig::default();
        assert_eq!(def.mode, PrinterMode::Lpr);
        assert!(def.bill_printer.is_empty());
        assert!(def.kot_printer.is_empty());
        assert!(def.cash_drawer_enabled);

        let mut map = HashMap::new();
        map.insert("PrinterMode".into(), "device".into());
        map.insert("BillPrinter".into(), "/dev/usb/lp1".into());
        map.insert("KotPrinter".into(), "/dev/usb/lp2".into());
        map.insert("CashDrawerEnabled".into(), "false".into());
        let cfg = PrinterConfig::from_map(&map);
        assert_eq!(cfg.mode, PrinterMode::Device("/dev/usb/lp1".into()));
        assert_eq!(cfg.bill_printer, "/dev/usb/lp1");
        assert_eq!(cfg.kot_printer, "/dev/usb/lp2");
        assert!(!cfg.cash_drawer_enabled);

        let mut map2 = HashMap::new();
        map2.insert("PrinterMode".into(), "usb".into());
        let cfg2 = PrinterConfig::from_map(&map2);
        assert_eq!(cfg2.mode, PrinterMode::Device("/dev/usb/lp0".into()));

        let mut map3 = HashMap::new();
        map3.insert("PrinterMode".into(), "network".into());
        let cfg3 = PrinterConfig::from_map(&map3);
        assert_eq!(cfg3.mode, PrinterMode::Network("127.0.0.1:9100".into()));

        let mut map4 = HashMap::new();
        map4.insert("PrinterMode".into(), "tcp".into());
        map4.insert("BillPrinter".into(), "192.168.1.50:9100".into());
        let cfg4 = PrinterConfig::from_map(&map4);
        assert_eq!(cfg4.mode, PrinterMode::Network("192.168.1.50:9100".into()));

        let mut map5 = HashMap::new();
        map5.insert("PrinterMode".into(), "simulated".into());
        let cfg5 = PrinterConfig::from_map(&map5);
        assert_eq!(cfg5.mode, PrinterMode::Simulated);

        let mut map6 = HashMap::new();
        map6.insert("PrinterMode".into(), "unknown_value".into());
        let cfg6 = PrinterConfig::from_map(&map6);
        assert_eq!(cfg6.mode, PrinterMode::Lpr);
    }

    fn test_order(id: u32, label: &str, area: Option<&str>, is_ac: bool) -> Order {
        Order {
            id,
            label: label.to_string(),
            service: Service::DineIn,
            table_number: Some(1),
            area: area.map(|a| a.to_string()),
            is_ac,
            ac_rate: if is_ac { 0.05 } else { 0.0 },
            discount_percent: 0.0,
            cart: Vec::new(),
            cart_index: 0,
            status: OrderStatus::Ordering,
            customer_mobile: None,
            payment_mode: None,
            kot_sent_count: 0,
        }
    }

    #[test]
    fn test_build_escpos_receipt() {
        let mut order = test_order(42, "T-1", Some("AC Room"), true);
        let mut line1 = CartLine::new("Paneer Butter Masala", 250.0, 2);
        line1.note = Some("Extra spicy".into());
        let mut line2 = CartLine::new("Butter Naan", 40.0, 3);
        line2.is_complimentary = true;
        let mut line3 = CartLine::new("Gulab Jamun", 50.0, 1);
        line3.discount_percent = 20.0;
        order.cart.push(line1);
        order.cart.push(line2);
        order.cart.push(line3);
        order.payment_mode = Some(PaymentMode::Upi);

        let bytes_default = build_escpos_receipt(&order, "", "", "", "", None, false);
        assert!(!bytes_default.is_empty());
        assert!(bytes_default.windows(ESC_CUT_PAPER.len()).any(|w| w == ESC_CUT_PAPER));

        let bytes = build_escpos_receipt(
            &order,
            "TEST RESTAURANT",
            "123 Main St",
            "9999999999",
            "GST12345",
            Some("9876543210"),
            true,
        );
        let str_rep = String::from_utf8_lossy(&bytes);
        assert!(str_rep.contains("TEST RESTAURANT"));
        assert!(str_rep.contains("123 Main St"));
        assert!(str_rep.contains("Contact: 9999999999"));
        assert!(str_rep.contains("GSTIN: GST12345"));
        assert!(str_rep.contains("Customer: 9876543210"));
        assert!(str_rep.contains("Paneer Butter Masala"));
        assert!(str_rep.contains("Extra spicy"));
        assert!(str_rep.contains("[NC]"));
        assert!(str_rep.contains("[DISC]"));
        assert!(str_rep.contains("GRAND TOTAL"));
        assert!(str_rep.contains("Settled Via"));
        assert!(bytes.windows(ESC_DRAWER_KICK.len()).any(|w| w == ESC_DRAWER_KICK));
    }

    #[test]
    fn test_build_escpos_kot() {
        let mut order = test_order(10, "Table 5", Some("Garden"), false);
        let mut line = CartLine::new("Veg Biryani", 180.0, 2);
        line.note = Some("Less oil".into());
        order.cart.push(line.clone());

        let bytes1 = build_escpos_kot(&order, &order.cart, false, false, "KOT RESTAURANT");
        let str1 = String::from_utf8_lossy(&bytes1);
        assert!(str1.contains("*** KOT (KITCHEN ORDER) ***"));
        assert!(str1.contains("AREA : Garden"));
        assert!(str1.contains("Veg Biryani"));
        assert!(str1.contains("Less oil"));

        let bytes2 = build_escpos_kot(&order, &order.cart, true, false, "");
        let str2 = String::from_utf8_lossy(&bytes2);
        assert!(str2.contains("*** KOT [REPRINT] ***"));
        assert!(str2.contains("SHREE KRISHNA RESTAURANT"));

        let bytes3 = build_escpos_kot(&order, &order.cart, false, true, "TEST");
        let str3 = String::from_utf8_lossy(&bytes3);
        assert!(str3.contains("*** KOT [ADD-ON / DELTA] ***"));
    }

    #[test]
    fn test_send_bytes_simulated_and_device_and_network() {
        let sim_config = PrinterConfig {
            mode: PrinterMode::Simulated,
            ..Default::default()
        };
        assert!(send_bytes(&sim_config, None, b"TEST DATA").is_ok());
        let _ = std::fs::remove_dir_all("receipts");

        let temp_file = std::env::temp_dir().join(format!("mock_device_{}", std::process::id()));
        let _ = std::fs::File::create(&temp_file).unwrap();
        let dev_config = PrinterConfig {
            mode: PrinterMode::Device(temp_file.to_str().unwrap().to_string()),
            ..Default::default()
        };
        assert!(send_bytes(&dev_config, None, b"DEVICE BYTES").is_ok());
        let read_bytes = std::fs::read(&temp_file).unwrap();
        assert_eq!(read_bytes, b"DEVICE BYTES");
        let _ = std::fs::remove_file(&temp_file);

        let err_config = PrinterConfig {
            mode: PrinterMode::Device("/dev/nonexistent_dir/printer_xyz".into()),
            ..Default::default()
        };
        assert!(send_bytes(&err_config, None, b"ERR").is_err());

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let local_addr = listener.local_addr().unwrap().to_string();
        let net_config = PrinterConfig {
            mode: PrinterMode::Network(local_addr),
            ..Default::default()
        };
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::Read;
                let mut buf = [0u8; 16];
                let _ = stream.read(&mut buf);
            }
        });
        assert!(send_bytes(&net_config, None, b"NET BYTES").is_ok());
        let _ = handle.join();

        let bad_addr_cfg = PrinterConfig {
            mode: PrinterMode::Network("invalid-address".into()),
            ..Default::default()
        };
        assert!(send_bytes(&bad_addr_cfg, None, b"FAIL").is_err());
    }
}

