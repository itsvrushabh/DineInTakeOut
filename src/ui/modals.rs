//! Modal dialogs: mobile number capture, payment mode confirmation, and offer selection.

use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{app::App, models::PaymentMode, receipts::money, ui::centered_rect};

pub fn render_mobile_entry(f: &mut Frame, app: &App) {
    let has_crm = app.customer_crm.as_ref().is_some_and(|c| c.visit_count > 0);
    let height = if has_crm { 10 } else { 7 };
    let area = centered_rect(54, height, f.area());
    f.render_widget(Clear, area);

    let complete = app.mobile_buffer.len() == 10;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Customer mobile ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(if complete {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Cyan)
        });

    let mut padded: String = app.mobile_buffer.clone();
    for _ in app.mobile_buffer.len()..10 {
        padded.push('_');
    }
    let display = format!("{} {}", &padded[..5], &padded[5..]);

    let mut lines = vec![
        Line::styled(
            "Enter customer mobile number (or Enter to skip)",
            Style::default().fg(Color::DarkGray),
        ),
        Line::from(Span::styled(
            format!("   {display}|   "),
            Style::default()
                .fg(if complete {
                    Color::Green
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        )),
    ];

    if let Some(crm) = &app.customer_crm {
        if crm.visit_count > 0 {
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "★ VIP Member: {} visits · Spent {}",
                    crm.visit_count,
                    money(crm.total_spent)
                ),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));
            if !crm.favorite_items.is_empty() {
                let favs = crm
                    .favorite_items
                    .iter()
                    .map(|(n, q)| format!("{n} ({q})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(Line::styled(
                    format!("Favorites: {favs}"),
                    Style::default().fg(Color::Yellow),
                ));
            }
        }
    }

    lines.push(Line::styled(
        "Enter: bill · Backspace: edit · Esc: cancel",
        Style::default().fg(Color::DarkGray),
    ));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical(vec![Constraint::Length(1); lines.len()])
        .flex(Flex::Center)
        .split(inner)
        .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line).alignment(Alignment::Center), rect);
    }
}

pub fn render_payment_mode(f: &mut Frame, app: &App) {
    let modes = PaymentMode::all();
    let area = centered_rect(54, (modes.len() as u16) + 7, f.area());
    f.render_widget(Clear, area);

    let title_text = if app.close_on_payment {
        " Settle & Close Order "
    } else {
        " Update Payment Type "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title_text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let header = match app.orders.get(app.active_order) {
        Some(order) => {
            if app.close_on_payment {
                format!(
                    "Close {} · Bill #{} — {}",
                    order.label,
                    order.id,
                    money(order.totals().total)
                )
            } else {
                format!(
                    "Bill #{} ({}) — {} · Paid",
                    order.id,
                    order.label,
                    money(order.totals().total)
                )
            }
        }
        None => String::from("No order selected"),
    };
    let mut lines = vec![Line::styled(
        header,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )];
    for (idx, mode) in modes.iter().enumerate() {
        let selected = idx == app.payment_mode_index;
        let marker = if selected { "▸ " } else { "  " };
        let shortcut = match mode {
            PaymentMode::Cash => "[1 / c]",
            PaymentMode::Upi => "[2 / u]",
            PaymentMode::Card => "[3 / d]",
            PaymentMode::Split => "[4 / s]",
            PaymentMode::PersonCredit => "[5]",
            PaymentMode::HaveItOnHotel => "[6]",
        };
        let line_text = format!("{marker}{:<8} {:<18}", shortcut, mode.display());
        lines.push(Line::from(Span::styled(
            line_text,
            if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }
    let footer_text = if app.close_on_payment {
        "↑↓/1–6/c,u,d,s: select · Enter: close · Esc: cancel"
    } else {
        "↑↓/1–6/c,u,d,s: select · Enter: update bill · Esc: cancel"
    };
    lines.push(Line::styled(
        footer_text,
        Style::default().fg(Color::DarkGray),
    ));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical(vec![Constraint::Length(1); lines.len()])
        .flex(Flex::Center)
        .split(inner)
        .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line).alignment(Alignment::Center), rect);
    }
}

pub fn render_split_payment(f: &mut Frame, app: &App) {
    let area = centered_rect(54, 13, f.area());
    f.render_widget(Clear, area);

    let (bill_total, cash, upi, card) = app.split_payment_totals();
    let total_paid = cash + upi + card;
    let balance = bill_total - total_paid;

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Split Payment Tender ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(if balance <= 0.01 {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Yellow)
        });

    let header = match app.orders.get(app.active_order) {
        Some(order) => format!(
            "Bill #{} ({}) — Total: {}",
            order.id,
            order.label,
            money(bill_total)
        ),
        None => "No order selected".to_string(),
    };

    let fields = [
        ("1. Cash Amount", &app.split_cash, 0),
        ("2. UPI Amount", &app.split_upi, 1),
        ("3. Card Amount", &app.split_card, 2),
    ];

    let mut lines = vec![
        Line::styled(
            header,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            "------------------------------------------------",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    for (label, val, idx) in fields {
        let active = app.split_field == idx;
        let prefix = if active { "▶ " } else { "  " };
        let display_val = if val.is_empty() {
            "0.00".to_string()
        } else {
            format!("{val}|")
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{prefix}{:<18}: ₹", label),
                if active {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                display_val,
                if active {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                        .underlined()
                } else {
                    Style::default()
                },
            ),
        ]));
    }

    lines.push(Line::styled(
        "------------------------------------------------",
        Style::default().fg(Color::DarkGray),
    ));
    lines.push(Line::from(vec![
        Span::raw(" Total Paid: "),
        Span::styled(
            money(total_paid),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   Balance Due: "),
        Span::styled(
            if balance <= 0.01 {
                "₹0.00 (Settled)".to_string()
            } else {
                money(balance)
            },
            if balance <= 0.01 {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            },
        ),
    ]));
    lines.push(Line::styled(
        "Tab/↑↓: switch field · a: auto-fill · Enter: confirm · Esc: cancel",
        Style::default().fg(Color::DarkGray),
    ));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical(vec![Constraint::Length(1); lines.len()])
        .flex(Flex::Center)
        .split(inner)
        .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line).alignment(Alignment::Center), rect);
    }
}

