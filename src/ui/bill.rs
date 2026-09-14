//! Bill and cart table rendering with non-overlapping totals breakdown.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::{
    app::App,
    models::{Focus, OrderStatus, RecentTab},
    receipts::money,
};

pub fn render_bill(f: &mut Frame, app: &App, area: Rect) {
    if app.focus == Focus::RecentBills {
        match app.recent_tab {
            RecentTab::Bills => {
                let block = Block::default().borders(Borders::ALL).title(Span::styled(
                    " [5] Previous bill — read only ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                let receipt = app
                    .recent_bills
                    .get(app.recent_bill_index)
                    .map(|bill| bill.receipt.as_str())
                    .unwrap_or("No saved bills to review.");
                f.render_widget(Paragraph::new(receipt).block(block), area);
                return;
            }
            RecentTab::Kots => {
                let block = Block::default().borders(Borders::ALL).title(Span::styled(
                    " [5] KOT Ticket — read only ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                let ticket = app
                    .recent_kots
                    .get(app.recent_kot_index)
                    .map(|kot| kot.ticket_text.as_str())
                    .unwrap_or("No KOT tickets to review.");
                f.render_widget(Paragraph::new(ticket).block(block), area);
                return;
            }
        }
    }

    if app.orders.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " [5] Cart ",
                if app.focus == Focus::Cart {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ))
            .border_style(if app.focus == Focus::Cart {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });
        f.render_widget(Paragraph::new("No active order").block(block), area);
        return;
    }

    let order = app.order();
    let paid = matches!(order.status, OrderStatus::Paid);
    let serving = matches!(order.status, OrderStatus::Serving);
    let bill_ready = matches!(order.status, OrderStatus::BillRequested);
    let title = if paid {
        if let Some(mode) = order.payment_mode {
            format!(
                " [5] Bill #{} — PAID via {} ✔ ({} / {}) ",
                order.id,
                mode.display().to_uppercase(),
                order.label,
                order.service.label()
            )
        } else {
            format!(
                " [5] Bill #{} — PAID ✔ ({} / {}) ",
                order.id,
                order.label,
                order.service.label()
            )
        }
    } else if bill_ready {
        format!(
            " [5] Bill #{} — READY FOR BILL ⏳ ({} / {}) ",
            order.id,
            order.label,
            order.service.label()
        )
    } else if serving {
        format!(
            " [5] Bill #{} — SERVING ★ ({} / {}) ",
            order.id,
            order.label,
            order.service.label()
        )
    } else {
        format!(
            " [5] Bill #{} — {} / {} ",
            order.id,
            order.label,
            order.service.label()
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            if paid {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if app.focus == Focus::Cart {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ))
        .border_style(if app.focus == Focus::Cart {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    // Render outer block frame
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    // Build totals lines
    let totals = order.totals();
    let mut total_lines = vec![Line::from(Span::raw(format!(
        " {:<16}{:>14}",
        "Subtotal",
        money(totals.subtotal)
    )))];

    if totals.discount > 0.0 {
        total_lines.push(Line::from(Span::styled(
            format!(
                " {:<16}{:>14}",
                format!("Discount ({:.0}%)", order.discount_percent),
                money(-totals.discount)
            ),
            Style::default().fg(Color::Yellow),
        )));
    }

    if totals.ac_charge > 0.0 {
        total_lines.push(Line::from(Span::styled(
            format!(
                " {:<16}{:>14}",
                format!("AC charge ({:.0}%)", order.ac_surcharge() * 100.0),
                money(totals.ac_charge)
            ),
            Style::default().fg(Color::Cyan),
        )));
    }

    total_lines.push(Line::from(Span::raw(format!(
        " {:<16}{:>14}",
        format!("GST ({:.0}%)", totals.gst_rate * 100.0),
        money(totals.gst)
    ))));
    total_lines.push(Line::from(Span::styled(
        format!(" {:<16}{:>14}", "TOTAL", money(totals.total)),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    if let Some(mode) = order.payment_mode {
        total_lines.push(Line::from(Span::styled(
            format!(
                " {:<16}{:>14}",
                "Payment Type",
                mode.display().to_uppercase()
            ),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
    }

    let totals_height = total_lines.len() as u16;

    // Split inner area into table area and totals area to guarantee NO overlaps
    let [table_area, totals_area] =
        Layout::vertical([Constraint::Min(2), Constraint::Length(totals_height)]).areas(inner);

    let rows: Vec<Row> = order
        .cart
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let selected = i == order.cart_index && app.focus == Focus::Cart;
            let has_note = line.note.is_some();
            let item_cell = if let Some(note) = &line.note {
                Cell::from(Text::from(vec![
                    Line::from(line.name.clone()),
                    Line::from(Span::styled(
                        format!("  ↳ {note}"),
                        Style::default().fg(Color::Yellow),
                    )),
                ]))
            } else {
                Cell::from(line.name.clone())
            };
            Row::new(vec![
                item_cell,
                Cell::from(Text::from(format!("{}", line.qty)).alignment(Alignment::Right)),
                Cell::from(Text::from(money(line.unit_price)).alignment(Alignment::Right)),
                Cell::from(Text::from(money(line.total())).alignment(Alignment::Right)),
            ])
            .height(if has_note { 2 } else { 1 })
            .style(if selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let widths = [
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Length(9),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(
        Row::new(vec!["Item", "Qty", "Each", "Total"])
            .bold()
            .underlined(),
    );

    f.render_widget(table, table_area);
    f.render_widget(Paragraph::new(total_lines), totals_area);
}
