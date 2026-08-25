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

`src/main.rs` is the composition layer. It owns `App`, reads input, manages order state, loads the menu and saved receipts, writes/prints receipts, and renders the interface.

## Lifecycle

1. Open a table order or a take-out order.
2. Add menu items and adjust quantities while the order is unpaid.
3. Generate the receipt to mark the order paid and store it in recent-bill history.
4. Close the paid order to free its table.

Recent receipts are loaded from the five newest files in `bills/` at startup. Legacy root-level `bill_*.txt` receipts are also read for compatibility. They are deliberately read-only.
