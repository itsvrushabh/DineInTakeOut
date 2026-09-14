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
    name     TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    unit     TEXT NOT NULL DEFAULT '',
    price    REAL NOT NULL CHECK (price >= 0)
);
```

| Column | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `name` | `TEXT` | `PRIMARY KEY` | Unique dish name (e.g. "Paneer Butter Masala") |
| `category` | `TEXT` | `NOT NULL` | Menu section (e.g. "Main Course", "Starters") |
| `unit` | `TEXT` | `NOT NULL DEFAULT ''` | Portion size description (e.g. "1 plate", "2 pcs") |
| `price` | `REAL` | `NOT NULL CHECK (price >= 0)` | Price in Indian Rupees |

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
    line_total REAL NOT NULL
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
- `'GSTNumber'`: The restaurant's registered GSTIN (printed on receipts).
- `'AcRate'`: Surcharge percentage rate for AC dining areas (e.g. `'6.0'`).

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

-- Insert line items
INSERT INTO order_items (order_id, name, unit_price, qty, line_total)
VALUES (?1, ?2, ?3, ?4, ?5);

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

- **Creating a Backup**: Simply copy the database file:
  ```bash
  cp data/billing.db data/billing_backup_$(date +%F).db
  ```
- **Resetting to Clean Slate**:
  Removing `data/billing.db` will cause the application to re-initialize an empty database seeded from `menu.csv` on next boot.
