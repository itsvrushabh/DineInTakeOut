//! Floor plan rendering: area bars, table cards, take-out chips, and lifecycle legend.

use chrono::Local;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
    Frame,
};

use crate::{
    app::App,
    models::{Focus, Service, TableStatus, CLEANING_MINUTES},
};

pub fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let active = app.focus == Focus::Tables;
    let border_style = if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let title_style = if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(vec![
            Span::styled(" Floor plan ", title_style),
            Span::styled(" (Press ? for help) ", Style::default().fg(Color::DarkGray)),
        ]))
        .border_style(border_style);

    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let area_count = app.areas.len();
    let mut constraints = vec![Constraint::Length(1); area_count];
    constraints.push(Constraint::Length(1)); // TakeOut
    constraints.push(Constraint::Length(1)); // Legend
    let rows: Vec<Rect> = Layout::vertical(constraints).split(inner).to_vec();

    let now = Local::now();

    // One horizontal bar per configured area.
    for (bar_i, area_type) in app.areas.iter().enumerate() {
        if bar_i >= rows.len() {
            break;
        }
        let is_focused_bar = active && app.selected_area_index == bar_i;

        // Free-seating summary for the area label.
        let free = app
            .physical_tables
            .iter()
            .filter(|t| t.area == area_type.name && t.status == TableStatus::Ready)
            .count();
        let total = area_type.table_count;

        let (label_fg, label_mod) = if is_focused_bar {
            (Color::Yellow, Modifier::BOLD | Modifier::REVERSED)
        } else {
            (Color::DarkGray, Modifier::BOLD)
        };
        let ac_tag = if area_type.is_ac { " (AC)" } else { "" };
        let mut spans = vec![
            Span::styled(
                format!(" {:<16}{}", area_type.name, ac_tag),
                Style::default().fg(label_fg).add_modifier(label_mod),
            ),
            Span::styled(
                format!("{free}/{total} "),
                Style::default().fg(if free > 0 { Color::Green } else { Color::Red }),
            ),
        ];

        for table_num in 1..=total {
            let table = app
                .physical_tables
                .iter()
                .find(|t| t.area == area_type.name && t.number == table_num);

            let status = table.map_or(TableStatus::Ready, |t| t.status);
            let glyph = match status {
                TableStatus::Ready => 'R',
                TableStatus::Ordering => 'O',
                TableStatus::Serving => 'S',
                TableStatus::BillRequested => 'B',
                TableStatus::Paid => 'P',
                TableStatus::Dirty => 'C',
            };
            let status_color = status.color();

            // Dirty tables count down to auto-ready.
            let countdown = if status == TableStatus::Dirty {
                table.and_then(|t| t.dirty_since).map(|since| {
                    let elapsed = now - since;
                    let remaining = chrono::Duration::minutes(CLEANING_MINUTES) - elapsed;
                    let mins = remaining.num_minutes().max(0);
                    format!(" {mins}m")
                })
            } else {
                None
            };

            let is_selected = is_focused_bar && app.selected_table_index == table_num - 1;

            let cell_text = match &countdown {
                Some(mins) => format!("{glyph}{table_num}{mins}"),
                None => format!("{glyph}{table_num}"),
            };
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("[{cell_text}]"),
                if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD)
                },
            ));
        }

        f.render_widget(Line::from(spans), rows[bar_i]);
    }

    // Take-out bar (virtual tables).
    if area_count < rows.len() {
        let tk_active = active
            && app
                .orders
                .get(app.active_order)
                .is_some_and(|o| o.service == Service::TakeOut);
        let tk_label_style = if tk_active {
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        };
        let mut tk_spans = vec![Span::styled(format!(" {:<12}", "TakeOut"), tk_label_style)];
        let has_takeout = app.orders.iter().any(|o| o.service == Service::TakeOut);
        for (idx, order) in app.orders.iter().enumerate() {
            if order.service != Service::TakeOut {
                continue;
            }
            let is_active = app.active_order == idx;
            let style = if is_active {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Magenta)
            };
            let label = order.label.clone();
            tk_spans.push(Span::styled(format!(" [{label}] "), style));
        }
        if !has_takeout {
            tk_spans.push(Span::styled(
                " (none — press t)",
                Style::default().fg(Color::DarkGray),
            ));
        }
        f.render_widget(Line::from(tk_spans), rows[area_count]);
    }

    // Lifecycle legend.
    if area_count + 1 < rows.len() {
        let legend = Line::from(vec![
            Span::raw(" "),
            legend_span('R', "Ready", TableStatus::Ready.color()),
            legend_span('O', "Ordering", TableStatus::Ordering.color()),
            legend_span('S', "Serving", TableStatus::Serving.color()),
            legend_span('B', "For bill", TableStatus::BillRequested.color()),
            legend_span('P', "Paid", TableStatus::Paid.color()),
            legend_span(
                'C',
                &format!("Cleaning ({CLEANING_MINUTES}m)"),
                TableStatus::Dirty.color(),
            ),
        ]);
        f.render_widget(legend, rows[area_count + 1]);
    }
}

fn legend_span(glyph: char, label: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!("{glyph} {label}  "),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}
