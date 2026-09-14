# DineInTakeOut

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange?logo=rust)](https://www.rust-lang.org/)
[![TUI](https://img.shields.io/badge/TUI-Ratatui_0.28-blue)](https://github.com/ratatui-org/ratatui)
[![Database](https://img.shields.io/badge/Database-Turso_SQLite-teal)](https://turso.tech/)
[![Tests](https://img.shields.io/badge/Tests-18_Unit_%7C_27_Integration_Passing-brightgreen)](tests/integration_smoke.rs)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

A high-performance, keyboard-driven Terminal User Interface (TUI) billing, table management, and point-of-sale system engineered for dine-in restaurants and take-out counters.

Built in pure Rust using **Ratatui** and powered by an embedded **Turso SQLite engine**, DineInTakeOut combines zero-latency keyboard workflows, live dining room table lifecycle coordination, instant fuzzy search, automated Indian GST/AC-surcharge calculations, thermal receipt generation, and crash-resilient local persistence.

---

```text
┌────────────────────────────────────────────────────────┬──────────────────────────────────┐
│ [1] Floor Plan: Main Hall  AC Rooms  [TK]              │ [2] Table Details [g: Jump]      │
│ T1 [Ready]  T2 [Ordering]  T3 [Serving]  T4 [Paid: UPI]│ Table: T4 [Paid: UPI]            │
│ TK1 [Ready for Bill]                                   │ Order: #4 (Main-T4)              │
│                                                        │ Bill : ₹450.00 [UPI] (3 items)   │
├────────────────────────────────────────────────────────┴──────────────────────────────────┤
│ [3] Search: paneer_                                           (2 matches)                 │
├──────────────────────────────────┬────────────────────────────────────────────────────────┤
│ [4] Menu [ ◀ [Main Course] Breads ▶ ] │ [5] Current Bill #4 — PAID via UPI ✔               │
│ Paneer Butter Masala    ₹180.00  │ Item                  Qty    Each    Total             │
│ Palak Paneer   [86 OUT] ₹170.00  │ Butter Naan [KOT]       2   40.00    80.00             │
│ Dal Makhani             ₹140.00  │   ↳ Extra crisp                                        │
│                                  │ Paneer Butter [NC]      1  180.00     0.00             │
│                                  │ ──────────────────────────────────────────────         │
│                                  │ Subtotal                              ₹80.00           │
│                                  │ GST (5.0%)                             ₹4.00           │
│                                  │ TOTAL                                 ₹84.00           │
│                                  │ Payment Type                            UPI            │
├──────────────────────────────────┴────────────────────────────────────────────────────────┤
│ [6] Recent Bills [Active]                 │ [7] KOT Bills (latest 10)                     │
│ Bill #4   Main-T4    Dine-In  [UPI]   ₹84 │ KOT #2  Main-T4 (DELTA)    14:32              │
│ Bill #3   TK1        Take-Out [SPLIT] ₹250│ KOT #1  AC-101  (3 items)  14:15              │
├───────────────────────────────────────────┴───────────────────────────────────────────────┤
│ Notification: Generated KOT (DELTA) for Main-T4. Saved to database (ID #2)                │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│ [1-7] Box · ←/→: Cat/Tab · Enter: Add · c: NC · d: Disc · k/K: KOT · A: Analytics · ?: Help│
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Key Highlights

- **Generalized Numbered Box Architecture (`1`–`7`)**: Every panel across the TUI is numbered: `[1]` Floor Plan, `[2]` Table Details & Jump, `[3]` Menu Search, `[4]` Menu Catalogue, `[5]` Bill & Cart, `[6]` Recent Bills, and `[7]` KOT Bills. Pressing any digit `1`–`7` jumps directly to that specific box without requiring multi-step `Tab` cycling.
- **Incremental / Delta KOT Dispatch (`k`) & Force Reprint (`Shift+K`)**: Tracks `kot_sent_qty` per cart item. When adding items to an active order or increasing quantities after an initial KOT has fired, pressing `k` prints an **ADD-ON / DELTA** ticket containing *only* the new items and incremental quantities. Press `Shift+K` to force a full reprint of all order items.
- **Kitchen Display System (KDS) Fullscreen Monitor (`K` / `F7`)**: Real-time kitchen monitor with color-coded preparation timers (`<10m` Green, `10-20m` Amber, `>20m` Urgent Red) and ticket bumping (`PENDING` ➔ `PREPARING` ➔ `READY` ➔ `SERVED`) via `Space` / `Enter`.
- **Split Payment Tender (`4` / `s`)**: Settle checks across mixed payment modes (Cash + UPI + Card) with real-time balance calculations and one-key auto-fill (`a`).
- **Menu Category Fast-Filter Pill Bar in Box `[4]`**: Horizontally browse and filter dishes by category (`◀ [ ALL ] [ Starters ] [ Mains ] ▶`) with `←` / `→` (or `h` / `l`).
- **Complimentary (NC) Items (`c`) & Item Discounts (`d`)**: Tag lines as non-chargeable (`[NC]`, ₹0.00 subtotal) or cycle discounts (10%, 20%, 50%, 100%) in Cart Box `[5]`. Press `Shift+C` to clear cart.
- **Customer CRM & Visit History**: Instant VIP member banner on mobile entry modal showing lifetime visits, total spend, and favorite dishes.
- **End-of-Day Sales & Tax Analytics Modal (`A` / `F8`)**: Interactive business dashboard showing Gross & Net Revenue, CGST / SGST breakdown, Tender Distribution table, and Top 5 best-selling items with ASCII volume trend bars.
- **Direct ESC/POS Thermal Printer Driver**: ESC/POS byte generator supporting drawer kicks on Cash/Split, paper cut, and peripheral routing (`lpr`, character device `/dev/usb/lp0`, network TCP socket, or simulation) via `config.csv`.
- **Automated Backup Rotation Policy**: Automatically purges SQLite backups older than 30 days during daily snapshot routines.
- **Visual Floor Plan & Stage Lifecycle**: Track dining rooms in real time (`Ready` → `Taking order` → `Serving` → `Ready for bill` → `Bill paid` → `Cleaning` → `Ready`). Tables feature an automated 10-minute cleaning countdown or instant turnaround override (`r`).
- **Table Move, Transfer & Merge (`m`)**: Seamlessly move an active check to a free table (automatically recalculating taxes & AC charges) or merge carts into an already occupied table.
- **Interactive Table Search & Jump (`g`)**: Instantly search across all tables by number, room name, or status code (`ready`, `paid`, `dirty`) and jump directly to any order.
- **Dual-Box History & Kitchen Order Tickets (KOT) (`k`)**: Side-by-side **Recent Bills** (left) and **KOT Bills** (right) with instant reprinting (`p` / `Enter`) and full SQLite ticket archiving.
- **Item Cooking Notes / Special Instructions (`n`)**: Attach specific guest requests ("Less spicy", "No onion/garlic") directly to cart line items, visible on receipts, KOT slips, and saved in database records.
- **Out-of-Stock / "86" Toggling (`o`)**: Live availability toggle with prominent `[86 OUT]` indicators.
- **Daily Sales & Shift Summary / Z-Report (`z`)**: Instant settlement reports with gross revenue, discounts, AC charges, tax collections, net revenue, and detailed payment method breakdowns.
- **Dynamic On-Screen UPI QR Code (`q`)**: High-contrast Unicode half-block QR codes for contact-free smartphone scans.
- **Spreadsheet-Friendly Configuration & Hotel Details**: Sourced directly from CSV files (`menu.csv`, `table.csv` / `areas.csv`, `offers.csv`, `config.csv`). Receipts prominently feature hotel name ("SHREE KRISHNA RESTAURANT"), address, phone contact, and GSTIN.

---

## Quick Start

### Prerequisites
- **Rust Toolchain**: Rust 1.75+ (stable). Install via [rustup.rs](https://rustup.rs).
- A terminal emulator supporting Unicode and ANSI color (e.g. Alacritty, Kitty, Foot, Windows Terminal, GNOME Terminal).

### Run Locally
```bash
# Clone the repository
git clone https://github.com/itsvrushabh/DineInTakeOut.git
cd DineInTakeOut

# Run immediately (seeds database automatically on first boot)
cargo run
```

### Build Release Binary
```bash
cargo build --release
./target/release/dinein-takeout-billing
```

---

## Common Keyboard Shortcuts

| Shortcut | Description |
| :--- | :--- |
| `1`–`7` | **Jump Directly to Box**: `[1]` Tables, `[2]` Jump, `[3]` Search, `[4]` Menu, `[5]` Cart, `[6]` Bills, `[7]` KOTs |
| `Tab` / `BackTab` | Cycle focus forward / backward across panels |
| `←` / `→` or `h` / `l` | Select table within current dining area / Toggle Recent Bills vs KOT Bills |
| `Enter` | Open order on table / Switch to table order / Add item to cart |
| `s` | Advance order stage (Taking order → Serving → Ready for bill) |
| `g` (or `2`) | Open **Search / Jump Table** popup |
| `m` | **Move / Transfer / Merge Table**: Transfer to clean table or merge into occupied table |
| `t` | Create a new **Take-Out** order |
| `[` / `]` | Cycle between all active open orders |
| `/` (or `3`) | Focus **Menu Search** bar |
| `o` | Toggle item **Out-of-Stock ("86")** status in Menu catalogue |
| `+` / `-` | Increase / decrease quantity of selected cart line |
| `x` / `Delete` | Remove selected item line from cart |
| `n` | Add / edit **Item Cooking Note** on selected cart item |
| `k` | Generate and print **Kitchen Order Ticket (KOT)** |
| `p` (or `b`) | **Billing / Payment**: Generate bill, or update payment mode (UPI, Cash, Card) if already paid |
| `q` | Display **Dynamic UPI QR Code** during payment flow |
| `z` | Open **Daily Sales Summary (Z-Report)** & Settlement breakdown |
| `/` or `s` | **Search & Reprint Past Bills** (when focusing Recent Bills panel) |
| `c` | Clear cart (in Menu) / Settle & close order (in Tables) |
| `r` | Override cleaning timer and mark table as **Ready** |
| `e` / `i` | **Export / Import** full CSV configuration bundle |
| `?` | Toggle keybinding help overlay |
| `q` / `Esc` | Cancel dialog / Exit application |

*See [`docs/KEYBINDINGS.md`](docs/KEYBINDINGS.md) for the complete reference.*

---

## Documentation Directory

The project includes an extensive, modular documentation library located in the [`docs/`](docs/) directory:

| Document | Description |
| :--- | :--- |
| [**User Guide**](docs/USER_GUIDE.md) | Step-by-step operational manual for managers, cashiers, and waitstaff. Covers all workflows, payment flows, and common scenarios. |
| [**Developer Guide**](docs/DEVELOPER_GUIDE.md) | Deep technical architecture, state machine details, component breakdowns, coding conventions, and instructions for adding new features. |
| [**Database Reference**](docs/DATABASE.md) | Complete schema definitions for all 11 Turso SQLite tables, SQL queries, indexing, and backup guidelines. |
| [**Architecture & Design**](docs/ARCHITECTURE.md) | High-level data flow diagrams, module boundaries, and lifecycle stages. |
| [**Keybindings Reference**](docs/KEYBINDINGS.md) | Exhaustive keyboard shortcut tables and modal controls. |
| [**Configuration via CSV**](docs/CONFIGURATION.md) | Specifications for `menu.csv`, `areas.csv`, `offers.csv`, and `config.csv`. |

---

## Quality Assurance & Testing

DineInTakeOut is built with rigorous automated verification:

```bash
# Run all unit and integration tests
cargo test

# Check code formatting
cargo fmt --check

# Strict Clippy lint check (all targets)
cargo clippy --all-targets -- -D warnings
```

### Test Coverage Highlights
- **18 Unit Tests**: Domain financial math, AC surcharges, GST rules, cart operations, table CSV & config CSV round-trips, empty fallback menu verification, isolated database round-trips, and daily sales aggregation queries.
- **27 Integration Tests**: Full terminal UI simulations using Ratatui's headless `TestBackend`, validating generalized 1–7 numbered box switching, table jump search, mobile entry, offer selection, payment type switching (UPI/Cash/Card/Split), receipt updates, hotel receipt header details, incremental/delta KOT generation, full KOT reprints, Kitchen Display System (KDS) prep timers and status bumping, split tender settlement with auto-fill, category tab filtering, complimentary (NC) items and cycling discounts, customer CRM VIP profiles, sales & tax analytics modal, dual-box Recent Bills & KOT viewer navigation, item notes, "86" stock toggle, table moves/merges, dynamic UPI QR rendering, Z-reports in database, bill search, and automated 30-day backup retention purge.

To run code coverage with LLVM tools:
```bash
cargo install cargo-llvm-cov
cargo llvm-cov --all-features --workspace --html
```

---

## Tech Stack

- **Language**: [Rust 2021 Edition](https://www.rust-lang.org/)
- **Terminal UI**: [Ratatui](https://github.com/ratatui-org/ratatui) + [Crossterm](https://github.com/crossterm-rs/crossterm)
- **Database**: [Turso Engine](https://turso.tech/) (libsql / SQLite compatible) + [Tokio](https://tokio.rs/)
- **Fuzzy Matching**: [Skim Matcher V2](https://github.com/lotabout/fuzzy-matcher)
- **Data Serialization**: [csv](https://docs.rs/csv) + [serde](https://serde.rs/)
- **Time & Dates**: [chrono](https://docs.rs/chrono)

---

## License

This project is licensed under the [MIT License](LICENSE).
