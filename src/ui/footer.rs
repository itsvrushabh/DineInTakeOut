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
    let help = if area.height < 4 {
        vec![Line::from(" Tab: focus · /: search · p: bill · q: quit ")]
    } else {
        vec![
            Line::from(" Tab/Shift+Tab: focus · ↑↓/j/k: select · /: search · [:] switch order "),
            Line::from(
                " Floor plan: Enter open/switch/clean · s next stage · t takeout · c close paid ",
            ),
            Line::from(
                " Bill: +/- qty · x remove line · c clear · p bill (mobile + offer + payment mode) ",
            ),
            Line::from(" e/i: export/import full config (menu, areas, offers, GST, AC) to CSV · q/Esc quit "),
        ]
    };
    f.render_widget(Paragraph::new(help).style(Style::default().dim()), area);
}
