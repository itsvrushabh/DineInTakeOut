# Architecture & System Design

The application is structured into decoupled domain, persistence, configuration, state management, and UI rendering modules.

```text
CSV / Default Fixtures ──> config.rs ──┐
                                       ▼
Turso SQLite DB ──────────> db.rs ──> app.rs ──> ui/ (Ratatui)
                                       │
                                       ├── open dine-in / take-out orders
                                       ├── table lifecycle & cart operations
                                       ├── table search / jump lookup
                                       └── receipt & payment mode management
                                                │
bills/bill_*.txt <── saved receipt / optional CUPS print ── receipts.rs
```

---

## Modules & Component Boundaries

- **`src/models.rs`**: Core domain types (`MenuItem`, `CartLine`, `Service`, `PaymentMode`, `Focus`, `OrderStatus`, `TableStatus`, `Area`, `PhysicalTable`, `Offer`, `BillTotals`, `Order`, `BillSummary`) and price breakdown calculation. Completely free of terminal and file-system dependencies.
- **`src/config.rs`**: Default menu dataset and spreadsheet-friendly CSV I/O for `menu.csv`, `areas.csv`, `offers.csv`, and `config.csv`.
- **`src/receipts.rs`**: Plaintext 42-column receipt rendering, `bills/` file storage, optional CUPS printing, and recent bill summary parsing.
- **`src/db.rs`**: Embedded Turso database engine (`data/billing.db`): menu catalogue, physical table status, unpaid open orders and cart lines (for session restoration across restarts), paid order history, areas, settings, and offers.
- **`src/app.rs`**: Application state (`App`), lifecycle transitions, cart actions, table search/jump filtering, payment mode updates, modal flows, and keyboard input routing.
- **`src/ui/`**: Modular Ratatui rendering components:
  - `ui/mod.rs`: Root layout and area distribution with adaptive compact height support.
  - `ui/floor_plan.rs`: Floor plan area bars, table cards, take-out chips, and lifecycle legend.
  - `ui/table_info.rs`: Active table details (status, order ID, bill total, payment mode badge) and interactive table search/jump input (`g`).
  - `ui/search.rs`: Fuzzy search input box and match counters.
  - `ui/menu.rs`: Menu table with category, unit, and price columns.
  - `ui/bill.rs`: Cart lines table, paid title banner, and non-overlapping totals breakdown.
  - `ui/notifications.rs`: Status banner (adapts to compact and standard terminal sizes).
  - `ui/recent_bills.rs`: Recent bill history panel with payment mode tags.
  - `ui/modals.rs`: Customer mobile capture, payment mode confirmation, and discount offer selection dialogs.
  - `ui/footer.rs`: Contextual keyboard shortcut reference.
- **`src/main.rs`**: Minimal entry point: terminal initialization/restoration, raw mode setup, and event polling loop.

---

## Operational Data Flow & Lifecycle

1. **Table Initiation**: Open a table order (`Enter`) or take-out order (`t`). Dine-in tables enter *Taking order*.
2. **Ordering**: Add menu items and adjust quantities while the order is unpaid. Use `s` to advance through *Serving* and *Ready for bill*.
3. **Table Jump (`g`)**: Instantly search across all tables in all areas by number, name, or status, jumping focus directly to the target order.
4. **Billing (`p` / `b`)**:
   - Captures the customer's 10-digit mobile number (optional, press `Enter` on empty to skip).
   - Prompts for an applicable discount offer if offers exist.
   - Saves the bill with item lines, tax, and discount breakdown to the database.
   - Generates the text receipt in `bills/` and prints via CUPS if configured.
   - Status changes to *Bill paid*.
5. **Payment Type Updates (`p` / `b`)**:
   - If the guest pays via UPI, Cash, or Card after billing, pressing `p` or `b` opens the payment modal.
   - Selecting a payment type immediately updates the active bill, totals breakdown (`Payment Type: UPI`), recent bills list (`[UPI]`), table details (`[Paid: UPI]`), and SQLite database.
6. **Settlement & Cleaning (`c`)**:
   - Confirms the final payment mode, archives the order, and transitions the table to *Cleaning*.
   - A 10-minute auto-ready countdown begins, turning *Ready* automatically, or immediately with `r`.

---

## Related Documentation

- [User Guide](USER_GUIDE.md) — Operational walkthrough for cashiers and managers.
- [Developer Guide](DEVELOPER_GUIDE.md) — Deep technical documentation and extension guide.
- [Database Reference](DATABASE.md) — Turso SQLite schema, tables, and query reference.
- [Keyboard Reference](KEYBINDINGS.md) — Full hotkey list and quick shortcuts.
- [Configuration via CSV](CONFIGURATION.md) — CSV format specification for menus, areas, and offers.
