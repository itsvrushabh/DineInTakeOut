//! Core domain types for menu items, orders, tables, and saved receipts.

use chrono::{DateTime, Local};

/// Minutes a table stays in the Cleaning state before it becomes Ready again.
#[allow(dead_code)]
pub const CLEANING_MINUTES: i64 = 10;

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

#[derive(PartialEq, Clone, Copy, Debug)]
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

    #[allow(dead_code)]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "TAKE-OUT" => Some(Self::TakeOut),
            "DINE-IN" => Some(Self::DineIn),
            _ => None,
        }
    }

    pub fn tax_rate(self) -> f64 {
        match self {
            Self::DineIn => 0.0,
            Self::TakeOut => 0.08,
        }
    }
}

#[allow(dead_code)]
/// How the bill was settled, captured in the payment confirmation popup.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaymentMode {
    Cash,
    Upi,
    Card,
    PersonCredit,
    HaveItOnHotel,
}

impl PaymentMode {
    pub fn all() -> [Self; 5] {
        [
            Self::Cash,
            Self::Upi,
            Self::Card,
            Self::PersonCredit,
            Self::HaveItOnHotel,
        ]
    }

    /// Stable identifier stored in the database and printed on receipts.
    pub fn label(self) -> &'static str {
        match self {
            Self::Cash => "CASH",
            Self::Upi => "UPI",
            Self::Card => "CARD",
            Self::PersonCredit => "PERSON_CREDIT",
            Self::HaveItOnHotel => "HAVE_IT_ON_HOTEL",
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            Self::Cash => "Cash",
            Self::Upi => "UPI",
            Self::Card => "Card",
            Self::PersonCredit => "Person credit",
            Self::HaveItOnHotel => "Have it on hotel",
        }
    }

    /// Kept alongside `label` for reading modes back from stored records.
    #[allow(dead_code)]
    pub fn parse(raw: &str) -> Option<Self> {
        PaymentMode::all().into_iter().find(|m| m.label() == raw)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Search,
    Menu,
    Cart,
    Tables,
    RecentBills,
    /// Modal capture of the customer mobile number during billing.
    MobileEntry,
    /// Modal confirmation of the mode of payment during billing.
    PaymentMode,
    AdminMode,
}

#[derive(PartialEq, Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum OrderStatus {
    Ordering,
    Serving,
    BillRequested,
    Paid,
}

impl OrderStatus {
    /// Next stage of the dine-in lifecycle:
    /// Ordering → Serving → Ready-for-bill. Paid is terminal.
    pub fn advance(self) -> Option<Self> {
        match self {
            Self::Ordering => Some(Self::Serving),
            Self::Serving => Some(Self::BillRequested),
            Self::BillRequested | Self::Paid => None,
        }
    }

