# DineInTakeOut

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange?logo=rust)](https://www.rust-lang.org/)
[![TUI](https://img.shields.io/badge/TUI-Ratatui_0.28-blue)](https://github.com/ratatui-org/ratatui)
[![Database](https://img.shields.io/badge/Database-Turso_SQLite-teal)](https://turso.tech/)
[![Tests](https://img.shields.io/badge/Tests-18_Unit_%7C_19_Integration_Passing-brightgreen)](tests/integration_smoke.rs)
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
│ [4] Menu [Category: Main Course] │ [5] Current Bill #4 — PAID via UPI ✔                   │
│ Paneer Butter Masala    ₹180.00  │ Item                  Qty    Each    Total             │
│ Palak Paneer   [86 OUT] ₹170.00  │ Butter Naan             2   40.00    80.00             │
│ Dal Makhani             ₹140.00  │   ↳ Extra crisp                                        │
│                                  │ Paneer Butter Masala    2  180.00   360.00             │
│                                  │ ──────────────────────────────────────────────         │
│                                  │ Subtotal                             ₹440.00           │
│                                  │ GST (5.0%)                            ₹22.00           │
│                                  │ TOTAL                                ₹462.00           │
│                                  │ Payment Type                            UPI            │
├──────────────────────────────────┴────────────────────────────────────────────────────────┤
│ [6] Recent Bills [Active]                 │ [7] KOT Bills (latest 10)                     │
│ Bill #4   Main-T4    Dine-In  [UPI]  ₹462 │ KOT #2  Main-T4 (3 items)  14:32              │
│ Bill #3   TK1        Take-Out [CASH] ₹250 │ KOT #1  AC-101  (2 items)  14:15              │
├───────────────────────────────────────────┴───────────────────────────────────────────────┤
│ Notification: Generated KOT for Main-T4. Saved to database (ID #2)                        │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│ [1-7] Box · ↑↓: Select · Enter: Add/Open · p: Bill · k: KOT · z: Z-Report · ?: Help       │
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Key Highlights

- **Generalized Numbered Box Architecture (`1`–`7`)**: Every panel across the TUI is numbered: `[1]` Floor Plan, `[2]` Table Details & Jump, `[3]` Menu Search, `[4]` Menu Catalogue, `[5]` Bill & Cart, `[6]` Recent Bills, and `[7]` KOT Bills. Pressing any digit `1`–`7` jumps directly to that specific box without requiring multi-step `Tab` cycling.
- **Visual Floor Plan & Stage Lifecycle**: Track dining rooms in real time (`Ready` → `Taking order` → `Serving` → `Ready for bill` → `Bill paid` → `Cleaning` → `Ready`). Tables feature an automated 10-minute cleaning countdown or instant turnaround override (`r`).
- **Table Move, Transfer & Merge (`m`)**: Seamlessly move an active check to a free table (automatically recalculating taxes & AC charges) or merge carts into an already occupied table.
- **Interactive Table Search & Jump (`g`)**: Instantly search across all tables by number, room name, or status code (`ready`, `paid`, `dirty`) and jump directly to any order.
- **Dual-Box History & Kitchen Order Tickets (KOT) (`k`)**: The recent panel is split into two side-by-side boxes: **Recent Bills** (left) and **KOT Bills** (right). Switch between them using `←` / `→` (or `h` / `l`), navigate with `↑` / `↓`, preview in the right panel, and reprint with `p` or `Enter`. KOTs generate standard 42-column slips with duplicate indicators (`*** KITCHEN ORDER TICKET [REPRINT] ***`), persisted directly into the SQLite database and dispatched to thermal printers with zero disk file clutter.
- **Item Cooking Notes / Special Instructions (`n`)**: Attach specific guest requests ("Less spicy", "No onion/garlic") directly to cart line items, visible on receipts, KOT slips, and saved in database records.
- **Out-of-Stock / "86" Toggling (`o`)**: Toggle item availability live from the menu catalogue with prominent `[86 OUT]` indicators, preventing accidental ordering and synchronizing with SQLite & CSV.
- **Daily Sales & Shift Summary / Z-Report (`z`)**: Instant settlement reports with gross revenue, discounts, AC charges, tax collections, net revenue, and detailed payment method breakdowns, saved directly to the database with `p` and sent to thermal printers.
- **Dynamic On-Screen UPI QR Code (`q`)**: Generate standard NPCI-compliant UPI payment QR codes displayed in full high-contrast Unicode half-block characters directly on terminal screens for contact-free customer smartphone scans.
- **Historical Bill Search & Reprint (`/` or `s` in Recent Bills)**: Query past orders by Bill ID or 10-digit mobile number, preview bill breakdowns, and reprint receipts anytime.
- **Accurate Tax & Surcharge Engine**: Automatically handles standard restaurant GST rates (5% on AC dine-in, 8% on take-out, 0% on non-AC dine-in) and configurable AC room surcharges.
- **Comprehensive Payment Settle & Updates**: Settle bills or update payment types after payment with dedicated hotkeys:
  - `1` or `c`: **Cash**
  - `2` or `u`: **UPI** (PhonePe, GPay, Paytm, QR)
  - `3` or `d`: **Card** (Debit / Credit)
  - `4`: **Person Credit** (Customer ledger)
  - `5`: **Have it on Hotel** (Complimentary / House tab)
- **Embedded Turso SQLite & Automatic Daily Backups**: Orders, table statuses, cart items, customer mobile numbers, KOT slips, Z-reports, and paid bills persist automatically to `data/billing.db`. Daily snapshots are safely preserved in `data/backups/billing_YYYY-MM-DD.db`.
- **Spreadsheet-Friendly Configuration & Hotel Details**: Sourced directly from CSV files (`menu.csv`, `table.csv` / `areas.csv`, `offers.csv`, `config.csv`). Receipts prominently feature hotel name ("SHREE KRISHNA RESTAURANT"), address, phone contact, and GSTIN. Export (`e`) and import (`i`) complete restaurant setups.

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
- **19 Integration Tests**: Full terminal UI simulations using Ratatui's headless `TestBackend`, validating generalized 1–7 numbered box switching, table jump search, mobile entry, offer selection, payment type switching (UPI/Cash/Card), receipt updates, hotel receipt header details, KOT generation and database persistence, dual-box Recent Bills & KOT viewer navigation, item notes, "86" stock toggle, table moves/merges, dynamic UPI QR rendering, Z-reports in database, bill search, and automated daily backups.

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
