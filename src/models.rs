//! Core domain types for menu items, orders, tables, and saved receipts.

#[derive(Clone)]
pub struct MenuItem {
    pub category: String,
    pub name: String,
    pub unit: String,
    pub price: f64,
}

impl MenuItem {
    pub fn to_csv_row(&self) -> Vec<String> {
        vec![
            self.category.clone(),
            self.name.clone(),
            self.unit.clone(),
            format!("{:.2}", self.price),
        ]
    }
}

#[derive(Clone)]
pub struct CartLine {
    pub name: String,
    pub unit_price: f64,
    pub qty: u32,
}

impl CartLine {
    pub fn total(&self) -> f64 {
        self.unit_price * self.qty as f64
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum Service {
    DineIn,
    TakeOut,
}

impl Service {
    pub fn label(self) -> &'static str {
        match self {
            Self::DineIn => "DINE-IN",
            Self::TakeOut => "TAKE-OUT",
        }
    }

    pub fn tax_rate(self) -> f64 {
        match self {
            Self::DineIn => 0.0,
            Self::TakeOut => 0.08,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Search,
    Menu,
    Cart,
    Tables,
    RecentBills,
}

#[derive(PartialEq, Clone, Copy)]
pub enum OrderStatus {
    Ordering,
    Serving,
    Paid,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TableArea {
    FrontGarden,
    ACRooms,
    MainHall,
    BackGarden,
}

impl TableArea {
    pub fn label(self) -> &'static str {
        match self {
            Self::FrontGarden => "Front Garden",
            Self::ACRooms => "AC Rooms",
            Self::MainHall => "Main Hall",
            Self::BackGarden => "Back Garden",
        }
    }

    pub fn table_count(self) -> usize {
        match self {
            Self::FrontGarden => 5,
            Self::ACRooms => 4,
            Self::MainHall => 6,
            Self::BackGarden => 6,
        }
    }

    pub fn all() -> [Self; 4] {
        [
            Self::FrontGarden,
            Self::ACRooms,
            Self::MainHall,
            Self::BackGarden,
        ]
    }

    pub fn from_digit(digit: char) -> Option<Self> {
        match digit {
            '1' => Some(Self::FrontGarden),
            '2' => Some(Self::ACRooms),
            '3' => Some(Self::MainHall),
            '4' => Some(Self::BackGarden),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TableStatus {
    Empty,
    HasOrder,
    Serving,
}

#[derive(Clone, Debug)]
pub struct PhysicalTable {
    pub number: usize,
    pub area: TableArea,
    pub status: TableStatus,
    pub order_id: Option<u32>,
}

#[derive(Clone)]
pub struct Order {
    pub id: u32,
    pub label: String,
    pub service: Service,
    pub table_number: Option<usize>,
    pub area: Option<TableArea>,
    pub cart: Vec<CartLine>,
    pub cart_index: usize,
    pub status: OrderStatus,
}

#[derive(Clone)]
pub struct BillSummary {
    pub id: u32,
    pub label: String,
    pub service: Service,
    pub total: f64,
    pub receipt: String,
}

#[cfg(test)]
mod tests {
    use super::{CartLine, TableArea};

    #[test]
    fn cart_line_total_uses_quantity() {
        let line = CartLine {
            name: "Samosa".to_string(),
            unit_price: 20.0,
            qty: 3,
        };

        assert_eq!(line.total(), 60.0);
    }

    #[test]
    fn table_areas_have_expected_capacity() {
        let total: usize = TableArea::all()
            .into_iter()
            .map(TableArea::table_count)
            .sum();

        assert_eq!(total, 21);
    }
}
