# Database Specification & Schema Reference

This document provides complete documentation for the persistence layer of **DineInTakeOut**, powered by the embedded **Turso SQLite engine**.

---

## 1. Overview

DineInTakeOut uses an embedded, file-based Turso database located at:
```text
data/billing.db
```
The database engine is written in Rust, fully compatible with standard SQLite 3. It provides zero-overhead ACID transactions, instant startup, crash resilience, and seamless forward compatibility with Turso Cloud distributed synchronization.

---

## 2. Table Schemas

The database schema consists of nine interrelated tables created automatically on first run:

```text
┌────────────────┐       ┌─────────────────┐
│   menu_items   │       │     orders      │
└────────────────┘       └────────┬────────┘
                                  │ 1:N
                                  ▼
                         ┌─────────────────┐
                         │   order_items   │
                         └─────────────────┘

┌────────────────┐       ┌─────────────────┐
│  open_orders   │       │ physical_tables │
└───────┬────────┘       └─────────────────┘
        │ 1:N
        ▼
┌───────────────────┐    ┌─────────────────┐
│ open_order_items  │    │      areas      │
└───────────────────┘    └─────────────────┘

┌────────────────┐       ┌─────────────────┐
│     offers     │       │    settings     │
└────────────────┘       └─────────────────┘
```

---

### Table 1: `menu_items`
Stores the active menu catalogue.

```sql
CREATE TABLE IF NOT EXISTS menu_items (
    name         TEXT PRIMARY KEY,
    category     TEXT NOT NULL,
    unit         TEXT NOT NULL DEFAULT '',
    price        REAL NOT NULL CHECK (price >= 0),
    is_available INTEGER NOT NULL DEFAULT 1
);
```

| Column | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `name` | `TEXT` | `PRIMARY KEY` | Unique dish name (e.g. "Paneer Butter Masala") |
| `category` | `TEXT` | `NOT NULL` | Menu section (e.g. "Main Course", "Starters") |
| `unit` | `TEXT` | `NOT NULL DEFAULT ''` | Portion size description (e.g. "1 plate", "2 pcs") |
| `price` | `REAL` | `NOT NULL CHECK (price >= 0)` | Price in Indian Rupees |
| `is_available` | `INTEGER` | `NOT NULL DEFAULT 1` | `1` if item is active; `0` if marked out-of-stock ("86") |

---

### Table 2: `orders`
Archive of completed/paid orders. Once written, these records are immutable.

```sql
CREATE TABLE IF NOT EXISTS orders (
    id              INTEGER PRIMARY KEY,
    label           TEXT NOT NULL,
    service         TEXT NOT NULL,
    table_number    INTEGER,
    area            TEXT,
    customer_mobile TEXT NOT NULL DEFAULT '',
    subtotal        REAL NOT NULL DEFAULT 0,
    discount        REAL NOT NULL DEFAULT 0,
    ac_charge       REAL NOT NULL DEFAULT 0,
    tax             REAL NOT NULL,
    total           REAL NOT NULL,
    payment_mode    TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (DATETIME('now', 'localtime'))
);
```

| Column | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `id` | `INTEGER` | `PRIMARY KEY` | Incremental Bill / Order ID |
| `label` | `TEXT` | `NOT NULL` | Order label (e.g. "Main-T1", "TK1") |
| `service` | `TEXT` | `NOT NULL` | `"DINE_IN"` or `"TAKE_OUT"` |
| `table_number` | `INTEGER` | `NULLABLE` | Physical table number (for dine-in) |
| `area` | `TEXT` | `NULLABLE` | Dining room area name |
| `customer_mobile`| `TEXT` | `NOT NULL DEFAULT ''` | 10-digit customer phone number |
| `subtotal` | `REAL` | `NOT NULL DEFAULT 0` | Gross food subtotal before discounts |
| `discount` | `REAL` | `NOT NULL DEFAULT 0` | Applied discount amount |
| `ac_charge` | `REAL` | `NOT NULL DEFAULT 0` | Applied AC surcharge amount |
| `tax` | `REAL` | `NOT NULL` | Total GST amount |
| `total` | `REAL` | `NOT NULL` | Final payable amount |
| `payment_mode` | `TEXT` | `NOT NULL DEFAULT ''` | Settlement mode (`"UPI"`, `"CASH"`, `"CARD"`, etc.) |
| `status` | `TEXT` | `NOT NULL` | Order status (typically `"PAID"`) |
| `created_at` | `TEXT` | `DEFAULT (DATETIME('now'))` | Timestamp of bill generation |

---

### Table 3: `order_items`
Line items belonging to completed bills.

