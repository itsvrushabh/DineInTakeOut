# Architecture & System Design

The application is structured into decoupled domain, persistence, configuration, state management, and UI rendering modules.

```text
CSV / Default Fixtures ──> config.rs ──┐
                                       ▼
Turso SQLite DB ──────────> db.rs ──> app.rs ──> ui/ (Ratatui)
(with daily backups)                   │
                                       ├── open dine-in / take-out orders
                                       ├── table lifecycle, move & merge
                                       ├── table search / jump lookup
                                       ├── item cooking notes & 86 toggles
                                       ├── daily sales summary (Z-report)
                                       ├── bill search & historical reprint
                                       └── receipt, KOT & UPI QR codes
                                                │
bills/kot_*.txt      <── kitchen order ticket ──┤
bills/bill_*.txt     <── saved receipt print  ──┤── receipts.rs
bills/z_report_*.txt <── daily sales summary  ──┘
```

---

## Modules & Component Boundaries

- **`src/models.rs`**: Core domain types (`MenuItem`, `CartLine`, `Service`, `PaymentMode`, `Focus`, `OrderStatus`, `TableStatus`, `Area`, `PhysicalTable`, `Offer`, `BillTotals`, `Order`, `BillSummary`, `DailySalesSummary`, `HistoricalBill`) and price breakdown calculation. Completely free of terminal and file-system dependencies.
- **`src/config.rs`**: Default menu dataset and spreadsheet-friendly CSV I/O for `menu.csv` (with item availability), `areas.csv`, `offers.csv`, and `config.csv` (with `UpiId`).
- **`src/receipts.rs`**: Plaintext 42-column receipt rendering, Kitchen Order Ticket (KOT) formatting, daily Z-report rendering, dynamic Unicode half-block UPI QR code generator (`qrcode` crate), `bills/` file storage, and CUPS printing.
- **`src/db.rs`**: Embedded Turso database engine (`data/billing.db`): automated daily startup backups (`data/backups/`), menu catalogue, physical table status, unpaid open orders and cart lines with notes (for session restoration across restarts), paid order history, daily sales aggregation queries, and historical bill search.
- **`src/app.rs`**: Application state (`App`), lifecycle transitions, cart actions, item cooking notes, stock 86 toggling, table move and merge logic, bill search, daily sales reporting, payment mode updates, modal flows, and keyboard input routing.
- **`src/ui/`**: Modular Ratatui rendering components:
  - `ui/mod.rs`: Root layout and area distribution with adaptive compact height support.
  - `ui/floor_plan.rs`: Floor plan area bars, table cards, take-out chips, and lifecycle legend.
  - `ui/table_info.rs`: Active table details (status, order ID, bill total, payment mode badge) and interactive table search/jump input (`g`).
  - `ui/search.rs`: Fuzzy search input box and match counters.
  - `ui/menu.rs`: Menu table with category, unit, price columns, and `[86 OUT]` badges.
  - `ui/bill.rs`: Cart lines table with item cooking notes (`↳ <note>`), paid title banner, and totals breakdown.
  - `ui/notifications.rs`: Status banner (adapts to compact and standard terminal sizes).
  - `ui/recent_bills.rs`: Recent bill history panel with payment mode tags.
  - `ui/modals.rs`: Customer mobile capture, payment mode confirmation, discount offer selection, daily sales summary (Z-report), historical bill search, dynamic UPI QR display, table move/merge selector, item note buffer, and help overlay.
  - `ui/footer.rs`: Contextual keyboard shortcut reference.
- **`src/main.rs`**: Minimal entry point: terminal initialization/restoration, raw mode setup, and event polling loop.

---

## Operational Data Flow & Lifecycle

1. **Table Initiation**: Open a table order (`Enter`) or take-out order (`t`). Dine-in tables enter *Taking order*.
2. **Ordering & Notes**: Add menu items (out-of-stock items toggled with `o` are blocked). Highlight cart items and press `n` to attach custom cooking notes. Use `s` to advance through *Serving* and *Ready for bill*.
3. **Kitchen Dispatch (`k`)**: Press `k` to dispatch a Kitchen Order Ticket (KOT) to `bills/` and the kitchen printer. Subsequent prints are marked `[REPRINT]`.
4. **Table Move & Merge (`m`)**: Move active checks to any free table (recalculating taxes and AC surcharges) or merge into an already seated table.
5. **Table Jump (`g`)**: Instantly search across all tables in all areas by number, name, or status, jumping focus directly to the target order.
6. **Billing (`p` / `b`)**:
   - Captures customer's 10-digit mobile number (optional, press `Enter` on empty to skip).
   - Prompts for an applicable discount offer if offers exist.
   - Saves bill with item lines, kitchen notes, tax, and discount breakdown to the database.
   - Generates text receipt in `bills/` and prints via CUPS if configured.
   - Status changes to *Bill paid*.
7. **Payment & Dynamic UPI QR (`q`)**:
   - If guest pays via UPI, Cash, or Card, pressing `p` or `b` opens the payment modal.
   - Cashiers can press `q` to display an on-screen high-contrast UPI QR code for direct scanning with PhonePe, GPay, Paytm, or BHIM.
   - Selecting a payment type updates active bill, totals, recent bills (`[UPI]`), table details, and SQLite database.
8. **Settlement & Cleaning (`c`)**:
   - Confirms final payment mode, archives order, and transitions table to *Cleaning*.
   - A 10-minute auto-ready countdown begins, turning *Ready* automatically, or immediately with `r`.
9. **Reporting & Historical Lookup**:
   - Press `z` anytime to review end-of-day metrics and export the Daily Sales Report (Z-Report) via `p`.
   - In Recent Bills, press `/` or `s` to search historical receipts by Bill ID or mobile number and reprint immediately.

---

## Related Documentation

- [User Guide](USER_GUIDE.md) — Operational walkthrough for cashiers and managers.
- [Developer Guide](DEVELOPER_GUIDE.md) — Deep technical documentation and extension guide.
- [Database Reference](DATABASE.md) — Turso SQLite schema, tables, and query reference.
- [Keyboard Reference](KEYBINDINGS.md) — Full hotkey list and quick shortcuts.
- [Configuration via CSV](CONFIGURATION.md) — CSV format specification for menus, areas, and offers.
