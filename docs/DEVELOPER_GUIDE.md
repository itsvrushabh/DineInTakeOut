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
│   ├── printer.rs            # Direct ESC/POS thermal driver & peripheral routing
│   ├── receipts.rs           # Receipt layout formatting & CUPS integration
│   ├── config.rs             # Default fixtures, CSV import/export engines
│   └── ui/                   # Ratatui rendering components
│       ├── mod.rs            # Root layout orchestrator & responsive constraint logic
│       ├── floor_plan.rs     # Dining area tabs & table card grid
│       ├── table_info.rs     # Table details panel & interactive table search/jump
│       ├── search.rs         # Fuzzy menu search input
│       ├── menu.rs           # Menu catalogue table & category pill bar
│       ├── bill.rs           # Active cart lines & financial breakdown (NC, disc, KOT tags)
│       ├── recent_bills.rs   # Recent completed bills & KOT tickets panel
│       ├── kds.rs            # Kitchen Display System (KDS) live order monitor
│       ├── analytics.rs      # End-of-Day Sales & Tax Analytics dashboard
│       ├── modals.rs         # Mobile entry (with CRM), offer pick, split tender, and payment modals
│       ├── notifications.rs  # Status message banner
│       └── footer.rs         # Contextual keybinding helper bar
└── tests/
    └── integration_smoke.rs  # End-to-end integration test suite (27 integration tests)
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
  - Recent Bills & KOTs: 7 rows.
  - Footer: 1 row.
  - Notifications: 3 rows.
- **Compact Mode (Height < 33 rows)**:
  - Recent bills are hidden (`Constraint::Length(0)`).
  - Notifications are compressed to 2 rows.
  - Table info width is capped at `34.min(f.area().width / 2)`.

### Generalized Numbered Box Architecture (`1`–`7`)
Every functional container in the TUI is identified by an index number rendered in its border header:
- **`[1]` Floor Plan** (`ui/floor_plan.rs`): Tables and area lifecycle cards (`Focus::Tables`).
- **`[2]` Table Details / Jump** (`ui/table_info.rs`): Active table specs and live search input (`Focus::TableJump`).
- **`[3]` Menu Search** (`ui/search.rs`): Fuzzy item filter bar (`Focus::Search`).
- **`[4]` Menu Catalogue** (`ui/menu.rs`): Categorized dish listings with category pill bar (`◀ [ ALL ] [ Starters ] [ Mains ] ▶`) and stock availability (`Focus::Menu`).
- **`[5]` Bill & Cart** (`ui/bill.rs`): Active order cart lines, totals, complimentary (`[NC]`), discount (`[-10%]`), KOT status (`[KOT]`), and historical receipt preview (`Focus::Cart`).
- **`[6]` Recent Bills** (`ui/recent_bills.rs`): Latest settled checks log (`Focus::RecentBills`, `RecentTab::Bills`).
- **`[7]` KOT Bills** (`ui/recent_bills.rs`): Latest kitchen order tickets log (`Focus::RecentBills`, `RecentTab::Kots`).

`App::switch_to_box(box_num: u8)` handles zero-latency navigation when digit keys `1`–`7` are pressed across all non-modal views.

### Specialized Screens & Modals
- **Kitchen Display System (`ui/kds.rs`)**: Fullscreen live kitchen order monitor (`Focus::KitchenDisplay`, hotkey `K` / `F7`) with color-coded preparation timers (<10m Green, 10-20m Amber, >20m Urgent Red) and ticket status bumping (`PENDING` ➔ `PREPARING` ➔ `READY` ➔ `SERVED`) via `Space` / `Enter`.
- **Sales & Tax Analytics Dashboard (`ui/analytics.rs`)**: Full-screen modal dashboard (`Focus::Analytics`, hotkey `A` / `F8`) rendering 4 KPI cards (Net Revenue, Total Orders, GST Collected with CGST/SGST split, Discounts Given), payment tender distribution table, and top 5 best-selling items with ASCII trend volume bars.
- **Split Payment Tender Modal (`ui/modals.rs`)**: Mixed settlement modal (`Focus::SplitPayment`, hotkey `4` / `s` in payment mode) with real-time balance calculations, Cash / UPI / Card fields, and one-key balance auto-fill (`a`).
- **Customer CRM Profile in Mobile Entry (`ui/modals.rs`)**: Automatically queries customer history upon entering 4+ digits, rendering a VIP membership banner showing lifetime visits, total spend, and favorite dishes.
- **Daily Report (Z-Report)**: Formatted Z-Report table with sales breakdown and payment metrics.
- **Bill Search**: Filtered historical order table with customer mobile and reprint prompt.
- **Dynamic UPI QR Code**: High-contrast Unicode half-block QR display with VPA and total amount.
- **Table Move & Merge**: Table destination selector distinguishing `[MOVE]` (free table) and `[MERGE]` (occupied table).
- **Item Cooking Notes**: Live text buffer for kitchen preparation instructions.

---

## 7. Direct ESC/POS Thermal Printing & Routing (`printer.rs`)

The `printer` module provides a native ESC/POS thermal printing engine independent of external text file writes:
- **Command Constants**: Hardware initialization (`ESC @`), text alignment (Left, Center, Right), text bolding/double-height, paper cutting (`GS V 66 0`), and cash drawer kick (`ESC p 0 25 250`).
- **Receipt & KOT Builders**: `build_escpos_receipt` and `build_escpos_kot` generate standardized 42-column ESC/POS byte streams including item notes, complimentary badges, discounts, and delta KOT add-on headers.
- **Peripheral Routing**: Sourced from `config.csv` (`printer_mode`, `receipt_printer`, `kot_printer`, `drawer_kick_on_cash`):
  - `Lpr`: Spools directly via system CUPS (`lp -d <printer>`).
  - `Device`: Directly writes raw bytes to character device paths (e.g. `/dev/usb/lp0`, `/dev/ttyUSB0`).
  - `Network`: Streams raw bytes over TCP socket to network receipt printers (e.g. `192.168.1.100:9100`).
  - `Simulated`: Writes raw `.bin` byte dumps and formatted `.txt` files to `bills/` for testing without hardware.

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
