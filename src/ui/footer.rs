//! Footer shortcut guide rendering.

use ratatui::{
    layout::Rect,
    text::{Line},
    widgets::{Paragraph},
    Frame,
};

use crate::app::App;

pub fn render_footer(f: &mut Frame, _app: &App, area: Rect) {
    f.render_widget(Paragraph::new(Line::default()), area);
}
