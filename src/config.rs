//! Configuration persistence, default menu data, and CSV import/export.

use std::{
    collections::HashMap,
    io,
    path::Path,
};

use crate::models::{Area, MenuItem, Offer};

pub const CSV_HEADER: [&str; 4] = ["Category", "Item Name", "Unit", "Price"];

/// Default menu — Indian restaurant items with a fixed price chosen from the
/// standard estimated range. Used to seed an empty menu.
pub fn default_menu() -> Vec<MenuItem> {
    let rows: &[(&str, &str, &str, f64)] = &[
        ("Breakfast & Snacks", "Samosa", "1-2 pcs", 20.0),
        ("Breakfast & Snacks", "Vada Pav", "1 pc", 25.0),
        ("Breakfast & Snacks", "Idli Sambhar", "2 pcs", 35.0),
        ("Breakfast & Snacks", "Masala Dosa", "1 plate", 60.0),
        (
            "Breakfast & Snacks",
            "Aloo Paratha (with curd/pickle)",
            "1 plate (1-2 pcs)",
            45.0,
        ),
        (
            "Breakfast & Snacks",
            "Chole Bhature",
            "2 bhature + curry",
            75.0,
        ),
        (
            "Breakfast & Snacks",
            "Poori Bhaji / Sabzi",
            "4 pooris + curry",
            55.0,
        ),
        ("Breads & Rice", "Tawa Roti / Phulka", "Per piece", 10.0),
        (
            "Breads & Rice",
            "Tandoori Roti / Butter Roti",
            "Per piece",
            18.0,
        ),
        ("Breads & Rice", "Plain / Butter Naan", "Per piece", 35.0),
        (
            "Breads & Rice",
            "Plain Steamed / Jeera Rice",
            "Full plate",
            75.0,
        ),
        (
            "Main Course (Veg)",
            "Dal Tadka / Dal Fry",
            "Full plate",
            95.0,
        ),
        ("Main Course (Veg)", "Dal Makhani", "Full plate", 120.0),
        (
            "Main Course (Veg)",
            "Aloo Gobi / Jeera Aloo",
            "Full plate",
            100.0,
        ),
        (
            "Main Course (Veg)",
            "Mixed Vegetable Curry",
            "Full plate",
            125.0,
        ),
        ("Main Course (Veg)", "Paneer Butter Masala", "Full plate", 165.0),
        ("Main Course (Veg)", "Palak Paneer", "Full plate", 150.0),
        ("Main Course (Veg)", "Kadai Paneer", "Full plate", 160.0),
        ("Main Course (Veg)", "Veg Biryani / Pulao", "Full plate", 130.0),
        (
            "Main Course (Veg)",
            "Special Veg Thali",
            "Curry + Dal + 3 Roti + Rice + Sweet",
            160.0,
        ),
        (
            "Main Course (Non-Veg)",
            "Egg Curry (with 2 eggs)",
            "Full plate",
            110.0,
        ),
        ("Main Course (Non-Veg)", "Chicken Curry / Masala", "Full plate", 190.0),
        ("Main Course (Non-Veg)", "Butter Chicken", "Full plate", 225.0),
        ("Main Course (Non-Veg)", "Chicken Biryani", "Full plate", 175.0),
        ("Main Course (Non-Veg)", "Mutton Curry / Biryani", "Full plate", 275.0),
        ("Fast Food & Street", "Pav Bhaji", "2 pav + bhaji", 75.0),
        (
            "Fast Food & Street",
            "Veg Burger / Sandwich",
            "1 item",
            60.0,
        ),
        (
            "Fast Food & Street",
            "Veg / Chicken Momos",
            "6-8 pcs",
            65.0,
        ),
        (
            "Fast Food & Street",
            "Veg / Egg / Chicken Roll",
            "1 roll",
            65.0,
        ),
        ("Fast Food & Street", "Veg Fried Rice / Noodles", "Full plate", 100.0),
        ("Desserts & Beverages", "Gulab Jamun", "2 pcs", 45.0),
        ("Desserts & Beverages", "Rasgulla", "2 pcs", 45.0),
        ("Desserts & Beverages", "Cutting Chai / Special Tea", "1 cup / glass", 15.0),
        ("Desserts & Beverages", "Filter Coffee / Cold Coffee", "1 cup / glass", 40.0),
        ("Desserts & Beverages", "Sweet / Salted / Mango Lassi", "1 glass", 50.0),
    ];

    rows.iter()
        .map(|&(category, name, unit, price)| MenuItem {
            category: category.to_string(),
            name: name.to_string(),
            unit: unit.to_string(),
            price,
        })
        .collect()
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
        items.push(MenuItem {
            category: get(0),
            name: get(1),
            unit: get(2),
            price,
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

// -- areas.csv ----------------------------------------------------------------

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

pub fn export_config_csv(path: &Path, gst_number: &str, ac_rate: f64) -> io::Result<()> {
    let mut wtr = csv::Writer::from_path(path).map_err(io::Error::other)?;
    wtr.write_record(["Key", "Value"])
        .map_err(io::Error::other)?;
    wtr.write_record(["GSTNumber", gst_number])
        .map_err(io::Error::other)?;
    let ac_rate_str = format!("{:.2}", ac_rate * 100.0);
    wtr.write_record(["AcRate", &ac_rate_str])
        .map_err(io::Error::other)?;
    wtr.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_menu_has_valid_items() {
        let items = default_menu();
        assert_eq!(items.len(), 35);
        for item in &items {
            assert!(!item.name.is_empty());
            assert!(!item.category.is_empty());
            assert!(item.price > 0.0);
        }
    }

    #[test]
    fn config_csv_roundtrip() {
        let dir = std::env::temp_dir().join(format!("test_config_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.csv");

        export_config_csv(&path, "27AAPFU0939F1ZV", 0.065).unwrap();
        let map = load_config_csv(&path).unwrap();

        assert_eq!(map.get("GSTNumber").unwrap(), "27AAPFU0939F1ZV");
        assert_eq!(map.get("AcRate").unwrap(), "6.50");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
