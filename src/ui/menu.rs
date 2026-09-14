//! Menu item catalogue table rendering.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::{app::App, models::Focus, receipts::money, ui::search::render_search};

pub fn render_menu(f: &mut Frame, app: &App, area: Rect) {
    let is_search = app.focus == Focus::Search;
    let is_menu = app.focus == Focus::Menu;
    let is_active = is_search || is_menu;

    let title = if is_search {
        " [4] Menu · [3] Search (Enter/↓: select item · Esc: exit search) "
    } else if is_menu {
        " [4] Menu (/: search · Enter: add · ←/→: cat · o: stock) "
    } else {
        " [4] Menu "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            if is_active {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ))
        .border_style(if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let search_h = if inner.height >= 12 { 3 } else { 1 };
    let [search_area, cat_area, table_area] = Layout::vertical([
        Constraint::Length(search_h),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);

    render_search(f, app, search_area);

    // Build category pill bar
    let mut spans = Vec::new();
    let has_prev = app.selected_category_index > 0;
    let has_next = app.selected_category_index + 1 < app.categories.len();

    spans.push(Span::styled(
        if has_prev { " ◀ " } else { "   " },
        if has_prev {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    ));

    for (idx, cat) in app.categories.iter().enumerate() {
        let is_selected = idx == app.selected_category_index;
        let dist = (idx as isize - app.selected_category_index as isize).abs();
        if dist > 2 && app.categories.len() > 5 {
            continue;
        }
        if is_selected {
            spans.push(Span::styled(
                format!("[ {cat} ]"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {cat} "),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    spans.push(Span::styled(
        if has_next { " ▶ " } else { "   " },
        if has_next {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    ));

    f.render_widget(Paragraph::new(Line::from(spans)), cat_area);

    let vis = app.visible_items();

    let rows: Vec<Row> = vis
        .iter()
        .enumerate()
        .map(|(row_i, &idx)| {
            let it = &app.items[idx];
            let selected = row_i == app.menu_index && app.focus == Focus::Menu;
            let item_cell = if it.is_available {
                Cell::from(it.name.clone())
            } else {
                Cell::from(Line::from(vec![
                    Span::styled(it.name.clone(), Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                    Span::styled(
                        "[86 OUT]",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                ]))
            };
            Row::new(vec![
                item_cell,
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
    .header(Row::new(vec!["Item", "Unit", "Price"]).bold().underlined());

    f.render_widget(table, table_area);
}
