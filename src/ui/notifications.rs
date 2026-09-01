//! Notification banner rendering.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn render_notification(f: &mut Frame, app: &App, area: Rect) {
    if app.notifications.is_empty() || area.height == 0 {
        return;
    }

    let lines: Vec<Line> = app
        .notifications
        .iter()
        .map(|(msg, _)| {
            Line::styled(
                msg,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Notice ")
        .border_style(Style::default().fg(Color::Green));

    f.render_widget(Paragraph::new(lines).block(block), area);
}
