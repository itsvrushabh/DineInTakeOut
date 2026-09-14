# DineInTakeOut

[![Rust](https://img.shields.io/badge/Rust-2021_Edition-orange?logo=rust)](https://www.rust-lang.org/)
[![TUI](https://img.shields.io/badge/TUI-Ratatui_0.28-blue)](https://github.com/ratatui-org/ratatui)
[![Database](https://img.shields.io/badge/Database-Turso_SQLite-teal)](https://turso.tech/)
[![Tests](https://img.shields.io/badge/Tests-16_Unit_%7C_11_Integration_Passing-brightgreen)](tests/integration_smoke.rs)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

A high-performance, keyboard-driven Terminal User Interface (TUI) billing, table management, and point-of-sale system engineered for dine-in restaurants and take-out counters.

Built in pure Rust using **Ratatui** and powered by an embedded **Turso SQLite engine**, DineInTakeOut combines zero-latency keyboard workflows, live dining room table lifecycle coordination, instant fuzzy search, automated Indian GST/AC-surcharge calculations, thermal receipt generation, and crash-resilient local persistence.

---

```text
┌────────────────────────────────────────────────────────┬──────────────────────────────────┐
│ Areas / Floor Plan: [1] Main Hall  [2] AC Rooms  [TK]   │ Table Details [g: Jump]          │
│ T1 [Ready]  T2 [Ordering]  T3 [Serving]  T4 [Paid: UPI]│ Table: T4 [Paid: UPI]            │
│ TK1 [Ready for Bill]                                   │ Order: #4 (Main-T4)              │
│                                                        │ Bill : ₹450.00 [UPI] (3 items)   │
├────────────────────────────────────────────────────────┴──────────────────────────────────┤
│ Search: paneer_                                               (2 matches)                 │
├──────────────────────────────────┬────────────────────────────────────────────────────────┤
│ Menu [Category: Main Course]     │ Bill #4 — PAID via UPI ✔ (Main-T4 / Dine-In)           │
│ Paneer Butter Masala    ₹180.00  │ Item                  Qty    Each    Total             │
│ Palak Paneer            ₹170.00  │ Butter Naan             2   40.00    80.00             │
│ Dal Makhani             ₹140.00  │ Paneer Butter Masala    2  180.00   360.00             │
│                                  │ ──────────────────────────────────────────────         │
│                                  │ Subtotal                             ₹440.00           │
│                                  │ GST (5.0%)                            ₹22.00           │
│                                  │ TOTAL                                ₹462.00           │
│                                  │ Payment Type                            UPI            │
├──────────────────────────────────┴────────────────────────────────────────────────────────┤
│ Recent Bills (latest 5):                                                                  │
│ Bill #4   Main-T4    Dine-In    [UPI]            ₹462.00                                  │
│ Bill #3   TK1        Take-Out   [CASH]           ₹250.00                                  │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│ Notification: Closed Main-T4 via UPI. Table 4 cleaning — auto-ready in 10 min.            │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│ Tab: Next panel · ↑↓: Select · Enter: Add/Open · p: Bill/Payment · c: Close · ?: Help    │
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Key Highlights

- **Visual Floor Plan & Stage Lifecycle**: Track dining rooms in real time (`Ready` → `Taking order` → `Serving` → `Ready for bill` → `Bill paid` → `Cleaning` → `Ready`). Tables feature an automated 10-minute cleaning countdown or instant turnaround override (`r`).
- **Interactive Table Search & Jump (`g`)**: Instantly search across all tables by number, room name, or status code (`ready`, `paid`, `dirty`) and jump directly to any order.
- **Dine-In & Take-Out Management**: Simultaneous handling of dine-in table checks and fast counter take-out queues (`t`).
- **Fast Fuzzy Item Search (`/`)**: Skim-based fuzzy search instantly filters hundreds of menu items by name or category.
- **Accurate Tax & Surcharge Engine**: Automatically handles standard restaurant GST rates (5% on AC dine-in, 8% on take-out, 0% on non-AC dine-in) and configurable AC room surcharges.
- **Comprehensive Payment Settle & Updates**: Settle bills or update payment types after payment with dedicated hotkeys:
  - `1` or `c`: **Cash**
  - `2` or `u`: **UPI** (PhonePe, GPay, Paytm, QR)
  - `3` or `d`: **Card** (Debit / Credit)
  - `4`: **Person Credit** (Customer ledger)
  - `5`: **Have it on Hotel** (Complimentary / House tab)
- **Embedded Turso SQLite Persistence**: Orders, table statuses, cart items, customer mobile numbers, and paid bills persist automatically to `data/billing.db`. Recovers shift state seamlessly across restarts or power outages.
- **Thermal Printing & Receipt Storage**: Auto-generates standard 42-column 80mm thermal receipts in `bills/` and prints via CUPS (`lp`).
- **Spreadsheet-Friendly Configuration**: Export (`e`) and import (`i`) complete restaurant setups (`menu.csv`, `areas.csv`, `offers.csv`, `config.csv`) using Excel, LibreOffice, or Google Sheets.

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
| `Tab` / `BackTab` | Cycle focus forward / backward across panels |
| `←` / `→` or `h` / `l` | Select table within current dining area |
| `1`–`9` | Jump directly to a dining area tab |
| `Enter` | Open order on table / Switch to table order / Add item to cart |
| `s` | Advance order stage (Taking order → Serving → Ready for bill) |
| `g` | Open **Search / Jump Table** popup |
| `t` | Create a new **Take-Out** order |
| `[` / `]` | Cycle between all active open orders |
| `/` | Focus **Menu Search** bar |
| `+` / `-` | Increase / decrease quantity of selected cart line |
| `x` / `Delete` | Remove selected item line from cart |
| `p` (or `b`) | **Billing / Payment**: Generate bill, or update payment mode (UPI, Cash, Card) if already paid |
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
| [**Database Reference**](docs/DATABASE.md) | Complete schema definitions for all 9 Turso SQLite tables, SQL queries, indexing, and backup guidelines. |
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
- **16 Unit Tests**: Domain financial math, AC surcharges, GST rules, cart operations, CSV serialization/deserialization, and isolated database round-trips.
- **11 Integration Tests**: Full terminal UI simulations using Ratatui's headless `TestBackend`, validating table jump search, mobile entry, offer selection, payment type switching (UPI/Cash/Card), receipt updates, and database consistency.

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
