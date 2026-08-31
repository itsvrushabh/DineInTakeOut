//! Footer shortcut guide rendering.

use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    text::Line,
    widgets::Paragraph,
    Frame,
};

use crate::app::App;

pub fn render_footer(f: &mut Frame, _app: &App, area: Rect) {
    let help = vec![Line::from(" Press ? for help ")];
    f.render_widget(Paragraph::new(help).style(Style::default().dim()), area);
}
