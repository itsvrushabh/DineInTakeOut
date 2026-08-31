//! Notification banner rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn render_notification(f: &mut Frame, app: &App, area: Rect) {
    if app.notification.is_empty() || area.height == 0 {
        return;
    }
    let msg = app.notification.as_str();

    let block = if area.height <= 2 {
        Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::Green))
    } else {
        Block::default()
            .borders(Borders::ALL)
            .title(" Notice ")
            .border_style(Style::default().fg(Color::Green))
    };

    f.render_widget(
        Paragraph::new(msg)
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .block(block),
        area,
    );
}
