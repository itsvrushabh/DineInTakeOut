//! Fullscreen Kitchen Display System (KDS) live order monitor.

use chrono::Local;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub fn render_kds(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Clear, area);

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(area);

    // 1. Top Header
    let active_count = app.kds_kots.len();
    let header = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " 🍳 KITCHEN DISPLAY SYSTEM (KDS) — LIVE ORDER MONITOR ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let header_text = Line::from(vec![
        Span::raw("Active KOT Tickets: "),
        Span::styled(
            format!("{active_count}"),
            Style::default()
                .fg(if active_count > 0 {
                    Color::Yellow
                } else {
                    Color::Green
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  ·  Restaurant: "),
        Span::styled(
            &app.restaurant_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  ·  Time: "),
        Span::styled(
            Local::now().format("%H:%M:%S").to_string(),
            Style::default().fg(Color::Magenta),
        ),
    ]);
    f.render_widget(
        Paragraph::new(header_text)
            .block(header)
            .alignment(Alignment::Center),
        header_area,
    );

    // 2. Footer
    let footer_text = Line::from(vec![
        Span::styled(
            "Space / Enter: ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Bump Status (PENDING ➔ PREPARING ➔ READY ➔ SERVED)  ·  "),
        Span::styled(
            "↑↓/ws: ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Select Ticket  ·  "),
        Span::styled(
            "r/F5: ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Refresh  ·  "),
        Span::styled(
            "Esc / K: ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Exit KDS"),
    ]);
    f.render_widget(
        Paragraph::new(footer_text).alignment(Alignment::Center),
        footer_area,
    );

    // 3. Body
    if app.kds_kots.is_empty() {
        let empty_msg = vec![
            Line::raw(""),
            Line::styled(
                "✔ All orders have been prepared and served!",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                "Waiting for new tickets to arrive from dining floor...",
                Style::default().fg(Color::DarkGray),
            ),
        ];
        f.render_widget(
            Paragraph::new(empty_msg)
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)),
                ),
            body_area,
        );
        return;
    }

    // Render tickets as grid columns
    let num_tickets = app.kds_kots.len();
    let cols = 3.min(num_tickets.max(1));
    let col_constraints = vec![Constraint::Ratio(1, cols as u32); cols];
    let col_areas = Layout::horizontal(col_constraints).split(body_area);

    for (i, kot) in app.kds_kots.iter().take(cols).enumerate() {
        let is_selected = i == app.kds_index;
        let target_rect = col_areas[i];

        // Determine elapsed time and urgency color
        let (elapsed_desc, urgency_style) = parse_elapsed(&kot.created_at);

        let status_color = match kot.status.as_str() {
            "PREPARING" => Color::Yellow,
            "READY" => Color::Green,
            "SERVED" => Color::Magenta,
            _ => Color::Cyan, // PENDING
        };

        let card_title = format!(" [{}] {} · #{} ", kot.status, kot.label, kot.id);

        let border_color = if is_selected {
            Color::Yellow
        } else {
            status_color
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                card_title,
                Style::default()
                    .fg(if is_selected {
                        Color::Yellow
                    } else {
                        status_color
                    })
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(border_color));

        let mut lines = Vec::new();

        // Area & Timer Row
        let area_name = if kot.area.is_empty() {
            "Take-Out"
        } else {
            kot.area.as_str()
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("Area: {:<12}", area_name),
                Style::default().fg(Color::White),
            ),
            Span::styled(format!("Elapsed: {elapsed_desc}"), urgency_style),
        ]));
        lines.push(Line::from(vec![
            Span::raw("Items: "),
            Span::styled(
                format!("{}", kot.item_count),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("   Created: "),
            Span::styled(&kot.created_at, Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::styled(
            "----------------------------------------------",
            Style::default().fg(Color::DarkGray),
        ));

        // Extract items from ticket_text
        let mut in_items = false;
        for raw_line in kot.ticket_text.lines() {
            if raw_line.contains("ITEM") && raw_line.contains("QTY") {
                in_items = true;
                continue;
            }
            if in_items && raw_line.starts_with("---") {
                continue;
            }
            if in_items && raw_line.contains("Send to Kitchen") {
                break;
            }
            if in_items && !raw_line.trim().is_empty() {
                if raw_line.contains("↳") {
                    lines.push(Line::styled(
                        format!("   {}", raw_line.trim()),
                        Style::default().fg(Color::Yellow),
                    ));
                } else {
                    lines.push(Line::styled(
                        raw_line.to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }

        f.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            target_rect,
        );
    }
}

fn parse_elapsed(created_at: &str) -> (String, Style) {
    let time_part = if created_at.contains(' ') {
        created_at.split(' ').nth(1).unwrap_or(created_at)
    } else {
        created_at
    };

    let now = Local::now().time();
    if let Ok(created_time) = chrono::NaiveTime::parse_from_str(time_part, "%H:%M:%S") {
        let diff = now - created_time;
        let mins = diff.num_minutes();
        let secs = (diff.num_seconds() % 60).abs();
        if mins < 10 {
            (
                format!("{mins}m {secs}s (Normal)"),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else if mins < 20 {
            (
                format!("{mins}m {secs}s (Warning)"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                format!("{mins}m {secs}s (URGENT)"),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        }
    } else {
        (time_part.to_string(), Style::default().fg(Color::Green))
    }
}
