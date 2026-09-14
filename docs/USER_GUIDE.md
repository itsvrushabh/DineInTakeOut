# DineInTakeOut — User Guide

A comprehensive manual for restaurant managers, cashiers, and waitstaff using the **DineInTakeOut** terminal billing and table management system.

---

## Table of Contents

1. [Overview & System Requirements](#1-overview--system-requirements)
2. [Terminal Screen Layout](#2-terminal-screen-layout)
3. [Navigation Basics](#3-navigation-basics)
4. [Dine-In Order Workflow](#4-dine-in-order-workflow)
5. [Take-Out Order Workflow](#5-take-out-order-workflow)
6. [Cart Management & Item Adjustments](#6-cart-management--item-adjustments)
7. [Table Search & Quick Jump (`g`)](#7-table-search--quick-jump-g)
8. [Billing & Customer Mobile Capture (`p` / `b`)](#8-billing--customer-mobile-capture-p--b)
9. [Applying Discount Offers](#9-applying-discount-offers)
10. [Payment Modes: UPI, Cash, Card](#10-payment-modes-upi-cash-card)
11. [Updating Payment Type on Paid Bills](#11-updating-payment-type-on-paid-bills)
12. [Order Settlement & Table Cleaning](#12-order-settlement--table-cleaning)
13. [Reviewing Recent Bills (History)](#13-reviewing-recent-bills-history)
14. [Menu & Configuration Management (CSV `e` / `i`)](#14-menu--configuration-management-csv-e--i)
15. [Receipt Printing & Local File Storage](#15-receipt-printing--local-file-storage)
16. [Keyboard Shortcuts Cheat Sheet](#16-keyboard-shortcuts-cheat-sheet)
17. [Frequently Asked Questions & Troubleshooting](#17-frequently-asked-questions--troubleshooting)

---

## 1. Overview & System Requirements

**DineInTakeOut** is a high-performance terminal user interface (TUI) designed for front-of-house restaurant operations. It coordinates dining room table states, take-out orders, fast item searching, receipt generation with tax calculations, and bill settlements.

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
│ Palak Paneer            ₹170.00  │ Butter Naan             2   40.00    80.00             │
│ Dal Makhani             ₹140.00  │ Paneer Butter Masala    2  180.00   360.00             │
│                                  │ ──────────────────────────────────────────────         │
│                                  │ Subtotal                             ₹440.00           │
│                                  │ GST (5.0%)                            ₹22.00           │
│                                  │ TOTAL                                ₹462.00           │
│                                  │ Payment Type                            UPI            │
├──────────────────────────────────┴────────────────────────────────────────────────────────┤
│ Recent Bills (latest 5):                                                                  │
│ Bill #4   Main-T4    Dine-In    [UPI]            ₹462.00                                  │
│ Bill #3   TK1        Take-Out   [CASH]           ₹250.00                                  │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│ Notification: Closed Main-T4 via UPI. Table 4 cleaning — auto-ready in 10 min.            │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│ Tab: Next panel · ↑↓: Select · Enter: Add/Open · p: Bill/Payment · c: Close · ?: Help    │
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

### Panel Overview
1. **Floor Plan (Top Left)**: Shows dining areas, table status cards, and active take-out chips.
2. **Table Details & Jump Box (Top Right)**: Shows active table info, order ID, items count, bill total with payment badge, or an interactive table search/jump input when pressing `g`.
3. **Menu Search (Middle)**: Real-time fuzzy item search with instant filtering.
4. **Menu Catalogue (Body Left)**: Categorized item listings with prices and portion units.
5. **Bill & Cart View (Body Right)**: Active order line items, quantity, item prices, subtotal, discount, AC surcharge, GST breakdown, final total, and payment type badge.
6. **Recent Bills (Lower Body)**: Quick-reference log of the last 5 settled bills with payment mode tags.
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

## 5. Take-Out Order Workflow

Take-out orders do not tie up physical dining room tables:

1. Press `t` from anywhere (except while typing in search or modals).
2. A new take-out order is created with a label like `TK1`, `TK2`, etc.
3. The cart automatically activates for that take-out order.
4. Add items in the **Menu** panel just like a dine-in order.
5. Take-out orders are calculated with the standard take-out GST rate (8%) without dining room AC surcharges.
6. Press `p` to generate the bill and proceed to payment.

---

## 6. Cart Management & Item Adjustments

When the active order's cart is focused (via `Tab` or pressing `Tab` from Menu):

- **Increase Quantity**: Press `+` or `=`.
- **Decrease Quantity**: Press `-`. Decreasing a quantity of `1` automatically removes the item from the cart.
- **Remove Line Item**: Press `x` or `Delete`.
- **Clear Cart**: Press `c` while in the Menu panel to clear all unpaid items.
- **Cart Calculations**: Subtotal, applicable discounts, AC surcharge (for AC rooms), and GST are automatically recalculated in real time.

---

## 7. Table Search & Quick Jump (`g`)

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

## 8. Billing & Customer Mobile Capture (`p` / `b`)

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

## 9. Applying Discount Offers

If your restaurant has active discount offers configured (e.g., Happy Hour 10%, Festival 15%):

1. Right after entering the mobile number, the **Apply Discount Offer** modal appears.
2. The modal lists all available offers alongside their discount percentages, with `0. None` as the first choice.
3. Select an offer:
   - Press the corresponding digit `0`–`9`, OR
   - Navigate with `↑` / `↓` and press `Enter`.
4. The discount is calculated on the subtotal before tax and displayed explicitly on the bill and receipt.

---

## 10. Payment Modes: UPI, Cash, Card

The application natively supports all standard restaurant settlement options:

| Mode | Shortcut Keys | Typical Use Case |
| :--- | :--- | :--- |
| **Cash** | `1` or `c` | Physical currency payment |
| **UPI** | `2` or `u` | QR code, PhonePe, Google Pay, Paytm, BHIM |
| **Card** | `3` or `d` | POS debit/credit card swipe or tap |
| **Person Credit** | `4` | Regular customer tab / ledger account |
| **Have it on Hotel** | `5` | House complimentary, VIP, or manager comp |

---

## 11. Updating Payment Type on Paid Bills

A common operational challenge in restaurants: a bill is printed, and the guest changes their payment method (e.g., guest tries UPI, the transaction fails, and they hand cash or card instead).

### How to update payment type:
1. Ensure the order is in `[Bill Paid]` status.
2. Press `p` or `b`.
3. The **Update Payment Type** modal opens:
   ```text
   ┌────────────────────────────────────────────────────┐
   │              Update Payment Type                   │
   │  Bill #4 (Main-T4) — ₹462.00 · Paid                │
   │                                                    │
   │    [1 / c]  Cash                                   │
   │  ▸ [2 / u]  UPI                                    │
   │    [3 / d]  Card                                   │
   │    [4]      Person credit                          │
   │    [5]      Have it on hotel                       │
   │                                                    │
   │  ↑↓/1–5/c,u,d: select · Enter: update bill · Esc   │
   └────────────────────────────────────────────────────┘
   ```
4. Press `u` (or `2`) for UPI, `c` (or `1`) for Cash, or `d` (or `3`) for Card.
5. The system immediately updates:
   - Active Bill Title: `Bill #4 — PAID via UPI ✔`
   - Bill Totals Breakdown: `Payment Type: UPI`
   - Saved Receipt: Header displays `Payment: UPI` and footer has `Paid via UPI`
   - Recent Bills List: Shows `[UPI]` tag
   - Table Details Panel: Shows `[Paid: UPI]`
   - SQLite Database: Updates the stored record in `orders.payment_mode`

---

## 12. Order Settlement & Table Cleaning

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

## 13. Reviewing Recent Bills (History)

To inspect past receipts during your shift:

1. Navigate to the **Recent bills** panel (press `Tab` from the Tables panel).
2. The panel lists the latest 5 completed bills with their Bill ID, Table Label, Service type, Payment Mode (`[UPI]`, `[CASH]`, `[CARD]`), and Total Amount.
3. Use `↑` / `↓` to scroll through recent bills.
4. The right-hand panel renders a **read-only view of the exact receipt** that was generated for that bill.

---

## 14. Menu & Configuration Management (CSV `e` / `i`)

All restaurant configuration is managed via spreadsheet-compatible CSV files:

### Export Configuration (`e`)
1. From the **Menu** panel, press `e`.
2. The application exports four CSV files to your working directory:
   - `menu.csv`: Dish names, categories, units, and prices.
   - `areas.csv`: Room names, AC status, and table counts.
   - `offers.csv`: Promotional discount names and percentages.
   - `config.csv`: Restaurant GSTIN and AC surcharge percentage rate.
3. You will receive a confirmation notification: `"Exported configuration bundle."`

### Edit in Excel / LibreOffice
You can open and edit any of these CSV files with Microsoft Excel, Google Sheets, or LibreOffice Calc.

### Import Configuration (`i`)
1. After editing and saving the CSV files, return to the **Menu** panel in DineInTakeOut.
2. Press `i`.
3. The application validates the data, updates the embedded database, and reconstructs the live floor plan.

---

## 15. Receipt Printing & Local File Storage

- **Local Storage**: Every generated bill is written to `bills/` as:
  ```text
  bills/bill_<order_id>_<timestamp>.txt
  ```
- **Physical Thermal Printers**: If your system has CUPS configured (Linux/macOS) with a default printer:
  - DineInTakeOut automatically sends the receipt to `lp`.
  - The receipt layout is formatted for standard 42-column 80mm thermal receipt paper.
  - If a printer is unavailable or offline, the bill is safely stored in the database and `bills/` without halting the app.

---

## 16. Keyboard Shortcuts Cheat Sheet

| Category | Key | Description |
| :--- | :--- | :--- |
| **Floor Plan** | `←` / `→` or `h` / `l` | Select table |
| | `1`–`9` | Jump directly to area |
| | `Enter` | Open order on ready table / switch to table |
| | `s` | Advance order stage (Taking order → Serving → Ready for bill) |
| | `g` | Search and jump to any table |
| | `r` | Mark cleaning table as ready immediately |
| | `c` | Settle and close paid order |
| **Menu & Cart** | `/` | Focus search bar |
| | `↑` / `↓` or `j` / `k` | Scroll menu or cart items |
| | `Enter` or `Space` | Add highlighted item to cart |
| | `+` / `=` | Increase item quantity |
| | `-` | Decrease item quantity (removes at 0) |
| | `x` / `Delete` | Remove selected line from cart |
| | `c` | Clear unpaid cart (in Menu panel) |
| | `e` | Export config bundle to CSV |
| | `i` | Import config bundle from CSV |
| **Billing & Pay** | `p` or `b` | Generate bill / Update payment type on paid bill |
| | `1` or `c` | Select Cash payment mode |
| | `2` or `u` | Select UPI payment mode |
| | `3` or `d` | Select Card payment mode |
| | `4` | Select Person Credit payment mode |
| | `5` | Select Have it on Hotel payment mode |
| **General** | `Tab` / `BackTab` | Cycle panel focus |
| | `[` / `]` | Switch between active open orders |
| | `?` | Toggle help overlay |
| | `q` / `Esc` | Cancel modal / Exit application |

---

## 17. Frequently Asked Questions & Troubleshooting

#### Q: How do I switch between Dine-In orders and Take-Out orders?
**A**: Press `[` and `]` to cycle through all open orders, or click/select them from the floor plan and take-out chips.

#### Q: Can I change an order after billing?
**A**: Once a bill is generated, the item lines and amounts are frozen to guarantee financial auditability. However, you can freely update the **Payment Type** (UPI, Cash, Card) at any time before closing the table by pressing `p` or `b`.

#### Q: What happens if the power goes out or the app closes?
**A**: All active orders, table states, cart items, customer mobile numbers, and paid bills are stored in the embedded SQLite database (`data/billing.db`). When you restart the app, everything resumes exactly where you left off.

#### Q: The screen appears cramped on my laptop. What should I do?
**A**: DineInTakeOut includes an automatic compact layout mode that activates when the terminal height is under 33 lines. To see all panels simultaneously including recent bill history, simply maximize your terminal window or reduce your terminal font size slightly.