    /// The physical-table state that mirrors this order stage.
    pub fn table_status(self) -> TableStatus {
        match self {
            Self::Ordering => TableStatus::Ordering,
            Self::Serving => TableStatus::Serving,
            Self::BillRequested => TableStatus::BillRequested,
            Self::Paid => TableStatus::Paid,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ordering => "ORDERING",
            Self::Serving => "SERVING",
            Self::BillRequested => "BILL_REQUESTED",
            Self::Paid => "PAID",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "ORDERING" => Some(Self::Ordering),
            "SERVING" => Some(Self::Serving),
            "BILL_REQUESTED" => Some(Self::BillRequested),
            "PAID" => Some(Self::Paid),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn parse(raw: &str) -> Option<Self> {
        TableArea::all()
            .into_iter()
            .find(|area| area.label() == raw)
    }

    pub fn table_count(self) -> usize {
        match self {
            Self::FrontGarden => 5,
            Self::ACRooms => 4,
            Self::MainHall => 6,
            Self::BackGarden => 8,
        }
    }

    /// Extra charge (as a fraction of the subtotal) for air-conditioned
    /// seating.
    pub fn ac_surcharge(self) -> f64 {
        match self {
            Self::ACRooms => 0.06,
            _ => 0.0,
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

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TableStatus {
    Ready,
    Ordering,
    Serving,
    BillRequested,
    Paid,
    Dirty,
}

#[allow(dead_code)]
impl TableStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Ordering => "ORDERING",
            Self::Serving => "SERVING",
            Self::BillRequested => "BILL_REQUESTED",
            Self::Paid => "PAID",
            Self::Dirty => "CLEANING",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "READY" => Some(Self::Ready),
            "ORDERING" => Some(Self::Ordering),
            "SERVING" => Some(Self::Serving),
            "BILL_REQUESTED" => Some(Self::BillRequested),
            "PAID" => Some(Self::Paid),
            "CLEANING" => Some(Self::Dirty),
            _ => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Ordering => "Taking order",
            Self::Serving => "Serving",
            Self::BillRequested => "Ready for bill",
            Self::Paid => "Bill paid",
            Self::Dirty => "Cleaning",
        }
    }

    pub fn color(self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Ready => Color::Green,
            Self::Ordering => Color::Yellow,
            Self::Serving => Color::Blue,
            Self::BillRequested => Color::Cyan,
            Self::Paid => Color::Magenta,
            Self::Dirty => Color::Red,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct PhysicalTable {
    pub number: usize,
    pub area: TableArea,
    pub status: TableStatus,
    pub order_id: Option<u32>,
    /// When the bill was closed and cleaning started; drives auto-ready.
    pub dirty_since: Option<DateTime<Local>>,
}

#[allow(dead_code)]
impl PhysicalTable {
    pub fn ready(area: TableArea, number: usize) -> Self {
        Self {
            number,
            area,
            status: TableStatus::Ready,
            order_id: None,
            dirty_since: None,
        }
    }
}

/// Full price breakdown for an order. AC-room seating adds a surcharge that
/// is taxed together with the food (GST applies on subtotal + surcharge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BillTotals {
    pub subtotal: f64,
    pub ac_charge: f64,
    pub gst_rate: f64,
    pub gst: f64,
    pub total: f64,
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

impl Order {
    pub fn ac_surcharge(&self) -> f64 {
        self.area.map_or(0.0, TableArea::ac_surcharge)
    }

    /// GST rate per India restaurant rules: 5% where tax applies (AC rooms);
    /// take-out keeps its existing 8% rate.
    pub fn gst_rate(&self) -> f64 {
        match self.service {
            Service::TakeOut => Service::TakeOut.tax_rate(),
            Service::DineIn => {
                if self.area == Some(TableArea::ACRooms) {
                    0.05
                } else {
                    Service::DineIn.tax_rate()
                }
            }
        }
    }

    pub fn totals(&self) -> BillTotals {
        let subtotal: f64 = self.cart.iter().map(|line| line.total()).sum();
        let ac_charge = subtotal * self.ac_surcharge();
        let gst_rate = self.gst_rate();
        let gst = (subtotal + ac_charge) * gst_rate;
        BillTotals {
            subtotal,
            ac_charge,
            gst_rate,
            gst,
            total: subtotal + ac_charge + gst,
        }
    }
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
    use super::{CartLine, Order, OrderStatus, Service, TableArea, CLEANING_MINUTES};

    fn order(service: Service, area: Option<TableArea>) -> Order {
        Order {
            id: 1,
            label: "Test".to_string(),
            service,
            table_number: None,
            area,
            cart: vec![CartLine {
                name: "Dal Makhani".to_string(),
                unit_price: 100.0,
                qty: 2,
            }],
            cart_index: 0,
            status: OrderStatus::Ordering,
        }
    }

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

        assert_eq!(total, 23);
        assert_eq!(TableArea::BackGarden.table_count(), 8);
    }

    #[test]
    fn ac_rooms_add_surcharge_and_gst() {
        // ₹200 food in an AC room: 6% AC charge + 5% GST on (200 + 12).
        let totals = order(Service::DineIn, Some(TableArea::ACRooms)).totals();
        assert_eq!(totals.subtotal, 200.0);
        assert!((totals.ac_charge - 12.0).abs() < 1e-9);
        assert!((totals.gst - 10.6).abs() < 1e-9);
        assert!((totals.total - 222.6).abs() < 1e-9);
    }

    #[test]
    fn non_ac_dine_in_has_no_extra_charges() {
        let totals = order(Service::DineIn, Some(TableArea::MainHall)).totals();
        assert_eq!(totals.ac_charge, 0.0);
        assert_eq!(totals.gst, 0.0);
        assert_eq!(totals.total, 200.0);
    }

    #[test]
    fn take_out_keeps_existing_tax_rate() {
        let totals = order(Service::TakeOut, None).totals();
        assert!((totals.gst - 16.0).abs() < 1e-9);
        assert!((totals.total - 216.0).abs() < 1e-9);
    }

    #[test]
    fn lifecycle_advances_then_stops_at_paid() {
        use super::OrderStatus::*;
        assert_eq!(Ordering.advance(), Some(Serving));
        assert_eq!(Serving.advance(), Some(BillRequested));
        assert_eq!(BillRequested.advance(), None);
        assert_eq!(Paid.advance(), None);
    }

    #[test]
    fn cleaning_window_constant_matches_requirement() {
        assert_eq!(CLEANING_MINUTES, 10);
    }

    #[test]
    fn payment_modes_roundtrip_through_labels() {
        for mode in super::PaymentMode::all() {
            assert_eq!(super::PaymentMode::parse(mode.label()), Some(mode));
        }
        assert_eq!(super::PaymentMode::all().len(), 5);
        assert_eq!(super::PaymentMode::Upi.display(), "UPI");
    }
}
