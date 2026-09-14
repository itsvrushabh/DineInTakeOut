//! Configuration persistence, default menu data, and CSV import/export.

use std::{collections::HashMap, io, path::Path};

use crate::models::{Area, MenuItem, Offer};

pub const CSV_HEADER: [&str; 4] = ["Category", "Item Name", "Unit", "Price"];

/// Default menu — empty by default; all menu items are sourced from `menu.csv`.
pub fn default_menu() -> Vec<MenuItem> {
    Vec::new()
}

// -- menu.csv -----------------------------------------------------------------

pub fn load_menu(path: &Path) -> io::Result<Vec<MenuItem>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(io::Error::other)?;
    let mut items = Vec::new();
    for row in rdr.records() {
        let rec = row.map_err(io::Error::other)?;
        let get = |i: usize| rec.get(i).unwrap_or("").trim().to_string();
        let price: f64 = get(3).parse().unwrap_or(0.0);
        if get(1).is_empty() || price <= 0.0 {
            continue;
        }
        let is_available = if let Some(val) = rec.get(4) {
            !matches!(
                val.trim().to_ascii_lowercase().as_str(),
                "no" | "false" | "0" | "out"
            )
        } else {
            true
        };
        items.push(MenuItem {
            category: get(0),
            name: get(1),
            unit: get(2),
            price,
            is_available,
        });
    }
    Ok(items)
}

pub fn export_menu_csv(path: &Path, items: &[MenuItem]) -> io::Result<usize> {
    let mut wtr = csv::Writer::from_path(path).map_err(io::Error::other)?;
    wtr.write_record(CSV_HEADER).map_err(io::Error::other)?;
    for it in items {
        wtr.write_record(it.to_csv_row())
            .map_err(io::Error::other)?;
    }
    wtr.flush()?;
    Ok(items.len())
}

// -- table.csv / areas.csv ---------------------------------------------------

pub const TABLE_CSV_HEADER: [&str; 3] = ["Table Type", "Count", "IsAC"];

/// Loads dining tables/areas from `table.csv`. Flexible with column header names
/// ("Table Type" or "Area", "Count" or "TableCount", "IsAC" or "AC").
pub fn load_table_csv(path: &Path) -> io::Result<Vec<Area>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(io::Error::other)?;

    let headers = rdr.headers().map_err(io::Error::other)?.clone();
    let mut name_idx = 0;
    let mut count_idx = 1;
    let mut ac_idx = None;

    for (i, h) in headers.iter().enumerate() {
        let norm = h.trim().to_ascii_lowercase();
        if norm.contains("type") || norm.contains("area") || norm.contains("section") {
            name_idx = i;
        } else if norm.contains("count") || norm.contains("tables") || norm.contains("qty") {
            count_idx = i;
        } else if norm.contains("ac") {
            ac_idx = Some(i);
        }
    }

    let mut areas = Vec::new();
    for row in rdr.records() {
        let rec = row.map_err(io::Error::other)?;
        let name = rec.get(name_idx).unwrap_or("").trim().to_string();
        if name.is_empty() {
            continue;
        }
        let count_str = rec.get(count_idx).unwrap_or("").trim();
        let table_count = count_str.parse::<usize>().unwrap_or(1).max(1);

        let is_ac = if let Some(idx) = ac_idx {
            matches!(
                rec.get(idx)
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase()
                    .as_str(),
                "yes" | "y" | "1" | "true"
            )
        } else {
            name.to_ascii_lowercase().contains("ac")
        };

        areas.push(Area {
            name,
            is_ac,
            table_count,
        });
    }
    Ok(areas)
}

pub fn export_table_csv(path: &Path, areas: &[Area]) -> io::Result<usize> {
    let mut wtr = csv::Writer::from_path(path).map_err(io::Error::other)?;
    wtr.write_record(TABLE_CSV_HEADER)
        .map_err(io::Error::other)?;
    for a in areas {
        wtr.write_record([
            a.name.clone(),
            a.table_count.to_string(),
            if a.is_ac { "yes" } else { "no" }.to_string(),
        ])
        .map_err(io::Error::other)?;
    }
    wtr.flush()?;
    Ok(areas.len())
}

