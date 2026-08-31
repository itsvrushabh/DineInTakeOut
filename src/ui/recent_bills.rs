//! Recent completed bills history panel rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{app::App, models::Focus, receipts::money};

pub fn render_recent_bills(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();
    let title_style = if app.focus == Focus::RecentBills {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    };

    let title = Span::styled(" Recent bills (latest 5) ", title_style);

    if app.recent_bills.is_empty() {
        lines.push(Line::styled(
            " No completed bills yet.",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        lines.extend(app.recent_bills.iter().enumerate().map(|(index, bill)| {
            let line = format!(
                " Bill #{:<4} {:<14} {:<10} {}",
                bill.id,
                bill.label,
                bill.service.label(),
                money(bill.total)
            );
            if app.focus == Focus::RecentBills && index == app.recent_bill_index {
                Line::styled(
                    line,
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::raw(line)
            }
        }));
    }

    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}
