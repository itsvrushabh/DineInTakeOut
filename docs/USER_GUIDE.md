# DineInTakeOut — User Guide

A comprehensive manual for restaurant managers, cashiers, and waitstaff using the **DineInTakeOut** terminal billing and table management system.

---

## Table of Contents

1. [Overview & System Requirements](#1-overview--system-requirements)
2. [Terminal Screen Layout](#2-terminal-screen-layout)
3. [Navigation Basics](#3-navigation-basics)
4. [Dine-In Order Workflow](#4-dine-in-order-workflow)
5. [Table Move, Transfer & Merge (`m`)](#5-table-move-transfer--merge-m)
6. [Take-Out Order Workflow](#6-take-out-order-workflow)
7. [Cart Management & Item Notes (`n`)](#7-cart-management--item-notes-n)
8. [Kitchen Order Tickets (KOT) (`k`)](#8-kitchen-order-tickets-kot-k)
9. [Menu Item Stock & Out-of-Stock ("86") Toggling (`o`)](#9-menu-item-stock--out-of-stock-86-toggling-o)
10. [Table Search & Quick Jump (`g`)](#10-table-search--quick-jump-g)
11. [Billing & Customer Mobile Capture (`p` / `b`)](#11-billing--customer-mobile-capture-p--b)
12. [Applying Discount Offers](#12-applying-discount-offers)
13. [Payment Modes & Dynamic UPI QR Code (`q`)](#13-payment-modes--dynamic-upi-qr-code-q)
14. [Updating Payment Type on Paid Bills](#14-updating-payment-type-on-paid-bills)
15. [Daily Sales Summary & Settlement Report / Z-Report (`z`)](#15-daily-sales-summary--settlement-report--z-report-z)
16. [Order Settlement & Table Cleaning](#16-order-settlement--table-cleaning)
17. [Reviewing Recent Bills & Historical Search (`/` or `s`)](#17-reviewing-recent-bills--historical-search--or-s)
18. [Menu & Configuration Management (CSV `e` / `i`)](#18-menu--configuration-management-csv-e--i)
19. [Receipt Printing & Local File Storage](#19-receipt-printing--local-file-storage)
20. [Automatic Daily Database Backups](#20-automatic-daily-database-backups)
21. [Keyboard Shortcuts Cheat Sheet](#21-keyboard-shortcuts-cheat-sheet)
22. [Frequently Asked Questions & Troubleshooting](#22-frequently-asked-questions--troubleshooting)

---

## 1. Overview & System Requirements

**DineInTakeOut** is a high-performance terminal user interface (TUI) designed for front-of-house restaurant operations. It coordinates dining room table states, take-out orders, fast item searching, kitchen order tickets (KOT), receipt generation with tax calculations, on-screen UPI QR codes, shift settlement reports (Z-Reports), and bill settlements.

### System Requirements
- **Operating System**: Linux, macOS, or Windows (with UTF-8 terminal support).
- **Terminal Emulator**: Any modern terminal with Unicode and 256-color support (e.g., Alacritty, Kitty, Foot, Windows Terminal, GNOME Terminal, iTerm2).
- **Recommended Terminal Size**: At least **100 columns × 30 rows** for optimal display. If run in compact windows (height < 33 rows), the interface automatically adapts by hiding the historical bills panel and compressing margins.

---

## 2. Terminal Screen Layout

The screen is divided into clear functional panels designed to minimize clutter while presenting all vital operational data simultaneously:

```text
┌────────────────────────────────────────────────────────┬──────────────────────────────────┐
│ Areas / Floor Plan: [1] Main Hall  [2] AC Rooms  [TK]   │ Table Details [g: Jump]          │
│ T1 [Ready]  T2 [Ordering]  T3 [Serving]  T4 [Paid: UPI]│ Table: T4 [Paid: UPI]            │
│ TK1 [Ready for Bill]                                   │ Order: #4 (Main-T4)              │
│                                                        │ Bill : ₹450.00 [UPI] (3 items)   │
├────────────────────────────────────────────────────────┴──────────────────────────────────┤
│ Search: paneer_                                               (2 matches)                 │
├──────────────────────────────────┬────────────────────────────────────────────────────────┤
│ Menu [Category: Main Course]     │ Bill #4 — PAID via UPI ✔ (Main-T4 / Dine-In)           │
│ Paneer Butter Masala    ₹180.00  │ Item                  Qty    Each    Total             │
│ Palak Paneer   [86 OUT] ₹170.00  │ Butter Naan             2   40.00    80.00             │
│ Dal Makhani             ₹140.00  │   ↳ Extra crisp                                        │
│                                  │ Paneer Butter Masala    2  180.00   360.00             │
│                                  │ ──────────────────────────────────────────────         │
│                                  │ Subtotal                             ₹440.00           │
│                                  │ GST (5.0%)                            ₹22.00           │
│                                  │ TOTAL                                ₹462.00           │
│                                  │ Payment Type                            UPI            │
├──────────────────────────────────┴────────────────────────────────────────────────────────┤
│ Recent Bills [Active] (←/→ switch)        │ KOT Bills (latest 10)                         │
│ Bill #4   Main-T4    Dine-In  [UPI]  ₹462 │ KOT #2  Main-T4 (3 items)  14:32              │
│ Bill #3   TK1        Take-Out [CASH] ₹250 │ KOT #1  AC-101  (2 items)  14:15              │
├───────────────────────────────────────────┴───────────────────────────────────────────────┤
│ Notification: Generated KOT for Main-T4. Saved to database (ID #2)                        │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│ Tab: Next panel · ↑↓: Select · Enter: Add/Open · p: Bill · k: KOT · z: Z-Report · ?: Help │
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

### Panel Overview
1. **Floor Plan (Top Left)**: Shows dining areas, table status cards, and active take-out chips.
2. **Table Details & Jump Box (Top Right)**: Shows active table info, order ID, items count, bill total with payment badge, or an interactive table search/jump input when pressing `g`.
3. **Menu Search (Middle)**: Real-time fuzzy item search with instant filtering.
4. **Menu Catalogue (Body Left)**: Categorized item listings with prices and portion units.
5. **Bill & Cart View (Body Right)**: Active order line items, quantity, item prices, subtotal, discount, AC surcharge, GST breakdown, final total, and payment type badge.
6. **Recent Bills & KOTs (Lower Body)**: Dual side-by-side boxes showing recent settled bills (left) and latest kitchen order tickets (right). Use `←`/`→` to toggle focus between boxes.
7. **Notification Banner (Bottom)**: Operational feedback, validation warnings, and confirmation messages.
8. **Footer (Bottom Edge)**: Contextual hotkey reminders.

---

## 3. Navigation Basics

You can operate the entire system using only the keyboard:

| Action | Primary Key | Alternate Key |
| :--- | :--- | :--- |
| **Move Focus between Panels** | `Tab` | `Shift+Tab` (Backwards) |
| **Navigate Items / Rows** | `↓` (Down) / `↑` (Up) | `j` / `k` |
| **Navigate Tables in Floor Plan** | `→` (Right) / `←` (Left) | `l` / `h` |
| **Switch Directly to Dining Area** | `1` to `9` | — |
| **Cycle through Open Orders** | `]` (Next) / `[` (Prev) | — |
| **Focus Search Bar** | `/` | — |
| **Search / Jump to Any Table** | `g` | — |
| **Open Help Screen** | `?` | — |
| **Quit Application** | `q` | `Esc` (when not in a modal) |

---

## 4. Dine-In Order Workflow

Every table follows an intuitive restaurant lifecycle:

```text
[Ready] ──Enter──> [Taking Order] ──s──> [Serving] ──s──> [Ready for Bill]
   ▲                                                             │
   │                                                             p (Bill & Pay)
   │                                                             ▼
[Cleaning] <─────────────── c (Settle & Close) ──────────── [Bill Paid]
(~10 min timer or 'r')
```

### Step-by-step:
1. **Select a Table**: Use `Tab` to navigate to the **Tables** panel, then use `←` / `→` (or `h` / `l`) to select a table card.
2. **Open Order**: Press `Enter` on a green `[Ready]` table. The table changes to yellow `[Ordering]`.
3. **Add Food Items**:
   - Press `Tab` or `BackTab` to move to the **Menu** panel.
   - Use `↑` / `↓` to highlight a dish, or press `/` to search.
   - Press `Enter` or `Space` to add the dish to the table's cart.
4. **Advance Food Preparation**:
   - In the **Tables** panel, press `s` to advance the stage:
     - First press: Changes to blue `[Serving]` (kitchen is preparing / serving food).
     - Second press: Changes to cyan `[Ready for Bill]` (guests have finished eating).

---

## 5. Table Move, Transfer & Merge (`m`)

In a busy dining room, guests frequently ask to move from one table to another (e.g., relocating to an AC room, joining friends at another table, or moving outside to the garden):

1. Highlight the occupied table in the **Tables** panel.
2. Press `m` to open the **Move / Transfer & Merge Table** modal.
3. The dialog presents all other dining tables in the restaurant categorized into two actions:
   - **`[MOVE]` (Free / Ready Tables)**:
     - Selecting a free table transfers the entire check and cart items to the new table.
     - The original table immediately enters cleaning or turns free.
     - **Automatic Tax Recalculation**: If you move a party from a non-AC area (0% GST) into an AC room (AC surcharge + 5% GST), the system automatically updates the order's tax rates and AC charges in real time.
   - **`[MERGE]` (Occupied Tables)**:
     - Selecting a table that already has active guests merges all items, quantities, and notes from the current cart into the destination table's cart.
     - The source table is released and marked clean.
4. Navigate targets using `↑` / `↓` and press `Enter` to confirm, or `Esc` to cancel.

---

## 6. Take-Out Order Workflow

Take-out orders do not tie up physical dining room tables:

1. Press `t` from anywhere (except while typing in search or modals).
2. A new take-out order is created with a label like `TK1`, `TK2`, etc.
3. The cart automatically activates for that take-out order.
4. Add items in the **Menu** panel just like a dine-in order.
5. Take-out orders are calculated with the standard take-out GST rate (8%) without dining room AC surcharges.
6. Press `p` to generate the bill and proceed to payment.

---

## 7. Cart Management & Item Notes (`n`)

When an active order's cart is open:

- **Increase Quantity**: Press `+` or `=`.
- **Decrease Quantity**: Press `-`. Decreasing a quantity of `1` automatically removes the item from the cart.
- **Remove Line Item**: Press `x` or `Delete`.
- **Clear Cart**: Press `c` while in the Menu panel to clear all unpaid items.
- **Attach Item Cooking Notes (`n`)**:
  - Highlight any line item in the cart and press `n`.
  - The **Item Special Note** dialog opens.
  - Type custom customer cooking instructions (e.g. `"Less spicy, extra crisp"`, `"No onion, no garlic"`, `"Pack gravy separately"`).
  - Press `Enter` to save.
  - The note renders directly beneath the item name as `  ↳ <note>` in the cart, prints on Kitchen Order Tickets (KOT), prints on the final receipt, and is saved in SQLite records.
- **Cart Calculations**: Subtotal, applicable discounts, AC surcharge (for AC rooms), and GST are automatically recalculated in real time.

---

## 8. Kitchen Order Tickets (KOT) (`k`)

DineInTakeOut provides full back-of-house kitchen coordination:

1. After taking or updating guest orders, press `k` from the Menu or Cart panel.
2. A 42-column Kitchen Order Ticket is generated:
   - Includes Table / Take-out label, Area name, Order ID, and current timestamp.
   - Lists each dish and quantity ordered alongside attached cooking notes (`↳ Note: ...`).
   - Omits prices and taxes so kitchen staff can focus purely on order preparation.
3. The ticket is saved directly to the database (`kots` table) and immediately appears in the **KOT Bills** box in the bottom panel. No text files are written to disk, keeping the system clean.
4. If a CUPS thermal printer is configured, the ticket is instantly printed to the kitchen printer.
5. **Duplicate / Reprint Protection**:
   - The first KOT printed displays `*** KITCHEN ORDER TICKET (KOT) ***`.
   - Any subsequent print for the same order automatically displays `*** KITCHEN ORDER TICKET [REPRINT] ***` to prevent chefs from accidentally double-preparing dishes.
6. **Reprint from Recent Panel**: You can highlight any past KOT in the **KOT Bills** box and press `p`, `r`, or `Enter` to reprint it at any time.

---

## 9. Menu Item Stock & Out-of-Stock ("86") Toggling (`o`)

When the kitchen runs out of an ingredient or dish during a shift:

1. Navigate to the **Menu** catalogue panel.
2. Use `↑` / `↓` or `/` to highlight the item that is sold out.
3. Press `o` to toggle its availability.
4. The dish immediately displays a high-visibility red `[86 OUT]` badge next to its name.
5. Staff cannot accidentally add out-of-stock items: pressing `Enter` shows a notification `"Item '<name>' is currently out of stock [86]."`
6. Pressing `o` again restores the item to active stock once the kitchen restocks.
7. Availability persists in SQLite (`menu_items.is_available`) and exports via the 5th column (`Available`) of `menu.csv`.

---

## 10. Table Search & Quick Jump (`g`)

In busy restaurants with multiple rooms and gardens, switching between dozens of tables is effortless using the Table Search & Jump tool:

1. Press `g` from the Tables, Menu, or Cart panel.
2. The **Search Table** dialog appears in the top-right box.
3. Type any search term:
   - Table number (e.g. `2`, `14`)
   - Area name (e.g. `ac`, `main`, `garden`)
   - Status code or name (e.g. `ready`, `paid`, `dirty`, `clean`)
4. The list updates instantly, displaying each table's area, number, status badge (`[R]`, `[O]`, `[S]`, `[B]`, `[P]`, `[C]`), and live order total.
5. Use `↑` / `↓` to highlight the desired table.
6. Press `Enter` to jump directly to that table and switch to its active order.
7. Press `Esc` at any time to cancel and return to your previous view.

---

## 11. Billing & Customer Mobile Capture (`p` / `b`)

When guests request the check or take-out is ready:

1. Press `p` (in Cart or Menu) or `b` / `p` (in Tables).
2. The **Customer Mobile** popup appears:
   - Type the customer's 10-digit phone number.
   - Digits are formatted in a clean `5 + 5` display (e.g. `98765 43210`).
   - Press `Backspace` to correct any typing mistakes.
   - **Skip Option**: If the customer prefers not to provide a phone number, simply press `Enter` on an empty field.
3. The bill is generated:
   - Order status changes to `[Bill Paid]`.
   - The bill is permanently saved to the embedded Turso/SQLite database.
   - A text receipt is generated and stored in `bills/`.
   - If CUPS (`lp`) is configured, a receipt print is automatically initiated.

---

## 12. Applying Discount Offers

If your restaurant has active discount offers configured (e.g., Happy Hour 10%, Festival 15%):

1. Right after entering the mobile number, the **Apply Discount Offer** modal appears.
2. The modal lists all available offers alongside their discount percentages, with `0. None` as the first choice.
3. Select an offer:
   - Press the corresponding digit `0`–`9`, OR
   - Navigate with `↑` / `↓` and press `Enter`.
4. The discount is calculated on the subtotal before tax and displayed explicitly on the bill and receipt.

---

## 13. Payment Modes & Dynamic UPI QR Code (`q`)

The application natively supports all standard restaurant settlement options:

| Mode | Shortcut Keys | Typical Use Case |
| :--- | :--- | :--- |
| **Cash** | `1` or `c` | Physical currency payment |
| **UPI** | `2` or `u` | QR code, PhonePe, Google Pay, Paytm, BHIM |
| **Card** | `3` or `d` | POS debit/credit card swipe or tap |
| **Person Credit** | `4` | Regular customer tab / ledger account |
| **Have it on Hotel** | `5` | House complimentary, VIP, or manager comp |

### Dynamic On-Screen UPI QR Code (`q`)
When customers choose UPI, cashiers can generate an on-screen QR code:
1. In the payment mode modal, press `q`.
2. The **UPI Payment QR** modal pops up in high contrast:
   - Generates an official NPCI UPI string: `upi://pay?pa=<upi_id>&pn=DineInTakeOut&am=<amount>&cu=INR&tn=Bill_<id>`.
   - Renders a razor-sharp Unicode half-block QR code directly in the terminal window.
   - Displays the exact amount in rupees and the recipient UPI VPA.
3. The customer simply points their camera from PhonePe, Google Pay, Paytm, BHIM, or any banking app to complete payment instantly.
4. Press `Enter` to confirm payment and mark the bill paid via UPI, or `Esc` to exit.

---

## 14. Updating Payment Type on Paid Bills

A common operational challenge in restaurants: a bill is printed, and the guest changes their payment method (e.g., guest tries UPI, the transaction fails, and they hand cash or card instead).

### How to update payment type:
1. Ensure the order is in `[Bill Paid]` status.
2. Press `p` or `b`.
3. The **Update Payment Type** modal opens.
4. Press `u` (or `2`) for UPI, `c` (or `1`) for Cash, or `d` (or `3`) for Card.
5. The system immediately updates:
   - Active Bill Title: `Bill #4 — PAID via UPI ✔`
   - Bill Totals Breakdown: `Payment Type: UPI`
   - Saved Receipt: Header displays `Payment: UPI` and footer has `Paid via UPI`
   - Recent Bills List: Shows `[UPI]` tag
   - Table Details Panel: Shows `[Paid: UPI]`
   - SQLite Database: Updates the stored record in `orders.payment_mode`

---

## 15. Daily Sales Summary & Settlement Report / Z-Report (`z`)

At shift change or end-of-day closing:

1. Press `z` from anywhere in the application.
2. The **Daily Sales & Settlement Report** modal appears:
   - **Date & Order Counts**: Total orders billed, with breakdown between Dine-In and Take-Out checks.
   - **Financial Totals**: Gross Food Subtotal, Total Discounts given, AC Surcharges collected, Total GST collected, and Net Daily Revenue.
   - **Payment Method Breakdown**: Exact monetary collections and check counts for:
     - Cash collections
     - UPI collections
     - Card collections
     - Person credit entries
     - House complimentary orders
3. **Print & Save to DB (`p`)**:
   - Press `p` while viewing the Z-report to print a clean 42-column register tape slip via CUPS (`lp`).
   - The summary and breakdown are saved directly into the SQLite database (`z_reports` table). No text files are written to disk.
4. Press `Esc` to close the report.

---

## 16. Order Settlement & Table Cleaning

When guests leave the table:

1. Select the paid table in the **Tables** panel.
2. Press `c` to open the **Settle & Close Order** popup.
3. Confirm or select the final payment mode (`Enter` confirms the current mode).
4. The table enters **Cleaning** status (red badge):
   - Table status displays `[Cleaning (10m)]`.
   - A 10-minute auto-ready countdown timer begins.
   - After 10 minutes, the table automatically turns green `[Ready]` for the next guests.
5. **Fast Turnaround Override**: If staff clean the table immediately, press `r` on the table to mark it `[Ready]` instantly without waiting for the timer.

---

## 17. Reviewing Recent Bills & KOTs (Dual-Box Layout) & Historical Search (`/` or `s`)

### Dual-Box Bottom Panel
Navigate to the bottom panel by pressing `Tab` from the Tables or Cart panel. The bottom area is split horizontally into two side-by-side boxes:

1. **Box 1 (Left): Recent Bills**
   - Displays the latest settled bills with Bill ID, Table Label, Service type, Payment Mode (`[UPI]`, `[CASH]`, `[CARD]`), and Total Amount.
   - When active, the right-hand panel renders `" Previous bill — read only "` showing the exact formatted receipt breakdown.
2. **Box 2 (Right): KOT Bills**
   - Displays the latest 10 Kitchen Order Tickets with KOT ID, Table/Take-out Label, item count, timestamp, and reprint flag.
   - When active, the right-hand panel renders `" KOT Ticket — read only "` displaying the full 42-column KOT ticket with special cooking instructions.

### Switching Between Boxes & Reprints
- **Switch Active Box**: Press `←` / `→` (or `h` / `l`). The active box header shows `[Active]` with a distinct cyan border.
- **Select Order / Ticket**: Use `↑` / `↓` (or `j` / `k`) to move the selection highlight within the active box.
- **Reprint Bill or KOT**: Press `p`, `r`, or `Enter` on the selected item to reprint the bill or KOT ticket directly to the thermal printer.

### Historical Bill Search & Reprint (`/` or `s`)
Need to locate an earlier bill or look up a customer's receipt by mobile?
1. While in the **Recent bills & KOTs** panel, press `/` or `s`.
2. The **Search Historical Bills** modal opens.
3. Type any search term:
   - Bill ID (e.g. `1`, `42`)
   - Customer mobile number (e.g. `98765`)
4. The table displays matching results from SQLite with columns:
   `Bill #`, `Table`, `Service`, `Mobile`, `Total`, `Mode`, and `Date/Time`.
5. Use `↑` / `↓` to select the bill.
6. Press `Enter` or `p` to reprint the exact receipt directly to CUPS and save a fresh copy in `bills/`.
7. Press `Esc` to exit search.

---

## 18. Menu & Configuration Management (CSV `e` / `i`)

All restaurant configuration is managed via spreadsheet-compatible CSV files:

### Export Configuration (`e`)
1. From the **Menu** panel, press `e`.
2. The application exports five CSV files to your working directory:
   - `menu.csv`: Dish names, categories, units, prices, and stock availability (`Available`).
   - `table.csv` & `areas.csv`: Dining room types/sections, AC status, and table counts.
   - `offers.csv`: Promotional discount names and percentages.
   - `config.csv`: Hotel name (`RestaurantName`), physical address (`Address`), contact phone (`Contact`), registered GSTIN (`GSTNumber`), AC surcharge rate (`AcRate`), and UPI VPA ID (`UpiId`).
3. You will receive a confirmation notification: `"Exported config: ... items, ... table types, ... offers, settings."`

### Edit in Excel / LibreOffice
You can open and edit any of these CSV files with Microsoft Excel, Google Sheets, or LibreOffice Calc.
- **Empty Default Menu**: All dishes must be defined in `menu.csv`. There are no hardcoded built-in items.
- **Table Definitions**: Modify `table.csv` to adjust dining sections and table counts.
- **Hotel Details**: Change `RestaurantName`, `Address`, `Contact`, or `GSTNumber` in `config.csv` to reflect on printed bills and slips.

### Import Configuration (`i`)
1. After editing and saving the CSV files, return to the **Menu** panel in DineInTakeOut.
2. Press `i`.
3. The application validates the data, updates the embedded database, updates hotel details, and reconstructs the live floor plan.

---

## 19. Receipt Printing & Storage

- **Customer Bill Receipts**: Generated customer receipts are saved to `bills/` as:
  ```text
  bills/bill_<order_id>_<timestamp>.txt
  ```
  and permanently stored in the `orders` / `order_items` database tables.
- **Kitchen Order Tickets (KOT)**: Stored directly in the SQLite database (`kots` table) and retrievable via the **KOT Bills** box in the bottom panel. No disk text files are written.
- **Daily Sales Summaries (Z-Reports)**: Stored directly in the SQLite database (`z_reports` table). No disk text files are written.
- **Physical Thermal Printers**: If your system has CUPS configured (Linux/macOS) with a default printer:
  - DineInTakeOut automatically sends receipts and KOTs to `lp`.
  - The layout is formatted for standard 42-column 80mm thermal receipt paper.
  - If a printer is unavailable or offline, all records remain safely stored in the database without halting the app.

---

## 20. Automatic Daily Database Backups

To protect against system crashes or disk faults:
- On every application startup, DineInTakeOut automatically backs up the database to:
  ```text
  data/backups/billing_YYYY-MM-DD.db
  ```
- Backups run once per calendar day without slowing startup or overwriting existing day-start snapshots.
- Stored safely alongside the active database for instant restoration.

---

## 21. Keyboard Shortcuts Cheat Sheet

| Category | Key | Description |
| :--- | :--- | :--- |
| **Floor Plan** | `←` / `→` or `h` / `l` | Select table |
| | `1`–`9` | Jump directly to area |
| | `Enter` | Open order on ready table / switch to table |
| | `s` | Advance order stage (Taking order → Serving → Ready for bill) |
| | `g` | Search and jump to any table |
| | `m` | Move / transfer table or merge into occupied table |
| | `r` | Mark cleaning table as ready immediately |
| | `c` | Settle and close paid order |
| **Menu & Cart** | `/` | Focus search bar |
| | `↑` / `↓` or `j` / `k` | Scroll menu or cart items |
| | `Enter` or `Space` | Add highlighted item to cart |
| | `o` | Toggle item out-of-stock ("86") status |
| | `+` / `=` | Increase item quantity |
| | `-` | Decrease item quantity (removes at 0) |
| | `x` / `Delete` | Remove selected line from cart |
| | `n` | Add / edit cooking note on selected item |
| | `k` | Generate and print Kitchen Order Ticket (KOT) |
| | `c` | Clear unpaid cart (in Menu panel) |
| | `e` | Export config bundle to CSV |
| | `i` | Import config bundle from CSV |
| **Billing & Pay** | `p` or `b` | Generate bill / Update payment type on paid bill |
| | `1` or `c` | Select Cash payment mode |
| | `2` or `u` | Select UPI payment mode |
| | `3` or `d` | Select Card payment mode |
| | `4` | Select Person Credit payment mode |
| | `5` | Select Have it on Hotel payment mode |
| | `q` | Display dynamic on-screen UPI QR Code |
| **Reports & Search**| `z` | Daily Sales Summary & Settlement (Z-Report) |
| | `←` / `→` or `h` / `l` | Switch between Recent Bills and KOT Bills boxes |
| | `p` / `r` / `Enter` | Reprint selected bill or KOT ticket (in bottom panel) |
| | `/` or `s` | Search & reprint historical bills (in Recent Bills) |
| **General** | `Tab` / `BackTab` | Cycle panel focus |
| | `[` / `]` | Switch between active open orders |
| | `?` | Toggle help overlay |
| | `q` / `Esc` | Cancel modal / Exit application |

---

## 22. Frequently Asked Questions & Troubleshooting

#### Q: How do I switch between Dine-In orders and Take-Out orders?
**A**: Press `[` and `]` to cycle through all open orders, or click/select them from the floor plan and take-out chips.

#### Q: Can I change an order after billing?
**A**: Once a bill is generated, the item lines and amounts are frozen to guarantee financial auditability. However, you can freely update the **Payment Type** (UPI, Cash, Card) at any time before closing the table by pressing `p` or `b`.

#### Q: How do I configure my restaurant's UPI QR code?
**A**: Export your configuration with `e`, open `config.csv`, and set `UpiId` to your bank or Merchant VPA (e.g. `restaurant@okicici` or `9876543210@paytm`). Import back with `i`. When customers choose UPI, press `q` to display the QR code.

#### Q: What happens if the power goes out or the app closes?
**A**: All active orders, table states, cart items, customer mobile numbers, item notes, and paid bills are stored in the embedded SQLite database (`data/billing.db`). When you restart the app, everything resumes exactly where you left off. In addition, daily backup snapshots are created automatically in `data/backups/`.

#### Q: The screen appears cramped on my laptop. What should I do?
**A**: DineInTakeOut includes an automatic compact layout mode that activates when the terminal height is under 33 lines. To see all panels simultaneously including recent bill history, simply maximize your terminal window or reduce your terminal font size slightly.
