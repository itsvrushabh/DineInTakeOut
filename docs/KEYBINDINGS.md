# Keybindings

| Key | Action |
| `?` | Show this keybinding help |
| `Tab` / `Shift+Tab` | Move focus between panels |
| `↑` / `↓` or `j` / `k` | Select a menu item, cart item, area, or saved bill |
| `←` / `→` or `h` / `l` | Select a table in the active area |
| `[` / `]` | Cycle left / right through all open orders (dine-in and take-out) |
| `1`–`9` | Jump to a table area |
| `g` | Search / jump to any table (by number, area, or status) |
| `/` | Focus menu search |
| `Enter` | Add menu item; open or switch table order; pay from cart |
| `=` / `+` | Increase selected cart-item quantity |
| `-` | Decrease selected cart-item quantity; removes it at quantity one |
| `x` / `Delete` | Remove selected cart item |
| `p` (or `b` in Tables) | Generate active bill; or update payment type (UPI, Cash, Card) if already paid |
| `t` | Open take-out order |
| `s` | Advance the active order: Taking order → Serving → Ready for bill |
| `c` | Clear an unpaid cart in Menu, or settle/close a paid order in Tables (prompts mode of payment) |
| `r` | Mark the selected cleaning table as ready |
| `e` / `i` | Export / import the **full config** (menu, areas, offers, GSTIN, AC rate) as CSV while in Menu |
| `q` / `Esc` | Quit (`Esc` also exits search and the billing popups) |

## Table lifecycle

```
Ready → Taking order → Serving → Ready for bill → Bill paid
      ↑                                              │
      └────────── Cleaning (~10 min) ←───────────────┘
```

- `Enter` opens an order on a Ready table (or switches to the order on an
  occupied one).
- `s` advances the stage; the floor-plan card updates instantly.
- `p` opens the billing prompt — type the customer's 10-digit mobile number
  (digits only, `Backspace` to fix, `Esc` to cancel). `Enter` generates the
  bill; if any discount **offers** exist, a popup first asks which offer (or
  none) to apply, then the table shows **Bill paid** while the guests finish up.
- `c` closes the paid order: first a popup asks the **mode of payment**
  (Cash / UPI / Card / Person credit / Have it on hotel), then the table
  enters **Cleaning** and turns Ready by itself after ~10 minutes, or
  immediately with `r`.
- The legend row in the floor plan maps every colour to its stage. Each area
   label shows how many of its tables are free. Areas are fully configurable via
   CSV (see below) — add, remove, rename, set table counts, and mark which are
   air-conditioned.

## Configuration (CSV import / export)

Configuration is managed with CSV files rather than an in-app editor. From the
**Menu** panel:

- `e` exports the full config bundle next to `menu.csv`: `menu.csv`, `areas.csv`,
  `offers.csv`, and `config.csv`.
- `i` imports those four files, validates them, persists everything to the
  database, and rebuilds the in-memory table map.

The files are plain, spreadsheet-friendly CSV — see `docs/CONFIGURATION.md` for
the exact columns. Offers appear at billing time so staff can apply them to a
bill; AC areas add the surcharge and 5% GST; the GSTIN is printed on every
receipt.

All table states, unpaid orders, and paid bills are stored in the database, so
an application restart resumes the shift exactly where it left off.

## Billing prompt

`p` opens a modal that captures the customer's 10-digit mobile number. Digits
are typed directly (5 + 5 display), `Backspace` deletes, `Esc` cancels billing,
and `Enter` generates the bill once all 10 digits are entered. The number is
printed on the receipt and stored with the paid order in the database.

## Updating payment type & closing orders

- Pressing `p` (or `b`) on an already-paid bill opens the **Update Payment Type** popup to set or change how the customer paid (`UPI`, `Cash`, `Card`, etc.). This immediately updates the bill title, totals breakdown (`Payment Type: UPI`), printed receipt, recent bills list (`[UPI]`), and SQLite database.
- Pressing `c` on a paid order opens the **Settle & Close Order** popup.
- In the payment modal, navigate with `↑`/`↓` or use quick shortcuts:
  - `1` or `c`: **Cash**
  - `2` or `u`: **UPI**
  - `3` or `d`: **Card**
  - `4`: **Person credit**
  - `5`: **Have it on hotel**
- Press `Enter` to confirm, or `Esc` to cancel.

## Recent bills

From the Tables panel, press `Tab` to focus **Recent bills**. Use `↑` / `↓` to select one and view its saved receipt in the bill panel. Previous bills cannot be edited.

---

## Related Documentation

- [User Guide](USER_GUIDE.md) — Comprehensive user and cashier guide.
- [Developer Guide](DEVELOPER_GUIDE.md) — Internal architecture and extension guide.
- [Database Reference](DATABASE.md) — Schema definitions and SQL queries.
- [Configuration via CSV](CONFIGURATION.md) — CSV format specification for menus, areas, and offers.