```sql
CREATE TABLE IF NOT EXISTS order_items (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id   INTEGER NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    unit_price REAL NOT NULL,
    qty        INTEGER NOT NULL CHECK (qty > 0),
    line_total REAL NOT NULL,
    notes      TEXT NOT NULL DEFAULT ''
);
```

---

### Table 4: `open_orders`
Stores active, unpaid orders to enable crash-recovery across application restarts.

```sql
CREATE TABLE IF NOT EXISTS open_orders (
    id           INTEGER PRIMARY KEY,
    label        TEXT NOT NULL,
    service      TEXT NOT NULL,
    table_number INTEGER,
    area         TEXT,
    is_ac        INTEGER NOT NULL DEFAULT 0,
    ac_rate      REAL NOT NULL DEFAULT 0,
    status       TEXT NOT NULL
);
```

---

### Table 5: `open_order_items`
Cart items for active, unpaid orders.

```sql
CREATE TABLE IF NOT EXISTS open_order_items (
    order_id   INTEGER NOT NULL REFERENCES open_orders(id) ON DELETE CASCADE,
    position   INTEGER NOT NULL,
    name       TEXT NOT NULL,
    unit_price REAL NOT NULL,
    qty        INTEGER NOT NULL CHECK (qty > 0),
    notes      TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (order_id, position)
);
```

---

### Table 6: `physical_tables`
Live floor plan state tracking physical tables across all dining areas.

```sql
CREATE TABLE IF NOT EXISTS physical_tables (
    area         TEXT NOT NULL,
    table_number INTEGER NOT NULL,
    status       TEXT NOT NULL,
    order_id     INTEGER,
    dirty_since  TEXT,
    PRIMARY KEY (area, table_number)
);
```

| Column | Type | Description |
| :--- | :--- | :--- |
| `area` | `TEXT` | Dining room name |
| `table_number` | `INTEGER` | Table number in that room |
| `status` | `TEXT` | `"READY"`, `"ORDERING"`, `"SERVING"`, `"BILL_REQUESTED"`, `"PAID"`, `"DIRTY"` |
| `order_id` | `INTEGER` | ID of currently assigned active order |
| `dirty_since` | `TEXT` | ISO-8601 timestamp when table entered cleaning state |

---

### Table 7: `areas`
Configured dining rooms, capacities, and air conditioning settings.

```sql
CREATE TABLE IF NOT EXISTS areas (
    name        TEXT PRIMARY KEY,
    is_ac       INTEGER NOT NULL DEFAULT 0,
    table_count INTEGER NOT NULL CHECK (table_count > 0)
);
```

---

### Table 8: `offers`
Promotional discounts selectable at billing time.

```sql
CREATE TABLE IF NOT EXISTS offers (
    id               INTEGER PRIMARY KEY,
    name             TEXT NOT NULL,
    discount_percent REAL NOT NULL CHECK (discount_percent >= 0 AND discount_percent <= 100)
);
```

---

### Table 9: `settings`
Global restaurant key-value settings.

```sql
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```
Standard keys:
- `'restaurant_name'`: The hotel / restaurant name (e.g. `'SHREE KRISHNA RESTAURANT'`).
- `'restaurant_address'`: Physical address (printed on receipts).
- `'restaurant_contact'`: Phone / mobile contact (printed on receipts).
- `'gst_number'`: The restaurant's registered GSTIN (printed on receipts).
- `'ac_rate'`: Surcharge percentage rate for AC dining areas (e.g. `'6.0'`).
- `'upi_id'`: Virtual Payment Address (VPA) for dynamic UPI QR generation (e.g. `'shreekrishna@upi'`).

---

### Table 10: `kots`
Persistent storage for all Kitchen Order Tickets (KOT) sent to the kitchen.

```sql
CREATE TABLE IF NOT EXISTS kots (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id    INTEGER NOT NULL,
    label       TEXT NOT NULL,
    area        TEXT NOT NULL DEFAULT '',
    item_count  INTEGER NOT NULL DEFAULT 0,
    ticket_text TEXT NOT NULL,
    is_reprint  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
```

| Column | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `id` | `INTEGER` | `PRIMARY KEY AUTOINCREMENT` | Unique ticket sequence ID |
| `order_id` | `INTEGER` | `NOT NULL` | Associated open or paid order ID |
| `label` | `TEXT` | `NOT NULL` | Table or takeout label (e.g. `AC-T4`, `TK1`) |
| `area` | `TEXT` | `NOT NULL DEFAULT ''` | Dining area name |
| `item_count` | `INTEGER` | `NOT NULL DEFAULT 0` | Total quantity of items in ticket |
| `ticket_text` | `TEXT` | `NOT NULL` | Full rendered 42-column KOT ticket |
| `is_reprint` | `INTEGER` | `NOT NULL DEFAULT 0` | 1 if duplicate / reprint, 0 for initial print |
| `created_at` | `TEXT` | `DEFAULT datetime('now', 'localtime')` | Ticket creation timestamp |

