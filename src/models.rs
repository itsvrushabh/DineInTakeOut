//! Core domain types for menu items, orders, tables, and saved receipts.

use chrono::{DateTime, Local};

/// Minutes a table stays in the Cleaning state before it becomes Ready again.
#[allow(dead_code)]
pub const CLEANING_MINUTES: i64 = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub category: String,
    pub name: String,
    pub unit: String,
    pub price: f64,
    pub is_available: bool,
}

impl MenuItem {
    pub fn new(
        category: impl Into<String>,
        name: impl Into<String>,
        unit: impl Into<String>,
        price: f64,
    ) -> Self {
        Self {
            category: category.into(),
            name: name.into(),
            unit: unit.into(),
            price,
            is_available: true,
        }
    }

    pub fn to_csv_row(&self) -> Vec<String> {
        vec![
            self.category.clone(),
            self.name.clone(),
            self.unit.clone(),
            format!("{:.2}", self.price),
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CartLine {
    pub name: String,
    pub unit_price: f64,
    pub qty: u32,
    pub note: Option<String>,
}

impl CartLine {
    pub fn new(name: impl Into<String>, unit_price: f64, qty: u32) -> Self {
        Self {
            name: name.into(),
            unit_price,
            qty,
            note: None,
        }
    }

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

#[derive(Clone, Copy, PartialEq, Debug)]
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
    /// Modal selection of a discount offer to apply at billing.
    OfferSelect,
    TableJump,
    DailyReport,
    BillSearch,
    UpiQr,
    TableMove,
    ItemNote,
}

#[derive(PartialEq, Clone, Copy, Debug)]
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

/// A configurable dining area (e.g. First Floor, Ground Floor, Serve in Cars).
/// Areas are user-defined via the admin panel, each with its own table count
/// and an `is_ac` flag that triggers the (global) AC surcharge and 5% GST.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub name: String,
    pub is_ac: bool,
    pub table_count: usize,
}

impl Area {
    /// Seed layout: preserves the names the pre-dynamic database used so that
    pub fn defaults() -> Vec<Area> {
        vec![
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
            Area {
                name: "Family Section".to_string(),
                is_ac: true,
                table_count: 4,
            },
            Area {
                name: "Garden".to_string(),
                is_ac: false,
                table_count: 6,
            },
        ]
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
#[derive(Clone, Debug)]
pub struct PhysicalTable {
    pub number: usize,
    pub area: String,
    pub status: TableStatus,
    pub order_id: Option<u32>,
    /// When the bill was closed and cleaning started; drives auto-ready.
    pub dirty_since: Option<DateTime<Local>>,
}

#[allow(dead_code)]
impl PhysicalTable {
    pub fn ready(area: &str, number: usize) -> Self {
        Self {
            number,
            area: area.to_string(),
            status: TableStatus::Ready,
            order_id: None,
            dirty_since: None,
        }
    }
}

/// A discount offer applied at billing time.
#[derive(Clone, Debug)]
pub struct Offer {
    pub id: u32,
    pub name: String,
    pub discount_percent: f64,
}

/// Full price breakdown for an order. AC-room seating adds a surcharge that
/// is taxed together with the food (GST applies on subtotal + surcharge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BillTotals {
    pub subtotal: f64,
    pub discount: f64,
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
    /// Area name for dine-in orders; `None` for take-out.
    pub area: Option<String>,
    /// Whether this order's area is air-conditioned (drives 5% GST + surcharge).
    pub is_ac: bool,
    /// AC surcharge rate (fraction of subtotal) frozen when the order started.
    pub ac_rate: f64,
    /// Discount percentage (0–100) applied to the subtotal at billing.
    pub discount_percent: f64,
    pub cart: Vec<CartLine>,
    pub cart_index: usize,
    pub status: OrderStatus,
    pub customer_mobile: Option<String>,
    pub payment_mode: Option<PaymentMode>,
    pub kot_sent_count: usize,
}

impl Order {
    pub fn ac_surcharge(&self) -> f64 {
        self.ac_rate
    }

    /// GST rate per India restaurant rules: 5% where tax applies (AC areas);
    /// take-out keeps its existing 8% rate.
    pub fn gst_rate(&self) -> f64 {
        match self.service {
            Service::TakeOut => Service::TakeOut.tax_rate(),
            Service::DineIn => {
                if self.is_ac {
                    0.05
                } else {
                    Service::DineIn.tax_rate()
                }
            }
        }
    }

    pub fn totals(&self) -> BillTotals {
        let subtotal: f64 = self.cart.iter().map(|line| line.total()).sum();
        let discount = subtotal * (self.discount_percent / 100.0);
        let taxable = subtotal - discount;
        let ac_charge = taxable * self.ac_surcharge();
        let gst_rate = self.gst_rate();
        let gst = (taxable + ac_charge) * gst_rate;
        BillTotals {
            subtotal,
            discount,
            ac_charge,
            gst_rate,
            gst,
            total: taxable + ac_charge + gst,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BillSummary {
    pub id: u32,
    pub label: String,
    pub service: Service,
    pub total: f64,
    pub receipt: String,
    pub payment_mode: Option<PaymentMode>,
}

/// Aggregated end-of-day or shift sales report (Z-Report).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DailySalesSummary {
    pub date: String,
    pub total_orders: usize,
    pub dine_in_orders: usize,
    pub takeout_orders: usize,
    pub subtotal: f64,
    pub discount: f64,
    pub ac_charge: f64,
    pub tax: f64,
    pub total_sales: f64,
    pub upi_count: usize,
    pub upi_total: f64,
    pub cash_count: usize,
    pub cash_total: f64,
    pub card_count: usize,
    pub card_total: f64,
    pub person_credit_count: usize,
    pub person_credit_total: f64,
    pub have_it_on_hotel_count: usize,
    pub have_it_on_hotel_total: f64,
    pub other_count: usize,
    pub other_total: f64,
}

/// Summary item for historical bill search.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoricalBill {
    pub id: u32,
    pub label: String,
    pub service: Service,
    pub customer_mobile: String,
    pub total: f64,
    pub payment_mode: Option<PaymentMode>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::{Area, CartLine, Order, OrderStatus, Service, CLEANING_MINUTES};

    fn order(service: Service, area: Option<&str>, is_ac: bool, ac_rate: f64) -> Order {
        Order {
            id: 1,
            label: "Test".to_string(),
            service,
            table_number: None,
            area: area.map(|a| a.to_string()),
            is_ac,
            ac_rate,
            discount_percent: 0.0,
            cart: vec![CartLine::new("Dal Makhani", 100.0, 2)],
            cart_index: 0,
            status: OrderStatus::Ordering,
            customer_mobile: None,
            payment_mode: None,
            kot_sent_count: 0,
        }
    }

    #[test]
    fn cart_line_total_uses_quantity() {
        let line = CartLine::new("Samosa", 20.0, 3);

        assert_eq!(line.total(), 60.0);
    }

    #[test]
    fn default_areas_keep_expected_capacity() {
        let total: usize = Area::defaults().iter().map(|a| a.table_count).sum();
        assert_eq!(total, 24);
        let ac = Area::defaults()
            .into_iter()
            .find(|a| a.name == "AC Dining")
            .unwrap();
        assert_eq!(ac.table_count, 6);
        assert!(ac.is_ac);
    }

    #[test]
    fn ac_rooms_add_surcharge_and_gst() {
        // ₹200 food in an AC area: 6% AC charge + 5% GST on (200 + 12).
        let totals = order(Service::DineIn, Some("AC Dining"), true, 0.06).totals();
        assert_eq!(totals.subtotal, 200.0);
        assert!((totals.ac_charge - 12.0).abs() < 1e-9);
        assert!((totals.gst - 10.6).abs() < 1e-9);
        assert!((totals.total - 222.6).abs() < 1e-9);
    }

    #[test]
    fn non_ac_dine_in_has_no_extra_charges() {
        let totals = order(Service::DineIn, Some("Main Hall"), false, 0.0).totals();
        assert_eq!(totals.ac_charge, 0.0);
        assert_eq!(totals.gst, 0.0);
        assert_eq!(totals.total, 200.0);
    }

    #[test]
    fn take_out_keeps_existing_tax_rate() {
        let totals = order(Service::TakeOut, None, false, 0.0).totals();
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
