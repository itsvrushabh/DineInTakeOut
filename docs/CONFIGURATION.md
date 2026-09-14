# Configuration via CSV

All configuration is managed through plain CSV files (spreadsheet-friendly,
editable in Excel/LibreOffice/Numbers). From the **Menu** panel:

- `e` — **export** the full config bundle next to `menu.csv`:
  `menu.csv`, `areas.csv`, `offers.csv`, `config.csv`.
- `i` — **import** those four files, validate them, persist everything to the
  embedded database, and rebuild the in-memory table map.

Export writes the files next to wherever `menu.csv` lives (the project root by
default). Import reads the same files; if any file is missing or empty the
import is aborted for that file with a notification, so a partial export never
wipes your live config.

## `menu.csv`

The menu catalogue (seeds an empty database on first run).

| Column      | Meaning                                         |
| ----------- | ----------------------------------------------- |
| `Category`  | Grouping shown in the menu (e.g. "Main Course") |
| `Item Name` | Dish name                                       |
| `Unit`      | Portion description (e.g. "1 plate", "250 ml")  |
| `Price`     | Price in rupees (numeric, > 0)                  |

## `areas.csv`

Dining areas and their tables. Empty rows are skipped; an empty file aborts the
import.

| Column       | Meaning                                                       |
| ------------ | ------------------------------------------------------------- |
| `Area`       | Area name (e.g. "AC Rooms") — must be unique                  |
| `IsAC`       | `yes` / `y` / `1` / `true` marks the area air-conditioned    |
| `TableCount` | Number of physical tables in the area (integer, minimum 1)    |

AC areas add the surcharge (see `config.csv`) plus 5 % GST; non-AC dine-in
areas have no tax (0 % GST). Take-out orders include 8 % GST.

## `offers.csv`

Named discount offers selectable at billing time.

| Column             | Meaning                                            |
| ------------------ | -------------------------------------------------- |
| `Name`             | Offer label shown in the billing popup             |
| `DiscountPercent`  | Discount on the subtotal, `0`–`100` (numeric)      |

An empty `offers.csv` simply clears all offers (offers are optional).

## `config.csv`

Key/value settings. Both keys are optional; missing keys keep the current value.

| Key         | Value                                  |
| ----------- | -------------------------------------- |
| `GSTNumber` | Registered GSTIN printed on receipts   |
| `AcRate`    | AC surcharge rate as a percentage (e.g. `6`) |

Export always writes both rows. On import, `AcRate` is parsed as a percentage
and clamped to `0`–`100`; `GSTNumber` is stored verbatim.

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
