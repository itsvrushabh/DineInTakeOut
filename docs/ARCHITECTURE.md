# Architecture

The application is intentionally split into a small domain layer and a terminal application layer.

```text
menu.csv ──────> menu loading ──────> App state ──────> Ratatui UI
                                      │
                                      ├── open dine-in / take-out orders
                                      ├── cart and table lifecycle
                                      └── receipt generation
                                               │
bills/bill_*.txt <── saved receipt / optional CUPS print ─┘
```

## Modules

`src/models.rs` contains plain data types and their small domain helpers. It has no terminal or file-system dependencies.

`src/receipts.rs` owns the receipt file format, `bills/` storage, optional CUPS printing, and receipt-history parsing.

`src/db.rs` owns the embedded Turso database (a Rust rewrite of SQLite, standard file format): the menu catalogue, all physical table states (`tables`), unpaid in-progress orders (`open_orders` + `open_order_items`, so a restart resumes mid-shift), and all paid orders with their line items. The file lives at `data/billing.db` and is created with its schema on first run; an empty database is seeded from `menu.csv`, and imports/exports write straight into the database. Cleaning tables auto-expire back to Ready after 10 minutes based on their stored timestamp.

`src/main.rs` is the composition layer. It owns `App`, reads input, manages order state, loads the menu, tables, and open orders from the database at startup, persists every lifecycle transition, writes/prints receipts, and renders the interface.

## Lifecycle

1. Open a table order or a take-out order (table → *Taking order*).
2. Add menu items and adjust quantities while the order is unpaid; `s` moves it
   through *Serving* and *Ready for bill*.
3. Generate the receipt (`p`): a prompt captures the customer's 10-digit
   mobile number. The order — with its line items, mobile number, and the
   AC/GST breakdown — is saved to the paid-history table, its open-order copy
   is removed, the receipt is printed if CUPS is available, and the table
   shows *Bill paid*.
4. Close the paid order (`c`): a popup confirms the mode of payment (Cash,
   UPI, Card, Person credit, Have it on hotel), records it against the stored
   bill, and sends the table to *Cleaning*; it returns to *Ready*
   automatically after ~10 minutes or immediately with `r`.

Recent receipts are loaded from the five newest files in `bills/` at startup. Legacy root-level `bill_*.txt` receipts are also read for compatibility. They are deliberately read-only.