pub fn load_areas_csv(path: &Path) -> io::Result<Vec<Area>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(io::Error::other)?;
    let mut areas = Vec::new();
    for row in rdr.records() {
        let rec = row.map_err(io::Error::other)?;
        let get = |i: usize| rec.get(i).unwrap_or("").trim().to_string();
        let name = get(0);
        if name.is_empty() {
            continue;
        }
        let is_ac = matches!(
            get(1).to_ascii_lowercase().as_str(),
            "yes" | "y" | "1" | "true"
        );
        let table_count = get(2).parse::<usize>().unwrap_or(1).max(1);
        areas.push(Area {
            name,
            is_ac,
            table_count,
        });
    }
    Ok(areas)
}

pub fn export_areas_csv(path: &Path, areas: &[Area]) -> io::Result<usize> {
    let mut wtr = csv::Writer::from_path(path).map_err(io::Error::other)?;
    wtr.write_record(["Area", "IsAC", "TableCount"])
        .map_err(io::Error::other)?;
    for a in areas {
        wtr.write_record([
            a.name.clone(),
            if a.is_ac { "yes" } else { "no" }.to_string(),
            a.table_count.to_string(),
        ])
        .map_err(io::Error::other)?;
    }
    wtr.flush()?;
    Ok(areas.len())
}

// -- offers.csv ---------------------------------------------------------------

pub fn load_offers_csv(path: &Path) -> io::Result<Vec<Offer>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(io::Error::other)?;
    let mut offers = Vec::new();
    for row in rdr.records() {
        let rec = row.map_err(io::Error::other)?;
        let get = |i: usize| rec.get(i).unwrap_or("").trim().to_string();
        let name = get(0);
        let percent = get(1).parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0);
        if name.is_empty() || percent <= 0.0 {
            continue;
        }
        offers.push(Offer {
            id: 0,
            name,
            discount_percent: percent,
        });
    }
    Ok(offers)
}

pub fn export_offers_csv(path: &Path, offers: &[Offer]) -> io::Result<usize> {
    let mut wtr = csv::Writer::from_path(path).map_err(io::Error::other)?;
    wtr.write_record(["Name", "DiscountPercent"])
        .map_err(io::Error::other)?;
    for o in offers {
        wtr.write_record([o.name.clone(), format!("{:.0}", o.discount_percent)])
            .map_err(io::Error::other)?;
    }
    wtr.flush()?;
    Ok(offers.len())
}

// -- config.csv ---------------------------------------------------------------

pub fn load_config_csv(path: &Path) -> io::Result<HashMap<String, String>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(io::Error::other)?;
    let mut map = HashMap::new();
    for row in rdr.records() {
        let rec = row.map_err(io::Error::other)?;
        let key = rec.get(0).unwrap_or("").trim().to_string();
        let value = rec.get(1).unwrap_or("").trim().to_string();
        if !key.is_empty() {
            map.insert(key, value);
        }
    }
    Ok(map)
}

