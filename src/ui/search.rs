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
    let title_style = if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            if active { " Search (/) " } else { " Search " },
            title_style,
        ))
        .border_style(if active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let cursor = if active { "|" } else { "" };
    let text = Text::from(Line::from(format!(" {}{}", app.search, cursor)));
    f.render_widget(Paragraph::new(text).block(block), area);

    // Show match count in the top-right corner.
    let count = app.visible_items().len();
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
}
