# DineInTakeOut

A terminal-based billing application for a small restaurant. It supports dine-in tables, take-out orders, fuzzy menu search, receipt generation, and read-only review of the five most recent saved bills.

## Run locally

Requirements: Rust stable and a terminal with Unicode support.

```bash
cargo run
```

The app reads its menu from `menu.csv`. If the file is missing or invalid, it uses the built-in default menu. Receipts are saved under `bills/` as `bill_<number>_<timestamp>.txt`; printing through CUPS (`lp`) is attempted when available.

## Project layout

- `src/main.rs` — application state, order workflow, keyboard event handling, and terminal rendering.
- `src/models.rs` — domain model shared by the application: menu, cart, service, table, order, and receipt types.
- `src/receipts.rs` — receipt rendering, printing, persistence, and recent-receipt loading.
- `menu.csv` — editable menu catalogue.
- `docs/ARCHITECTURE.md` — module boundaries and data flow.
- `docs/KEYBINDINGS.md` — complete keyboard reference.

## Rules

- A table or take-out order must have a generated bill before it can be closed.
- Paid orders are immutable, protecting the saved receipt from later cart changes.
- Previous receipts are read-only and can be reviewed from the Recent bills panel.

## Quality checks

```bash
cargo fmt --check
cargo check
```
