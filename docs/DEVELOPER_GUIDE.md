# DineInTakeOut — Developer Guide

A technical reference and architecture manual for software engineers developing, maintaining, or extending the **DineInTakeOut** codebase.

---

## Table of Contents

1. [Architecture Overview & Design Philosophy](#1-architecture-overview--design-philosophy)
2. [Source Code Directory Structure](#2-source-code-directory-structure)
3. [Domain Model & Type System (`models.rs`)](#3-domain-model--type-system-modelsrs)
4. [Application State Machine (`app.rs`)](#4-application-state-machine-apprs)
5. [Persistence Layer & Turso SQLite (`db.rs`)](#5-persistence-layer--turso-sqlite-dbrs)
6. [UI Rendering & Layout Engine (`ui/`)](#6-ui-rendering--layout-engine-ui)
7. [Receipt Generation & Printing (`receipts.rs`)](#7-receipt-generation--printing-receiptsrs)
8. [Configuration & CSV Pipeline (`config.rs`)](#8-configuration--csv-pipeline-configrs)
9. [Development Environment Setup](#9-development-environment-setup)
10. [Testing Strategy & Test Suite](#10-testing-strategy--test-suite)
11. [Code Quality, Linting & Formatting](#11-code-quality-linting--formatting)
12. [How-To: Adding New Features](#12-how-to-adding-new-features)
13. [Future Roadmap & Turso Cloud Sync](#13-future-roadmap--turso-cloud-sync)

---

## 1. Architecture Overview & Design Philosophy

DineInTakeOut is built on principles of **clean domain modeling**, **predictable unidirectional data flow**, and **zero-crash operational resilience**:

```text
               ┌────────────────────────────────────────────────┐
               │             Terminal Event Stream              │
               │            (Crossterm KeyEvents)               │
               └───────────────────────┬────────────────────────┘
                                       │
                                       ▼
┌───────────────────────────────────────────────────────────────────────────────┐
│                           Application State (App)                             │
│  - Active & Open Orders    - Physical Tables Map     - Panel Focus State      │
│  - Fuzzy Search Query      - Billing & Payment State - Live Notifications     │
└──────────────┬───────────────────────┬────────────────────────┬───────────────┘
               │                       │                        │
               ▼                       ▼                        ▼
┌────────────────────────┐ ┌────────────────────────┐ ┌─────────────────────────┐
│     Domain Logic       │ │      Persistence       │ │       TUI Render        │
│    (src/models.rs)     │ │     (src/db.rs)        │ │       (src/ui/)         │
│  - Tax calculations    │ │  - Turso SQLite Engine │ │  - Ratatui Widgets      │
│  - Totals breakdown    │ │  - Table state upsert  │ │  - Responsive layout    │
│  - Order lifecycle     │ │  - Open order recovery │ │  - Color-coded badges   │
│  - State validations   │ │  - Paid bill archives  │ │  - Modals & dialogs     │
└────────────────────────┘ └────────────────────────┘ └─────────────────────────┘
```

### Key Design Tenets
1. **Separation of Domain from I/O**: The domain layer (`models.rs`) contains pure data types and calculation functions with no dependencies on terminal crates or disk I/O.
2. **Session Persistence**: Unpaid orders, cart items, and physical table states are mirrored to the database in real time. If the terminal process terminates unexpectedly, reopening the app restores the exact shift state.
3. **Headless Testability**: The entire UI and state flow can be exercised without a physical terminal using Ratatui's headless `TestBackend`.
4. **Crash Avoidance**: Database failures gracefully downgrade to in-memory mode without crashing. Missing printers or CUPS failures do not prevent bill generation.

---

## 2. Source Code Directory Structure

```text
DineInTakeOut/
├── Cargo.toml                # Project manifest and dependencies
├── Cargo.lock                # Locked dependency tree
├── menu.csv                  # Default/editable menu catalogue
├── data/
│   └── billing.db            # Embedded Turso SQLite database (auto-created, git-ignored)
├── bills/                    # Saved plaintext receipt archives (auto-created)
├── docs/                     # Documentation suite
│   ├── ARCHITECTURE.md       # High-level architecture summary
│   ├── CONFIGURATION.md      # CSV schema reference
│   ├── DATABASE.md           # Database schema & query details
│   ├── DEVELOPER_GUIDE.md    # This technical manual
│   ├── KEYBINDINGS.md        # Exhaustive keyboard reference
│   └── USER_GUIDE.md         # End-user operational manual
├── src/
│   ├── main.rs               # Terminal initialization, raw mode, event loop
│   ├── lib.rs                # Library entry point exposing core modules
│   ├── models.rs             # Domain structs, enums, financial calculations
│   ├── app.rs                # Central state container, key handlers, business logic
│   ├── db.rs                 # Embedded Turso SQLite persistence layer
│   ├── receipts.rs           # Receipt layout formatting & CUPS integration
│   ├── config.rs             # Default fixtures, CSV import/export engines
│   └── ui/                   # Ratatui rendering components
│       ├── mod.rs            # Root layout orchestrator & responsive constraint logic
│       ├── floor_plan.rs     # Dining area tabs & table card grid
│       ├── table_info.rs     # Table details panel & interactive table search/jump
│       ├── search.rs         # Fuzzy menu search input
│       ├── menu.rs           # Menu catalogue table
│       ├── bill.rs           # Active cart lines & financial breakdown
│       ├── recent_bills.rs   # Recent completed bills panel
│       ├── modals.rs         # Mobile entry, offer pick, and payment mode popups
│       ├── notifications.rs  # Status message banner
│       └── footer.rs         # Contextual keybinding helper bar
└── tests/
    └── integration_smoke.rs  # End-to-end integration test suite
```

---

## 3. Domain Model & Type System (`models.rs`)

### Enums
- **`Service`**: `DineIn` or `TakeOut`. Determines whether dining table rules and AC surcharges apply. Take-out orders always use an 8% GST rate.
- **`Focus`**: Current UI keyboard focus:
  `Search`, `Menu`, `Cart`, `Tables`, `RecentBills`, `MobileEntry`, `PaymentMode`, `OfferSelect`, `TableJump`, `DailyReport`, `BillSearch`, `UpiQr`, `TableMove`, `ItemNote`.
- **`OrderStatus`**: `Ordering` → `Serving` → `BillRequested` → `Paid`.
- **`TableStatus`**: `Ready` (Green), `Ordering` (Yellow), `Serving` (Blue), `BillRequested` (Cyan), `Paid` (Magenta), `Dirty` (Red).
- **`PaymentMode`**: `Cash`, `Upi`, `Card`, `PersonCredit`, `HaveItOnHotel`.
  - `label()`: Returns uppercase database identifier (`"CASH"`, `"UPI"`, `"CARD"`, etc.).
  - `display()`: Returns user-facing string (`"Cash"`, `"UPI"`, `"Card"`, etc.).
  - `parse(str)`: Reconstructs enum from string.

### Core Structs
- **`MenuItem`**:
  ```rust
  pub struct MenuItem {
      pub name: String,
      pub category: String,
      pub unit: String,
      pub price: f64,
      pub is_available: bool, // Toggled via 'o' ("86" out-of-stock)
  }
  ```
- **`CartLine`**:
  ```rust
  pub struct CartLine {
      pub name: String,
      pub unit_price: f64,
      pub qty: usize,
      pub note: Option<String>, // Attached via 'n'
  }
  ```
- **`Order`**:
  ```rust
  pub struct Order {
      pub id: u32,
      pub label: String,
      pub service: Service,
      pub table_number: Option<usize>,
      pub area: Option<String>,
      pub is_ac: bool,
      pub ac_rate: f64,
      pub discount_percent: f64,
      pub cart: Vec<CartLine>,
      pub cart_index: usize,
      pub status: OrderStatus,
      pub customer_mobile: Option<String>,
      pub payment_mode: Option<PaymentMode>,
      pub kot_sent_count: usize, // Incremented each time KOT is dispatched
  }
  ```
- **`DailySalesSummary`**:
  Aggregated end-of-day / shift metrics: total orders, dine-in orders, takeout orders, gross subtotal, discounts, AC charges, GST collected, net sales, and a vector of `(PaymentMode, f64, usize)` counts.
- **`HistoricalBill`**:
  Lightweight record (`id`, `label`, `service`, `customer_mobile`, `total`, `payment_mode`, `created_at`) used for fuzzy search and instant receipt reprinting.

---

## 4. Application State Machine (`app.rs`)

The `App` struct is the central state store holding all active application variables:

### Key State Fields
- `orders: Vec<Order>`: All in-flight orders (both dine-in and take-out).
- `active_order: usize`: Index of the currently displayed order.
- `physical_tables: Vec<PhysicalTable>`: Live map of all physical tables across all rooms.
- `focus: Focus`: Currently active input target, including modal states (`DailyReport`, `BillSearch`, `UpiQr`, `TableMove`, `ItemNote`).
- `close_on_payment: bool`: Flag indicating whether the `PaymentMode` modal was triggered to settle and close the table, or simply to update the payment type on an active paid bill.
- `table_input: String` & `table_search_index: usize`: Live state for the Table Search/Jump modal (`g`).
- `upi_id: String`: Configured merchant VPA for dynamic UPI QR generation.
- `bill_search_query: String`, `bill_search_results: Vec<HistoricalBill>`, `bill_search_index: usize`: State for the historical bill lookup modal (`/` or `s` in Recent Bills).
- `item_note_buffer: String`: Text editing buffer for item special instructions (`n`).
- `table_move_target_index: usize`: Selection index for table move/transfer and merge (`m`).
- `daily_report_summary: Option<DailySalesSummary>`: Cached calculation for the Z-report modal (`z`).

### Key Event Routing (`App::handle_key`)
`handle_key(&mut self, key: KeyCode) -> bool` returns `true` when the application should terminate (on `q` or `Esc` when outside modals).

Input routing follows a strict priority:
1. Modal check (`in_protected`): When in `Search`, `MobileEntry`, `PaymentMode`, `OfferSelect`, `TableJump`, `BillSearch`, `TableMove`, or `ItemNote`, global single-character shortcuts are isolated to text entry.
2. Help overlay toggle (`?`).
3. Order cycle navigation (`[` and `]`).
4. Panel-specific key matching based on `self.focus`.

---

## 5. Persistence Layer & Turso SQLite (`db.rs`)

DineInTakeOut uses the **Turso embedded engine**, a modern Rust-native implementation compatible with SQLite.

### Automated Backups on Boot
During database initialization in `Database::open_local()`, `create_daily_backup()` automatically creates `data/backups/billing_YYYY-MM-DD.db` if today's snapshot does not yet exist.

### Tokio Async Bridge
`Database` encapsulates an internal Tokio single-threaded runtime (`tokio::runtime::Runtime`) and a `tokio::sync::Mutex<Connection>`. Public methods on `Database` expose a clean, synchronous API (`save_paid_order(...) -> Result<(), String>`) by executing futures internally via `self.rt.block_on(...)`.

### Database Tables & Migrations
1. `menu_items`: Menu catalogue items (`name`, `category`, `unit`, `price`, `is_available`).
2. `orders`: Historical paid orders archive.
3. `order_items`: Line items belonging to archived paid orders (`notes`).
4. `open_orders`: Snapshot of currently active orders.
5. `open_order_items`: Line items belonging to active orders (`notes`).
6. `physical_tables`: Table state, linked order ID, and cleaning timestamp.
7. `areas`: Configured dining rooms, AC flags, and capacities.
8. `offers`: Promotional discount names and percentages.
9. `settings`: Application configuration key-value pairs (`GSTNumber`, `AcRate`, `UpiId`).

Key specialized queries:
- `get_daily_sales_summary(date_prefix)`: Aggregates day/shift totals and payment method counts.
- `search_bills(query)`: Matches against Bill ID or customer mobile number.
- `update_menu_item_availability(name, available)`: Persists "86" out-of-stock toggles.

For detailed schemas and SQL queries, see [`docs/DATABASE.md`](DATABASE.md).

---

## 6. UI Rendering & Layout Engine (`ui/`)

The UI is rendered using **Ratatui 0.28+** with layout recalculations executed on every render cycle.

### Responsive Constraints (`ui/mod.rs`)
The root layout calculates vertical constraints dynamically based on terminal height:
- **Standard Mode (Height >= 33 rows)**:
  - Top Row (Tabs & Table Info): Fixed height based on area count.
  - Search Area: 3 rows.
  - Body (Menu & Cart): Flexible fill.
  - Recent Bills: 7 rows.
  - Footer: 1 row.
  - Notifications: 3 rows.
- **Compact Mode (Height < 33 rows)**:
  - Recent bills are hidden (`Constraint::Length(0)`).
  - Notifications are compressed to 2 rows.
  - Table info width is capped at `34.min(f.area().width / 2)`.

### Modal Views (`ui/modals.rs`)
- `render_daily_report`: Formatted Z-Report table with sales breakdown and payment metrics.
- `render_bill_search`: Filtered historical order table with customer mobile and reprint prompt.
- `render_upi_qr`: High-contrast Unicode half-block QR display with VPA and total amount.
- `render_table_move`: Table destination selector distinguishing `[MOVE]` (free table) and `[MERGE]` (occupied table).
- `render_item_note`: Live text buffer for cooking instructions.
- All modal dialogs use `Clear` widgets before drawing borders to prevent background content bleed-through.

---

## 7. Receipt Generation, KOT & Printing (`receipts.rs`)

### 42-Column Thermal Formatting
Receipts are formatted using fixed-width text matching standard 80mm thermal paper:
- Width: Exactly 42 characters.
- Centered restaurant name and header lines.
- Dynamic payment mode header (`Payment: UPI`).
- Line items with quantity, unit price, right-aligned line totals, and kitchen notes (`↳ <note>`).
- Financial breakdown with right-aligned subtotal, discount, AC surcharge, GST, total, and settlement line (`Paid via                    UPI`).

### Kitchen Order Ticket (KOT)
`render_kot(order, is_reprint)` generates kitchen-focused 42-column slips:
- Header indicates `*** KITCHEN ORDER TICKET (KOT) ***` or `*** KITCHEN ORDER TICKET [REPRINT] ***`.
- Omits prices and taxes to streamline food preparation.
- Prints special instructions clearly beneath each dish item.
- Persisted directly into the Turso SQLite `kots` table, immediately updating the in-app dual-box **KOT Bills** log without writing `.txt` files to disk.

### Dynamic Unicode Half-Block QR Code
`generate_upi_qr_blocks(uri)` converts an NPCI UPI URI string into high-contrast terminal rows using UTF-8 half-block characters (`▀`, `▄`, `█`, ` `), allowing scanability directly off computer screens without specialized graphical windows.

### CUPS Printing Integration
`print_receipt_text(text)` attempts to spawn `lp` via `std::process::Command`. If `lp` is not in `$PATH` or returns a non-zero exit code, the error is logged without failing the billing transaction. Customer bill receipts are saved to `bills/`, while KOT slips and Z-reports are persisted directly to Turso SQLite database tables (`kots` and `z_reports`).

---

## 8. Configuration & CSV Pipeline (`config.rs`)

Configuration is managed as spreadsheet-compatible CSV files:
- **`export_bundle(dir, items, areas, offers, gst, ac_rate)`**: Exports `menu.csv`, `areas.csv`, `offers.csv`, and `config.csv`.
- **`import_bundle(dir)`**: Parses and validates all four files with strict error checking (e.g. non-empty names, positive prices, valid booleans).
- **Default Seeding**: If the database is completely empty on boot, `default_menu()` populates standard North Indian restaurant items.

---

## 9. Development Environment Setup

### Prerequisites
- **Rust Toolchain**: Rust 1.75+ (stable). Install via [rustup](https://rustup.rs):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **SQLite CLI (Optional)**: Useful for querying `data/billing.db` directly:
  ```bash
  # Arch / CachyOS
  sudo pacman -S sqlite
  # Ubuntu / Debian
  sudo apt install sqlite3
  ```

### Build & Run Commands
```bash
# Clone repository
git clone https://github.com/itsvrushabh/DineInTakeOut.git
cd DineInTakeOut

# Check compilation
cargo check

# Build debug binary
cargo build

# Run application locally
cargo run

# Build optimized release binary
cargo build --release
```

---

## 10. Testing Strategy & Test Suite

The test suite provides comprehensive coverage across unit, persistence, and integration layers (33 total automated tests):

### 1. Unit Tests (`src/models.rs`, `src/config.rs`)
Test pure domain calculations:
```bash
cargo test models::
cargo test config::
```
Verifies tax rates, AC surcharge math, quantity adjustments, CSV serialization/deserialization, and menu integrity.

### 2. Embedded Database Tests (`src/db.rs`)
Tests use isolated in-memory or temporary databases via `Database::open_for_tests()`:
```bash
cargo test db::
```
Verifies table state persistence, open order recovery across restarts, paid order immutability, bill numbering, daily sales summary calculations, item note roundtrips, and bill search queries.

### 3. Headless TUI Integration Tests (`tests/integration_smoke.rs`)
Using Ratatui's `TestBackend`, 16 comprehensive integration tests run virtual terminal sessions:
```bash
cargo test --test integration_smoke
```
Covers:
- Boot and clean quit.
- End-to-end dine-in lifecycle (open → order → advance → bill → pay UPI → close → dirty).
- Take-out order search and cart quantity bounds.
- Table search and jump modal interactions (`g`).
- Payment type updates across active bill, receipt text, recent bills, and database.
- Compact vs standard terminal rendering.
- Kitchen Order Tickets (KOT) generation, printing, and duplicate/reprint markers.
- Item cooking notes attachment, cart display (`↳ <note>`), and receipt rendering.
- Item stock availability ("86") toggling, badge rendering, and cart addition prevention.
- Table move/transfer to free tables (with automatic tax adjustment) and merging into occupied tables.
- Dynamic on-screen UPI QR code generation and modal display.
- Daily Sales Summary (Z-Report) aggregation and text report export.
- Historical bill search by ID / mobile number and receipt reprinting.
- Automated daily database backup snapshot creation.

### Run All Tests
```bash
cargo test
```

---

## 11. Code Quality, Linting & Formatting

Strict formatting and linting standards are enforced:

```bash
# Format check
cargo fmt --check

# Auto-format code
cargo fmt

# Clippy lint check (all targets)
cargo clippy --all-targets -- -D warnings
```

### Test Coverage with LLVM-Cov
```bash
# Install tool
cargo install cargo-llvm-cov

# Generate HTML coverage report
cargo llvm-cov --all-features --workspace --html
```

---

## 12. How-To: Adding New Features

### Example: Adding a New Payment Mode
1. **`src/models.rs`**:
   - Add variant to `enum PaymentMode` (e.g. `Crypto`).
   - Add to `PaymentMode::all()`.
   - Update `PaymentMode::label()` (e.g. `"CRYPTO"`).
   - Update `PaymentMode::display()` (e.g. `"Crypto"`).
2. **`src/ui/modals.rs`**:
   - Update `render_payment_mode` to add a shortcut key hint (e.g. `[6 / k]`).
3. **`src/app.rs`**:
   - In `handle_key` for `Focus::PaymentMode`, map the shortcut key:
     ```rust
     KeyCode::Char('k') | KeyCode::Char('K') => self.select_payment_mode(PaymentMode::Crypto),
     ```
4. **`tests/integration_smoke.rs`**:
   - Add a test asserting that settling via the new mode reflects on the receipt and database.

---

## 13. Future Roadmap & Turso Cloud Sync

Because DineInTakeOut uses the Turso engine, multi-device cloud synchronization can be enabled:
- **Cloud Replicas**: Multiple billing terminals (e.g. POS counter + handheld waiter tablets) can sync to a shared Turso Cloud database.
- **Offline Resilience**: Turso embedded replicas continue reading and writing locally when the internet drops, automatically syncing upstream when connectivity returns.
- **Feature Flag**: Enable the `sync` feature in `Cargo.toml` without altering table schemas or application state logic.
