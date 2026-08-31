# Architecture

The application is structured into decoupled domain, persistence, configuration, state management, and UI rendering modules.

```text
CSV / Default Fixtures ──> config.rs ──┐
                                       ▼
Turso SQLite DB ──────────> db.rs ──> app.rs ──> ui/ (Ratatui)
                                       │
                                       ├── open dine-in / take-out orders
                                       ├── table lifecycle & cart operations
                                       └── receipt generation
                                                │
bills/bill_*.txt <── saved receipt / optional CUPS print ── receipts.rs
```

## Modules

- `src/models.rs`: Core domain types (`MenuItem`, `CartLine`, `Service`, `PaymentMode`, `Focus`, `OrderStatus`, `TableStatus`, `Area`, `PhysicalTable`, `Offer`, `BillTotals`, `Order`, `BillSummary`) and price breakdown calculation. No terminal or file-system dependencies.
- `src/config.rs`: Default menu dataset and spreadsheet-friendly CSV I/O for `menu.csv`, `areas.csv`, `offers.csv`, and `config.csv`.
- `src/receipts.rs`: Plaintext receipt rendering, `bills/` file storage, optional CUPS printing, and recent bill summary parsing.
- `src/db.rs`: Embedded Turso database engine (`data/billing.db`): menu catalogue, physical table status, unpaid open orders and cart lines (for session restoration across restarts), paid order history, areas, settings, and offers.
- `src/app.rs`: Application state (`App`), lifecycle transitions, cart actions, fuzzy search filtering, modal flows, and keyboard input routing.
- `src/ui/`: Modular Ratatui rendering components:
  - `ui/mod.rs`: Root layout and area distribution.
  - `ui/floor_plan.rs`: Floor plan area bars, table cards, take-out chips, and lifecycle legend.
  - `ui/search.rs`: Fuzzy search input box and match counters.
  - `ui/menu.rs`: Menu table with category, unit, and price columns.
  - `ui/bill.rs`: Cart lines table and non-overlapping totals breakdown.
  - `ui/notifications.rs`: Status banner (adapts to compact and standard terminal sizes).
  - `ui/recent_bills.rs`: Recent bill history panel.
  - `ui/modals.rs`: Customer mobile capture, payment mode confirmation, and discount offer selection dialogs.
  - `ui/footer.rs`: Contextual keyboard shortcut reference.
- `src/main.rs`: Minimal entry point: terminal initialization/restoration and event polling loop.

## Lifecycle

1. Open a table order or a take-out order (table → *Taking order*).
2. Add menu items and adjust quantities while the order is unpaid; `s` moves it through *Serving* and *Ready for bill*.
3. Generate the receipt (`p`): a prompt captures the customer's 10-digit mobile number (or press `Enter` on an empty prompt to skip). If any discount offers exist, a popup first picks an offer (or none) to apply. The order — with its line items, mobile number, applied discount, and the AC/GST breakdown — is saved to the paid-history table, its open-order copy is removed, the receipt (with the configured GSTIN) is printed if CUPS is available, and the table shows *Bill paid*.
4. Close the paid order (`c`): a popup confirms the mode of payment (Cash, UPI, Card, Person credit, Have it on hotel), records it against the stored bill, and sends the table to *Cleaning*; it returns to *Ready* automatically after ~10 minutes or immediately with `r`.

Configuration (areas, menu items, GSTIN, discount offers, AC surcharge rate) is managed through CSV import/export rather than an in-app editor. Press `e` in the Menu panel to export the full config bundle (`menu.csv`, `areas.csv`, `offers.csv`, `config.csv`) next to `menu.csv`; press `i` to import and apply the same four files. Import validates each file, persists everything to the embedded database, and rebuilds the in-memory table map. See `docs/CONFIGURATION.md` for the exact file formats.

Recent receipts are loaded from the five newest files in `bills/` at startup. Legacy root-level `bill_*.txt` receipts are also read for compatibility. They are deliberately read-only.
