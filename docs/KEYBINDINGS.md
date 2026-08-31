# Keybindings

| Key | Action |
| `?` | Show this keybinding help |
| `Tab` / `Shift+Tab` | Move focus between panels |
| `↑` / `↓` or `j` / `k` | Select a menu item, cart item, area, or saved bill |
| `←` / `→` or `h` / `l` | Select a table in the active area |
| `[` / `]` | Cycle left / right through all open orders (dine-in and take-out) |
| `1`–`9` | Jump to a table area |
| `/` | Focus menu search |
| `Enter` | Add menu item; open or switch table order; pay from cart |
| `=` / `+` | Increase selected cart-item quantity |
| `-` | Decrease selected cart-item quantity; removes it at quantity one |
| `x` / `Delete` | Remove selected cart item |
| `p` (or `b` in Tables) | Generate and save the active bill |
| `t` | Open take-out order |
| `s` | Advance the active order: Taking order → Serving → Ready for bill |
| `c` | Clear an unpaid cart in Menu, or close a paid order in Tables (asks the mode of payment first) |
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

## Closing & mode of payment

Pressing `c` on a paid order opens the **mode of payment** popup: `↑↓`,
`j/k`, or the number keys `1–5` choose between Cash, UPI, Card, Person
credit, and Have it on hotel. The order's label, bill number, and total are
shown for confirmation. `Enter` records the mode against the stored bill and
closes the order; `Esc` cancels closing.

## Recent bills

From the Tables panel, press `Tab` to focus **Recent bills**. Use `↑` / `↓` to select one and view its saved receipt in the bill panel. Previous bills cannot be edited.
