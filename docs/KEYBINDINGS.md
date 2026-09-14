# Keybindings

| Key | Action |
| :--- | :--- |
| `?` | Show this keybinding help |
| `Tab` / `Shift+Tab` | Move focus between panels |
| `↑` / `↓` or `j` / `k` | Select a menu item, cart item, area, or saved bill |
| `←` / `→` or `h` / `l` | Select a table in Tables; Filter category pills in Menu; Switch Bills vs KOTs in Recent panel |
| `[` / `]` | Cycle left / right through all open orders (dine-in and take-out) |
| `1`–`7` | Jump directly to Box: `[1]` Tables, `[2]` Jump, `[3]` Search, `[4]` Menu, `[5]` Cart, `[6]` Bills, `[7]` KOTs |
| `g` (or `2`) | Search / jump to any table (by number, area, or status) |
| `m` | Move / transfer order to clean table, or merge into occupied table (in Tables) |
| `/` | Focus menu search |
| `o` | Toggle menu item out-of-stock ("86") status (in Menu) |
| `Enter` | Add menu item; open or switch table order; pay from cart |
| `=` / `+` | Increase selected cart-item quantity |
| `-` | Decrease selected cart-item quantity; removes it at quantity one |
| `x` / `Delete` | Remove selected cart item |
| `c` | Toggle Complimentary (`[NC]`, ₹0 subtotal) on selected cart item (in Cart) / Settle & close order (in Tables) |
| `d` | Cycle item discount: 0% → 10% → 20% → 50% → 100% → 0% (in Cart) |
| `C` (`Shift+C`) | Clear active cart items (in Cart) |
| `n` | Add / edit kitchen note on selected cart item |
| `k` | Generate incremental / delta Kitchen Order Ticket (KOT) for newly added/increased items |
| `K` (`Shift+K`) | Force full reprint of all order items on Kitchen Order Ticket (in Cart) |
| `K` / `F7` | Open fullscreen **Kitchen Display System (KDS)** live order monitor (in Recent panel or globally) |
| `p` (or `b` in Tables) | Generate active bill; or update payment type (UPI, Cash, Card, Split) if already paid |
| `q` | Display dynamic on-screen UPI QR code (in payment modal) |
| `4` / `s` | Open **Split Payment Tender** modal (in payment modal) |
| `a` | Auto-fill remaining unpaid balance into active field (in Split Payment modal) |
| `A` / `F8` | Open **End-of-Day Sales & Tax Analytics** dashboard modal |
| `z` | Open Daily Sales Summary (Z-Report) & Settlement breakdown |
| `p` / `r` / `Enter` | Reprint selected bill or KOT ticket (in Recent panel) |
| `/` or `s` | Search and reprint historical bills by Bill ID or Mobile (in Recent Bills) |
| `t` | Open take-out order |
| `s` | Advance the active order: Taking order → Serving → Ready for bill |
| `r` | Mark the selected cleaning table as ready |
| `e` / `i` | Export / import the **full config** (menu, areas, offers, GSTIN, AC rate, UPI ID) as CSV while in Menu |
| `q` / `Esc` | Quit (`Esc` also exits search and modals) |

## Table lifecycle & Move / Merge (`m`)

```
Ready → Taking order → Serving → Ready for bill → Bill paid
      ↑                                              │
      └────────── Cleaning (~10 min) ←───────────────┘
```

- `Enter` opens an order on a Ready table (or switches to the order on an occupied one).
- `s` advances the stage; the floor-plan card updates instantly.
- `m` opens the **Table Move / Transfer & Merge** modal:
  - Select any free table `[MOVE]` to relocate the entire order. Taxes and AC surcharges automatically adjust to match the destination area.
  - Select an occupied table `[MERGE]` to combine all cart items and quantities into the target check.
- `p` opens the billing prompt — type the customer's 10-digit mobile number.
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
  - `q`: **Dynamic UPI QR Code**: displays full on-screen QR code for direct smartphone scanning.
