//! Menu item catalogue table rendering.

use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Span, Text},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::{app::App, models::Focus, receipts::money};

pub fn render_menu(f: &mut Frame, app: &App, area: Rect) {
    let vis = app.visible_items();

    let rows: Vec<Row> = vis
        .iter()
        .enumerate()
        .map(|(row_i, &idx)| {
            let it = &app.items[idx];
            let selected = row_i == app.menu_index && app.focus == Focus::Menu;
            Row::new(vec![
                Cell::from(it.name.clone()),
                Cell::from(Text::from(it.unit.clone()).alignment(Alignment::Right)),
                Cell::from(Text::from(money(it.price)).alignment(Alignment::Right)),
            ])
            .style(if selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Percentage(30),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec!["Item", "Unit", "Price"]).bold().underlined())
    .block(Block::default().borders(Borders::ALL).title(Span::styled(
        " Menu ",
        if app.focus == Focus::Menu {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    )));

    f.render_widget(table, area);
}
