# Configuration via CSV

All configuration is managed through plain CSV files (spreadsheet-friendly,
editable in Excel/LibreOffice/Numbers). From the **Menu** panel:

- `e` — **export** the full config bundle next to `menu.csv`:
  `menu.csv`, `table.csv`, `areas.csv`, `offers.csv`, `config.csv`.
- `i` — **import** those files, validate them, persist everything to the
  embedded database, and rebuild the in-memory table map.

Export writes the files next to wherever `menu.csv` lives (the project root by
default). Import reads the same files; if any file is missing or empty the
import is aborted for that file with a notification, so a partial export never
wipes your live config.

## `menu.csv`

The primary menu catalogue. In DineInTakeOut, the built-in fallback menu is completely empty (`default_menu() == []`); **all menu items are sourced strictly from `menu.csv`** (or the persisted database catalogue).

| Column      | Meaning                                         |
| ----------- | ----------------------------------------------- |
| `Category`  | Grouping shown in the menu (e.g. "Breakfast & Snacks", "Main Course (Veg)") |
| `Item Name` | Dish name                                       |
| `Unit`      | Portion description (e.g. "1 plate", "2 pcs")   |
| `Price`     | Price in rupees (numeric, > 0)                  |
| `Available` | Optional stock status (`yes`/`true`/`1` or `no`/`false`/`0`). Defaults to `yes`. Unavailable items show `[86 OUT]`. |

## `table.csv` (and `areas.csv`)

Dining areas, sections, and their table counts. DineInTakeOut supports both `table.csv` and `areas.csv` with flexible column headers.

### Standard `table.csv` Format:
| Column       | Meaning                                                      |
| ------------ | ------------------------------------------------------------ |
| `Table Type` | Area / section name (e.g. "Main Hall", "AC Dining", "Garden")|
| `Count`      | Number of physical tables in the area (integer, minimum 1)   |
| `IsAC`       | `yes` / `y` / `1` / `true` marks the area air-conditioned    |

*Alternative header aliases such as `Area`, `TableCount`, and `AC` are also automatically recognized.*

AC areas add the configurable AC surcharge (see `config.csv`) plus 5 % GST; non-AC dine-in
areas have 0 % GST. Take-out orders include 8 % GST.

Default configuration:
- **Main Hall**: 8 tables (Non-AC)
- **AC Dining**: 6 tables (AC)
- **Family Section**: 4 tables (AC)
- **Garden**: 6 tables (Non-AC)

## `offers.csv`

Named discount offers selectable at billing time.

| Column             | Meaning                                            |
| ------------------ | -------------------------------------------------- |
| `Name`             | Offer label shown in the billing popup             |
| `DiscountPercent`  | Discount on the subtotal, `0`–`100` (numeric)      |

An empty `offers.csv` simply clears all offers (offers are optional).

## `config.csv`

Restaurant identity and billing parameters. Exported and imported as Key/Value pairs.

| Key              | Default / Example                | Meaning                                  |
| ---------------- | -------------------------------- | ---------------------------------------- |
| `RestaurantName` | `SHREE KRISHNA RESTAURANT`       | Hotel / Restaurant name printed on receipts, KOTs, and Z-reports |
| `Address`        | `Station Road, Near Main Market` | Physical street address printed on bills |
| `Contact`        | `+91 98765 43210`                | Phone / mobile contact printed on bills  |
| `GSTNumber`      | `27AAPFU0939F1ZV`                | Registered GSTIN printed on receipts     |
| `AcRate`         | `6.00`                           | AC surcharge rate as a percentage (e.g. `6.00` for 6%) |
| `UpiId`          | `shreekrishna@upi`               | UPI VPA address used for QR payments     |

Export writes all fields. On import, `AcRate` is parsed as a percentage (e.g. `6.0` or `0.06`), and the hotel name, address, contact, GSTIN, and UPI ID are synchronized to the local database.

## Notes

- After import the table map is rebuilt; open orders are preserved but physical
  table assignments are reset to their saved/clean state.
- The database remains the source of truth at runtime — CSV is just the
  import/export transport.
- Receipts under `bills/` are unaffected by configuration import/export.

---

## Related Documentation

- [User Guide](USER_GUIDE.md) — Step-by-step operating instructions.
- [Developer Guide](DEVELOPER_GUIDE.md) — Codebase architecture and development guidelines.
- [Database Reference](DATABASE.md) — Embedded Turso SQLite schemas and query specifications.
- [Keybindings](KEYBINDINGS.md) — Comprehensive keyboard shortcut reference.