pub fn export_config_csv(
    path: &Path,
    restaurant_name: &str,
    address: &str,
    contact: &str,
    gst_number: &str,
    ac_rate: f64,
    upi_id: &str,
) -> io::Result<()> {
    let mut wtr = csv::Writer::from_path(path).map_err(io::Error::other)?;
    wtr.write_record(["Key", "Value"])
        .map_err(io::Error::other)?;
    wtr.write_record(["RestaurantName", restaurant_name])
        .map_err(io::Error::other)?;
    wtr.write_record(["Address", address])
        .map_err(io::Error::other)?;
    wtr.write_record(["Contact", contact])
        .map_err(io::Error::other)?;
    wtr.write_record(["GSTNumber", gst_number])
        .map_err(io::Error::other)?;
    let ac_rate_str = format!("{:.2}", ac_rate * 100.0);
    wtr.write_record(["AcRate", &ac_rate_str])
        .map_err(io::Error::other)?;
    wtr.write_record(["UpiId", upi_id])
        .map_err(io::Error::other)?;
    wtr.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_menu_is_empty() {
        let items = default_menu();
        assert!(
            items.is_empty(),
            "default_menu should have no hardcoded items"
        );
    }

    #[test]
    fn table_csv_roundtrip() {
        let dir = std::env::temp_dir().join(format!("test_table_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("table.csv");

        let input_areas = vec![
            Area {
                name: "Main Hall".to_string(),
                is_ac: false,
                table_count: 8,
            },
            Area {
                name: "AC Dining".to_string(),
                is_ac: true,
                table_count: 6,
            },
        ];

        export_table_csv(&path, &input_areas).unwrap();
        let loaded = load_table_csv(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Main Hall");
        assert_eq!(loaded[0].table_count, 8);
        assert!(!loaded[0].is_ac);
        assert_eq!(loaded[1].name, "AC Dining");
        assert_eq!(loaded[1].table_count, 6);
        assert!(loaded[1].is_ac);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_csv_roundtrip() {
        let dir = std::env::temp_dir().join(format!("test_config_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.csv");

        export_config_csv(
            &path,
            "SHREE KRISHNA RESTAURANT",
            "Station Road, Near Main Market",
            "+91 98765 43210",
            "27AAPFU0939F1ZV",
            0.065,
            "krishna@upi",
        )
        .unwrap();
        let map = load_config_csv(&path).unwrap();

        assert_eq!(
            map.get("RestaurantName").unwrap(),
            "SHREE KRISHNA RESTAURANT"
        );
        assert_eq!(
            map.get("Address").unwrap(),
            "Station Road, Near Main Market"
        );
        assert_eq!(map.get("Contact").unwrap(), "+91 98765 43210");
        assert_eq!(map.get("GSTNumber").unwrap(), "27AAPFU0939F1ZV");
        assert_eq!(map.get("AcRate").unwrap(), "6.50");
        assert_eq!(map.get("UpiId").unwrap(), "krishna@upi");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn areas_csv_roundtrip() {
        let dir = std::env::temp_dir().join(format!("test_areas_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("areas.csv");

        let input_areas = vec![
            Area {
                name: "Rooftop".to_string(),
                is_ac: false,
                table_count: 5,
            },
            Area {
                name: "VIP Lounge".to_string(),
                is_ac: true,
                table_count: 3,
            },
        ];

        export_areas_csv(&path, &input_areas).unwrap();
        let loaded = load_areas_csv(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Rooftop");
        assert!(!loaded[0].is_ac);
        assert_eq!(loaded[0].table_count, 5);
        assert_eq!(loaded[1].name, "VIP Lounge");
        assert!(loaded[1].is_ac);
        assert_eq!(loaded[1].table_count, 3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn offers_csv_roundtrip() {
        let dir = std::env::temp_dir().join(format!("test_offers_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("offers.csv");

        let input_offers = vec![
            Offer {
                id: 0,
                name: "Festive Discount".to_string(),
                discount_percent: 15.0,
            },
            Offer {
                id: 0,
                name: "Happy Hour".to_string(),
                discount_percent: 20.0,
            },
        ];

        export_offers_csv(&path, &input_offers).unwrap();
        let loaded = load_offers_csv(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Festive Discount");
        assert_eq!(loaded[0].discount_percent, 15.0);
        assert_eq!(loaded[1].name, "Happy Hour");
        assert_eq!(loaded[1].discount_percent, 20.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn menu_csv_roundtrip_and_availability_flags() {
        let dir = std::env::temp_dir().join(format!("test_menu_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("menu.csv");

        let items = vec![
            MenuItem::new("Starters", "Paneer Tikka", "plate", 180.0),
            MenuItem::new("Mains", "Dal Tadka", "bowl", 120.0),
        ];

        export_menu_csv(&path, &items).unwrap();
        let loaded = load_menu(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Paneer Tikka");
        assert!(loaded[0].is_available);

        // Custom CSV testing 86 / unavailable values and invalid rows
        let custom_csv = "Category,Item Name,Unit,Price,Available\n\
                          Drinks,Mango Lassi,glass,50,no\n\
                          Drinks,Cold Coffee,glass,60,false\n\
                          Drinks,Sweet Lime,glass,40,out\n\
                          Food,,plate,100,yes\n\
                          Food,Invalid Price,plate,-10,yes\n\
                          Food,Veg Pulao,plate,110,yes\n";
        std::fs::write(&path, custom_csv).unwrap();
        let parsed = load_menu(&path).unwrap();
        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[0].name, "Mango Lassi");
        assert!(!parsed[0].is_available);
        assert_eq!(parsed[1].name, "Cold Coffee");
        assert!(!parsed[1].is_available);
        assert_eq!(parsed[2].name, "Sweet Lime");
        assert!(!parsed[2].is_available);
        assert_eq!(parsed[3].name, "Veg Pulao");
        assert!(parsed[3].is_available);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