---

### Table 11: `z_reports`
Archived daily sales and shift settlement reports.

```sql
CREATE TABLE IF NOT EXISTS z_reports (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    report_date TEXT NOT NULL,
    gross_sales REAL NOT NULL DEFAULT 0,
    net_sales   REAL NOT NULL DEFAULT 0,
    bill_count  INTEGER NOT NULL DEFAULT 0,
    report_text TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);
```

| Column | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `id` | `INTEGER` | `PRIMARY KEY AUTOINCREMENT` | Unique report sequence ID |
| `report_date` | `TEXT` | `NOT NULL` | Sales report date (`YYYY-MM-DD`) |
| `gross_sales` | `REAL` | `NOT NULL DEFAULT 0` | Total sales before deductions |
| `net_sales` | `REAL` | `NOT NULL DEFAULT 0` | Net collected revenue |
| `bill_count` | `INTEGER` | `NOT NULL DEFAULT 0` | Total settled orders for the day |
| `report_text` | `TEXT` | `NOT NULL` | Complete rendered 42-column Z-Report text |
| `created_at` | `TEXT` | `DEFAULT datetime('now', 'localtime')` | Report generation timestamp |

---

## 3. Key Queries & Data Operations

### 1. Generating Next Sequential Bill Number
```sql
SELECT COALESCE(MAX(id), 0) + 1 FROM (
    SELECT id FROM orders
    UNION ALL
    SELECT id FROM open_orders
);
```

### 2. Archiving a Paid Bill
Executed within an ACID transaction:
```sql
BEGIN TRANSACTION;
INSERT INTO orders (id, label, service, table_number, area, customer_mobile,
                    subtotal, discount, ac_charge, tax, total, payment_mode, status)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'PAID');

-- Insert line items with kitchen notes
INSERT INTO order_items (order_id, name, unit_price, qty, line_total, notes)
VALUES (?1, ?2, ?3, ?4, ?5, ?6);

-- Remove from open orders
DELETE FROM open_orders WHERE id = ?1;
COMMIT;
```

### 3. Updating Payment Mode After Settlement
```sql
UPDATE orders SET payment_mode = ?2 WHERE id = ?1;
```

### 4. Fetching Recent Bills with Payment Type
```sql
SELECT id, label, service, total, payment_mode
FROM orders
ORDER BY id DESC
LIMIT 5;
```

### 5. Daily Sales & Shift Aggregation (Z-Report)
```sql
SELECT 
    COUNT(*),
    COALESCE(SUM(subtotal), 0),
    COALESCE(SUM(discount), 0),
    COALESCE(SUM(ac_charge), 0),
    COALESCE(SUM(tax), 0),
    COALESCE(SUM(total), 0)
FROM orders
WHERE substr(created_at, 1, 10) = ?1;
```

### 6. Historical Bill Search
```sql
SELECT id, label, service, customer_mobile, total, payment_mode, created_at
FROM orders
WHERE CAST(id AS TEXT) LIKE ?1 OR customer_mobile LIKE ?1
ORDER BY id DESC
LIMIT 50;
```

---

## 4. Inspecting the Database with SQLite CLI

You can query the database directly using any SQLite tool while the application is stopped:

```bash
# Check all tables
sqlite3 data/billing.db ".tables"

# View latest 5 paid orders
sqlite3 data/billing.db "SELECT id, label, total, payment_mode, created_at FROM orders ORDER BY id DESC LIMIT 5;"

# Check table states
sqlite3 data/billing.db "SELECT area, table_number, status, dirty_since FROM physical_tables;"
```

---

## 5. Backups & Disaster Recovery

- **Automated Daily Backups**:
  On application startup, `DineInTakeOut` automatically backs up the database file to:
  ```text
  data/backups/billing_YYYY-MM-DD.db
  ```
  If a backup for today's date already exists, it will not be overwritten, ensuring shift safety across day starts.

- **Manual Backups**:
  ```bash
  cp data/billing.db data/backups/billing_manual_$(date +%F_%T).db
  ```

- **Resetting to Clean Slate**:
  Removing `data/billing.db` will cause the application to re-initialize an empty database seeded from `menu.csv` on next boot.
