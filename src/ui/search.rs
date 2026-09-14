//! Search bar widget rendering.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{app::App, models::Focus};

pub fn render_search(f: &mut Frame, app: &App, area: Rect) {
    let active = app.focus == Focus::Search;
    let count = app.visible_items().len();

    if area.height >= 3 {
        let title_style = if active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                if active {
                    " [3] Search (/) "
                } else {
                    " [3] Search "
                },
                title_style,
            ))
            .border_style(if active {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let cursor = if active { "|" } else { "" };
        let text = if app.search.is_empty() && !active {
            Text::from(Line::from(vec![Span::styled(
                " type '/' or [3] to filter...",
                Style::default().fg(Color::DarkGray),
            )]))
        } else {
            Text::from(Line::from(format!(" {}{}", app.search, cursor)))
        };
        f.render_widget(Paragraph::new(text).block(block), area);

        // Show match count in the top-right corner.
        let info = Paragraph::new(format!("{} matches ", count))
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::DarkGray));
        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        f.render_widget(info, inner);
    } else {
        // Compact 1-line search bar inside menu box
        let cursor = if active { "|" } else { "" };
        let query_span = if app.search.is_empty() && !active {
            Span::styled(
                "type '/' or [3] to filter...",
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::styled(
                format!("{}{}", app.search, cursor),
                if active {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Cyan)
                },
            )
        };

        let spans = vec![
            Span::styled(
                " [3] 🔍 Search: ",
                if active {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            query_span,
            Span::styled(
                format!("  ({count} matches)"),
                Style::default().fg(Color::DarkGray),
            ),
        ];
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