pub fn render_admin_modal(
    f: &mut Frame,
    title: &str,
    title_color: Color,
    items: &[String],
    selected: usize,
    input_label: Option<&str>,
) {
    let extra = if input_label.is_some() { 3 } else { 1 };
    let height = (items.len() as u16) + extra;
    let area = centered_rect(64, height.clamp(6, 24), f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(title_color));

    let mut lines: Vec<Line> = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let is_sel = idx == selected;
        let marker = if is_sel { "▸ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{marker}{item}"),
            if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }
    if let Some(label) = input_label {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("▶ {label}: {}_", "_"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical(vec![Constraint::Length(1); lines.len()])
        .flex(Flex::Center)
        .split(inner)
        .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line), rect);
    }
}

pub fn render_offer_select(f: &mut Frame, app: &App) {
    let mut items = vec![String::from("0. No discount")];
    for (idx, offer) in app.offers.iter().enumerate() {
        items.push(format!(
            "{}. {} — {:.0}% off",
            idx + 1,
            offer.name,
            offer.discount_percent
        ));
    }
    let title = match app.orders.get(app.active_order) {
        Some(order) => format!(
            "Apply offer — Bill #{} {}",
            order.id,
            money(order.totals().total)
        ),
        None => String::from("Apply offer"),
    };
    render_admin_modal(f, &title, Color::Cyan, &items, app.offer_index, None);
}
pub fn render_help(f: &mut Frame, _app: &App) {
    let area = centered_rect(64, 22, f.area());
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Key Bindings & POS Features (?) ")
        .border_style(Style::default().fg(Color::Cyan));
    let text = vec![
        Line::styled(
            "--- Floor Plan & Tables ---",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from("↑↓: area · ←→: table · 1-9: jump to area"),
        Line::from("g: jump to any table · Enter: open/switch table"),
        Line::from("s: next order stage · t: take-out · c: cancel/close"),
        Line::from("m: move / transfer / merge table"),
        Line::from("r: mark table ready (cleans dirty tables)"),
        Line::from(""),
        Line::styled(
            "--- Cart & Menu ---",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from("/: search menu · Enter: add item · o: toggle out-of-stock (86)"),
        Line::from("=/-: adjust qty · x/Delete: remove line · c: clear cart"),
        Line::from("n: add item special note · k: send Kitchen Order Ticket (KOT)"),
        Line::from(""),
        Line::styled(
            "--- Billing & Reports ---",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from("p/b: bill (capture mobile + offer + payment mode)"),
        Line::from("q: show on-screen dynamic UPI QR code (in payment modal)"),
        Line::from("z: Daily Sales Summary (Z-Report) & print"),
        Line::from("/ (in Recent Bills): search & reprint past bills"),
        Line::from("e/i: export / import full configuration (CSV)"),
        Line::from(""),
        Line::styled(
            "--- Navigation ---",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from("Tab/BackTab: cycle panel focus · [ / ]: switch active order"),
        Line::from("?: toggle this help modal · Esc: cancel / close"),
    ];
    let p = Paragraph::new(text).block(block).alignment(Alignment::Left);
    f.render_widget(p, area);
}

pub fn render_daily_report(f: &mut Frame, app: &App) {
    let area = centered_rect(66, 23, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Daily Sales Summary (Z-Report) ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Magenta));

    let summary = match &app.daily_report_summary {
        Some(s) => s,
        None => {
            let p = Paragraph::new("No daily report data available.").block(block);
            f.render_widget(p, area);
            return;
        }
    };

    let lines = vec![
        Line::styled(
            format!(
                " Date: {}   |   Total Orders: {}",
                summary.date, summary.total_orders
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(format!(
            " Breakdown: Dine-In: {}   ·   Take-Out: {}",
            summary.dine_in_orders, summary.takeout_orders
        )),
        Line::raw("────────────────────────────────────────────────────────────"),
        Line::from(format!(
            " Subtotal:         {:>14}",
            money(summary.subtotal)
        )),
        Line::styled(
            format!(" Total Discount:   {:>14}", money(-summary.discount)),
            Style::default().fg(Color::Yellow),
        ),
        Line::styled(
            format!(" AC Surcharges:    {:>14}", money(summary.ac_charge)),
            Style::default().fg(Color::Cyan),
        ),
        Line::from(format!(" GST Collected:    {:>14}", money(summary.tax))),
        Line::styled(
            format!(" NET SALES:        {:>14}", money(summary.total_sales)),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw("────────────────────────────────────────────────────────────"),
        Line::styled(
            " Payment Modes Breakdown:",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(format!(
            "   UPI:             {} orders   ·   {:>12}",
            summary.upi_count,
            money(summary.upi_total)
        )),
        Line::from(format!(
            "   Cash:            {} orders   ·   {:>12}",
            summary.cash_count,
            money(summary.cash_total)
        )),
        Line::from(format!(
            "   Card:            {} orders   ·   {:>12}",
            summary.card_count,
            money(summary.card_total)
        )),
        Line::from(format!(
            "   Person Credit:   {} orders   ·   {:>12}",
            summary.person_credit_count,
            money(summary.person_credit_total)
        )),
        Line::from(format!(
            "   Hotel Account:   {} orders   ·   {:>12}",
            summary.have_it_on_hotel_count,
            money(summary.have_it_on_hotel_total)
        )),
        Line::from(format!(
            "   Other / Unset:   {} orders   ·   {:>12}",
            summary.other_count,
            money(summary.other_total)
        )),
        Line::raw("────────────────────────────────────────────────────────────"),
        Line::styled(
            " [p] Print & Save Z-Report to DB   ·   [Esc/z/Enter] Close",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let inner = block.inner(area);
    f.render_widget(block, area);
    let p = Paragraph::new(lines).alignment(Alignment::Left);
    f.render_widget(p, inner);
}

pub fn render_bill_search(f: &mut Frame, app: &App) {
    use ratatui::widgets::{Cell, Row, Table};

    let area = centered_rect(78, 20, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Search & Reprint Past Bills ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let [search_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(1),
    ])
    .areas(block.inner(area));
    f.render_widget(block, area);

    let search_p = Paragraph::new(format!(
        " ▶ Search (Bill ID / Mobile / Label): {}_",
        app.bill_search_query
    ))
    .block(Block::default().borders(Borders::BOTTOM))
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(search_p, search_area);

    let rows: Vec<Row> = app
        .bill_search_results
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let is_sel = i == app.bill_search_index;
            let mode = b.payment_mode.map_or("—", |m| m.display());
            Row::new(vec![
                Cell::from(format!("#{}", b.id)),
                Cell::from(b.label.clone()),
                Cell::from(b.service.label()),
                Cell::from(if b.customer_mobile.is_empty() {
                    "—".to_string()
                } else {
                    b.customer_mobile.clone()
                }),
                Cell::from(ratatui::text::Text::from(money(b.total)).alignment(Alignment::Right)),
                Cell::from(mode),
                Cell::from(b.created_at.clone()),
            ])
            .style(if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(13),
            Constraint::Length(11),
            Constraint::Length(10),
            Constraint::Length(19),
        ],
    )
    .header(
        Row::new(vec![
            "Bill",
            "Table",
            "Service",
            "Mobile",
            "Total",
            "Mode",
            "Date/Time",
        ])
        .bold()
        .underlined(),
    );
    f.render_widget(table, list_area);

    let footer = Paragraph::new(" ↑↓/Tab: select   ·   Enter/p: reprint receipt   ·   Esc: close")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, footer_area);
}

pub fn render_upi_qr(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 24, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Scan UPI QR Code to Pay ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some((_uri, blocks, amount)) = app.upi_qr_uri_and_blocks() else {
        let p = Paragraph::new("No active bill available for UPI QR.").alignment(Alignment::Center);
        f.render_widget(p, inner);
        return;
    };

    let [header_area, qr_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(blocks.len() as u16),
        Constraint::Length(3),
    ])
    .areas(inner);

    let header_text = vec![
        Line::styled(
            format!("Pay Amount: {}", money(amount)),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(format!("UPI VPA: {}", app.upi_id)),
    ];
    f.render_widget(
        Paragraph::new(header_text).alignment(Alignment::Center),
        header_area,
    );

    let qr_lines: Vec<Line> = blocks
        .iter()
        .map(|l| Line::styled(l, Style::default().fg(Color::Black).bg(Color::White)))
        .collect();
    f.render_widget(
        Paragraph::new(qr_lines).alignment(Alignment::Center),
        qr_area,
    );

    let footer_text = vec![
        Line::raw(""),
        Line::styled(
            "Press [Esc], [q], or [Enter] to close",
            Style::default().fg(Color::DarkGray),
        ),
    ];
    f.render_widget(
        Paragraph::new(footer_text).alignment(Alignment::Center),
        footer_area,
    );
}

pub fn render_table_move(f: &mut Frame, app: &App) {
    use ratatui::widgets::{Cell, Row, Table};

    let targets = app.table_move_targets();
    let area = centered_rect(68, (targets.len() as u16 + 8).clamp(10, 22), f.area());
    f.render_widget(Clear, area);

    let source_area = app.selected_area_name();
    let source_num = app.selected_table_index + 1;
    let title = format!(" Move / Merge: {} Table {} ", source_area, source_num);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Yellow));

    let [header_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(block.inner(area));
    f.render_widget(block, area);

    let header_p = Paragraph::new(vec![
        Line::styled(
            " Select destination table (Empty = Transfer · Occupied = Merge carts):",
            Style::default().fg(Color::Cyan),
        ),
        Line::raw(""),
    ]);
    f.render_widget(header_p, header_area);

    let rows: Vec<Row> = targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_sel = i == app.table_move_target_index;
            let is_occupied = t.order_id.is_some();
            let action_badge = if is_occupied {
                Span::styled(
                    " [MERGE] ",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    " [TRANSFER] ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            };
            let ac_badge = if t.is_ac { " (AC)" } else { "" };
            let order_info = t.order_label.as_deref().unwrap_or("Empty");

            Row::new(vec![
                Cell::from(format!("{} T{}{}", t.area_name, t.table_number, ac_badge)),
                Cell::from(t.status.title()),
                Cell::from(order_info),
                Cell::from(Line::from(action_badge)),
            ])
            .style(if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec!["Destination", "Status", "Order", "Action"])
            .bold()
            .underlined(),
    );
    f.render_widget(table, list_area);

    let footer =
        Paragraph::new(" ↑↓/←→: choose target   ·   Enter: confirm move/merge   ·   Esc: cancel")
            .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, footer_area);
}

pub fn render_item_note(f: &mut Frame, app: &App) {
    let area = centered_rect(58, 8, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Item Special Instructions / Note ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Yellow));

    let item_name = app
        .orders
        .get(app.active_order)
        .and_then(|o| o.cart.get(o.cart_index).map(|l| l.name.clone()))
        .unwrap_or_else(|| "Selected Item".to_string());

    let lines = vec![
        Line::styled(
            format!("Item: {item_name}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(Span::styled(
            format!("▶ Note: {}_", app.item_note_buffer),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::styled(
            "Enter: save note   ·   Backspace: edit   ·   Esc: cancel",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .split(inner)
    .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line).alignment(Alignment::Center), rect);
    }
}