- Press `Enter` to confirm, or `Esc` to cancel.

## Incremental / Delta KOTs (`k`) & Full Reprint (`Shift+K`)

- **Delta KOT Dispatch (`k`)**: Tracks `kot_sent_qty` per cart line. When adding items to an active order or increasing quantities after an initial KOT has fired, pressing `k` prints an **ADD-ON / DELTA** ticket containing *only* the new items and incremental quantities.
- **Full Reprint (`Shift+K`)**: Force-reprints the entire active order ticket with prominent `[REPRINT]` banners for kitchen staff.
- Cart items display live tags:
  - `[KOT]`: Item has been fully dispatched to the kitchen.
  - `[KOT 2/3]`: Partial quantity sent (e.g. 2 sent, 1 pending add-on).

## Kitchen Display System (KDS) (`K` / `F7`)

- Open the fullscreen **Kitchen Display System** monitor by pressing `K` from the Recent Bills / KOT panel, or `F7` globally.
- Real-time elapsed preparation timer per ticket:
  - `< 10 mins`: **Green** (Normal)
  - `10–20 mins`: **Yellow** (Warning)
  - `> 20 mins`: **Red** (Urgent alert)
- **Bump Order Status**: Press `Space` or `Enter` to cycle ticket progression:
  `PENDING` ➔ `PREPARING` ➔ `READY` ➔ `SERVED` (served tickets automatically archive from active monitor).
- Navigate tickets with `↑` / `↓` or `w` / `s`. Press `r` / `F5` to force refresh. Press `Esc` or `K` to exit KDS.

## Split Payment Tender (`4` / `s`)

- In the payment modal, select **Split Tender** (`4` or `s`) for mixed payments (e.g. part Cash, part UPI, part Card).
- Tab / `↑` / `↓` to switch between tender fields.
- Type amounts directly; press `Backspace` to edit.
- **Auto-Fill Balance (`a`)**: Press `a` in any active field to instantly compute and auto-populate the exact remaining balance due.
- Press `Enter` when the balance is settled (₹0.00 due) to complete payment.

## Complimentary (NC) (`c`) & Item Discounts (`d`)

- In Cart Box `[5]`:
  - Highlight any line item and press `c` to toggle **Complimentary (`[NC]`)**. Subtotal for that item evaluates to ₹0.00 and is excluded from taxable calculations.
  - Press `d` to cycle **Item Discounts**: `0%` ➔ `10%` ➔ `20%` ➔ `50%` ➔ `100%` ➔ `0%`.
  - Press `Shift+C` (`C`) to quickly clear all items from the active cart.

## Customer CRM & Visit History

- In the **Mobile Entry** modal, typing 4+ digits of a customer's phone number looks up their profile live in SQLite.
- Displays an instant **VIP Member Banner**:
  `★ VIP Member: 5 visits · Spent ₹4,250.00`
  `↳ Favorite dishes: Paneer Butter Masala, Butter Naan`

## End-of-Day Sales & Tax Analytics (`A` / `F8`)

- Press `A` or `F8` from anywhere in the app to launch the **Sales & Tax Analytics Dashboard**.
- Features 4 live KPI cards:
  - **Net Revenue** & Gross Sales
  - **Total Orders** & Average Ticket Size
  - **GST Tax Collected** (with CGST 2.5% & SGST 2.5% split)
  - **Discounts Given**
- **Payment Tender Distribution**: Bill counts, total revenues, and % share per tender (Cash, UPI, Card, Split, etc.).
- **Best Selling Dishes**: Top 5 revenue items with ASCII volume trend bars (`████████`).

---

## Related Documentation

- [User Guide](USER_GUIDE.md) — Comprehensive user and cashier guide.
- [Developer Guide](DEVELOPER_GUIDE.md) — Internal architecture and extension guide.
- [Database Reference](DATABASE.md) — Schema definitions and SQL queries.
- [Configuration via CSV](CONFIGURATION.md) — CSV format specification for menus, areas, and offers.
