# DineInTakeOut

A terminal-based billing application for a small restaurant. It supports dine-in tables, take-out orders, fuzzy menu search, receipt generation with customer mobile-number capture at billing time, configurable dining areas / menu / GSTIN / discount offers / AC charges, and read-only review of the five most recent saved bills. The menu, every table's live status, unpaid in-progress orders, and all paid orders (with the customer mobile number) are stored in an embedded Turso database (SQLite-compatible) at `data/billing.db`.

## Run locally

Requirements: Rust stable and a terminal with Unicode support.

```bash
cargo run
```

The app stores its menu, table states, unpaid orders, and paid-order history in an **embedded Turso database** — a ground-up Rust rewrite of SQLite — at `data/billing.db` (created automatically; git-ignored). The file is standard SQLite format, so you can inspect it with `sqlite3 data/billing.db` or any SQLite browser.

```bash
sqlite3 data/billing.db ".tables"
```

Cloud sync with Turso can be enabled later through the crate's `sync` feature without any schema or code changes.

The menu is read from `menu.csv` only to seed an empty database on first run; afterwards the database is the source of truth (CSV import/export still works from within the app). Receipts are saved under `bills/` as `bill_<number>_<timestamp>.txt`; printing through CUPS (`lp`) is attempted when available.

## Project layout

- `src/main.rs` — application state, order workflow, keyboard event handling, and terminal rendering.
- `src/models.rs` — domain model shared by the application: menu, cart, service, table, order, and receipt types.
- `src/receipts.rs` — receipt rendering, printing, persistence, and recent-receipt loading.
- `src/db.rs` — embedded Turso (SQLite-compatible) persistence layer: menu, tables, open orders, paid history.
- `menu.csv` — editable menu catalogue (seeds an empty database on first run).
- `docs/ARCHITECTURE.md` — module boundaries and data flow.
- `docs/KEYBINDINGS.md` — complete keyboard reference.

## Rules

- A table or take-out order must have a generated bill before it can be closed.
- Table lifecycle: Ready → Taking order → Serving → Ready for bill → Bill paid → Cleaning (~10 min) → Ready. The `s` key advances a stage; closing a paid order starts cleaning, which auto-clears after ~10 minutes (or immediately with `r`).
- Billing (`p`) opens a prompt for the customer's 10-digit mobile number; it is printed on the receipt and stored with the order. If discount offers exist, one can be applied at billing.
- Press `e` / `i` in the Menu panel to export / import the full configuration as CSV (`menu.csv`, `areas.csv`, `offers.csv`, `config.csv`): dining areas (add/remove/rename, table counts, AC flag), menu items and prices, the GSTIN printed on receipts, discount offers, and the AC surcharge rate. See `docs/CONFIGURATION.md` for the file formats.
- Closing a paid order (`c`) asks for the mode of payment (Cash, UPI, Card, Person credit, Have it on hotel) and records it against the stored bill.
- Dining areas, AC charges (default 6% surcharge + 5% GST on AC areas), and the GSTIN are all configured through CSV import/export; areas are fully dynamic.
- Paid orders are immutable, protecting the saved receipt from later cart changes.
- Previous receipts are read-only and can be reviewed from the Recent bills panel.

## Quality checks

```bash
cargo fmt --check
cargo check
```
