//! Table details panel and interactive search/jump widget.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    app::App,
    models::{Focus, TableStatus, CLEANING_MINUTES},
    receipts::money,
};

pub fn render_table_info(f: &mut Frame, app: &App, area: Rect) {
    if app.focus == Focus::TableJump {
        render_table_search(f, app, area);
    } else {
        render_table_details(f, app, area);
    }
}

fn render_table_details(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == Focus::Tables;
    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title_style = if is_focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Table Details [g: Jump] ", title_style))
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let area_name = app.selected_area_name();
    if area_name.is_empty() {
        f.render_widget(Paragraph::new("No areas configured"), inner);
        return;
    }

    let is_ac = app.selected_area().is_some_and(|a| a.is_ac);
    let table_num = app.selected_table_index + 1;
    let table = app.selected_physical_table();
    let status = table.map_or(TableStatus::Ready, |t| t.status);
    let order = app.selected_table_order();

    let mut lines = Vec::new();

    // Line 1: Area + AC indicator
    let ac_label = if is_ac { " (AC)" } else { "" };
    lines.push(Line::from(vec![
        Span::styled("Area : ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}{}", area_name, ac_label),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));

    // Line 2: Table # and Status badge
    let status_color = status.color();
    let status_str = match status {
        TableStatus::Dirty => {
            let mins = table
                .and_then(|t| t.dirty_since)
                .map(|since| {
                    let elapsed = (chrono::Local::now() - since).num_minutes();
                    CLEANING_MINUTES.saturating_sub(elapsed).max(0)
                })
                .unwrap_or(CLEANING_MINUTES);
            format!("Cleaning ({}m)", mins)
        }
        TableStatus::Paid => {
            if let Some(ord) = order {
                if let Some(mode) = ord.payment_mode {
                    format!("Paid: {}", mode.label())
                } else {
                    "Paid".to_string()
                }
            } else {
                "Paid".to_string()
            }
        }
        s => s.title().to_string(),
    };
    lines.push(Line::from(vec![
        Span::styled("Table: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("T{} ", table_num),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("[{status_str}]"),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Line 3: Order ID / Label
    if let Some(ord) = order {
        lines.push(Line::from(vec![
            Span::styled("Order: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("#{} ({})", ord.id, ord.label),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        // Line 4: Bill total and items
        let totals = ord.totals();
        let item_count: u32 = ord.cart.iter().map(|l| l.qty).sum();
        let mut bill_spans = vec![
            Span::styled("Bill : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                money(totals.total),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if let Some(mode) = ord.payment_mode {
            bill_spans.push(Span::styled(
                format!(" [{}]", mode.label()),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        bill_spans.push(Span::styled(
            format!(
                " ({} item{})",
                item_count,
                if item_count == 1 { "" } else { "s" }
            ),
            Style::default().fg(Color::DarkGray),
        ));
        lines.push(Line::from(bill_spans));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Order: ", Style::default().fg(Color::DarkGray)),
            Span::styled("None (Ready for guest)", Style::default().fg(Color::Green)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Hint : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[Enter] to open order",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    // Bottom action hint
    lines.push(Line::from(vec![
        Span::styled(
            "[g] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Search/Jump table", Style::default().fg(Color::Yellow)),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_table_search(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Search Table (Esc: Back) ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();

    // Input line
    let prompt = Span::styled(
        "Search: ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    let query_span = if app.table_input.is_empty() {
        Span::styled(
            "<type #, area, status>|",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::styled(
            format!("{}|", app.table_input),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
    };
    lines.push(Line::from(vec![prompt, query_span]));

    let matches = app.matching_tables();
    if matches.is_empty() {
        lines.push(Line::from(Span::styled(
            "No tables found matching query.",
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from(Span::styled(
            "Try '3', 'ac', 'main', 'ready'...",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_items = (inner.height as usize).saturating_sub(2);
        let total = matches.len();

        let selected = app.table_search_index.min(total.saturating_sub(1));
        let start = if selected >= max_items && max_items > 0 {
            selected + 1 - max_items
        } else {
            0
        };
        let end = (start + max_items).min(total);

        for (offset, m) in matches[start..end].iter().enumerate() {
            let i = start + offset;
            let is_sel = i == selected;

            let cursor = if is_sel { "> " } else { "  " };
            let area_short = match m.area_name.as_str() {
                "Front Garden" => "Front",
                "AC Rooms" => "AC",
                "Main Hall" => "Main",
                "Back Garden" => "Back",
                other => other.split_whitespace().next().unwrap_or(other),
            };

            let status_code = match m.status {
                TableStatus::Ready => "R",
                TableStatus::Ordering => "O",
                TableStatus::Serving => "S",
                TableStatus::BillRequested => "B",
                TableStatus::Paid => "P",
                TableStatus::Dirty => "C",
            };

            let order_info = if let Some(tot) = m.order_total {
                format!(" ₹{tot:.0}")
            } else {
                String::new()
            };

            let text = format!(
                "{cursor}{area_short}-T{} [{status_code}]{order_info}",
                m.table_number
            );

            let style = if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(m.status.color())
            };

            lines.push(Line::styled(text, style));
        }

        if lines.len() < inner.height as usize {
            lines.push(Line::styled(
                format!("{}/{} matches · Enter: Jump", selected + 1, total),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}
