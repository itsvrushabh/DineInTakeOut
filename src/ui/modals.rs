//! Modal dialogs: mobile number capture, payment mode confirmation, and offer selection.

use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    app::App,
    models::PaymentMode,
    receipts::money,
    ui::centered_rect,
};

pub fn render_mobile_entry(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 7, f.area());
    f.render_widget(Clear, area);

    let complete = app.mobile_buffer.len() == 10;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Customer mobile ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(if complete {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Cyan)
        });

    let mut padded: String = app.mobile_buffer.clone();
    for _ in app.mobile_buffer.len()..10 {
        padded.push('_');
    }
    let display = format!("{} {}", &padded[..5], &padded[5..]);

    let lines = vec![
        Line::styled(
            "Enter customer mobile number (or Enter to skip)",
            Style::default().fg(Color::DarkGray),
        ),
        Line::from(Span::styled(
            format!("   {display}|   "),
            Style::default()
                .fg(if complete {
                    Color::Green
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        )),
        Line::styled(
            "Enter: bill · Backspace: edit · Esc: cancel",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .split(inner)
    .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line).alignment(Alignment::Center), rect);
    }
}

pub fn render_payment_mode(f: &mut Frame, app: &App) {
    let modes = PaymentMode::all();
    let area = centered_rect(48, (modes.len() as u16) + 7, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Mode of payment ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let header = match app.orders.get(app.active_order) {
        Some(order) => format!(
            "Close {} · Bill #{} — {}",
            order.label,
            order.id,
            money(order.totals().total)
        ),
        None => String::from("No order selected"),
    };
    let mut lines = vec![Line::styled(
        header,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )];
    for (idx, mode) in modes.iter().enumerate() {
        let selected = idx == app.payment_mode_index;
        let marker = if selected { "▸ " } else { "  " };
        let line_text = format!("{marker}{}. {:<16}", idx + 1, mode.display());
        lines.push(Line::from(Span::styled(
            line_text,
            if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }
    lines.push(Line::styled(
        "↑↓/1–5: select · Enter: confirm & close · Esc: cancel",
        Style::default().fg(Color::DarkGray),
    ));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical(vec![Constraint::Length(1); lines.len()])
        .flex(Flex::Center)
        .split(inner)
        .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line), rect);
    }
}

pub fn render_admin_modal(
    f: &mut Frame,
    title: &str,
    title_color: Color,
    items: &[String],
    selected: usize,
    input_label: Option<&str>,
) {
    let extra = if input_label.is_some() { 3 } else { 1 };
    let height = (items.len() as u16) + extra;
    let area = centered_rect(64, height.clamp(6, 24), f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(title_color));

    let mut lines: Vec<Line> = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let is_sel = idx == selected;
        let marker = if is_sel { "▸ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{marker}{item}"),
            if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }
    if let Some(label) = input_label {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("▶ {label}: {}_", "_"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows: Vec<Rect> = Layout::vertical(vec![Constraint::Length(1); lines.len()])
        .flex(Flex::Center)
        .split(inner)
        .to_vec();
    for (line, rect) in lines.into_iter().zip(rows) {
        f.render_widget(Paragraph::new(line), rect);
    }
}

pub fn render_offer_select(f: &mut Frame, app: &App) {
    let mut items = vec![String::from("0. No discount")];
    for (idx, offer) in app.offers.iter().enumerate() {
        items.push(format!(
            "{}. {} — {:.0}% off",
            idx + 1,
            offer.name,
            offer.discount_percent
        ));
    }
    let title = match app.orders.get(app.active_order) {
        Some(order) => format!(
            "Apply offer — Bill #{} {}",
            order.id,
            money(order.totals().total)
        ),
        None => String::from("Apply offer"),
    };
    render_admin_modal(f, &title, Color::Cyan, &items, app.offer_index, None);
}
pub fn render_help(f: &mut Frame, _app: &App) {
    let area = centered_rect(60, 10, f.area());
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Key Bindings (?) ");
    let text = vec![
        Line::from("Tab/Shift+Tab: Focus between panels"),
        Line::from("↑↓/j/k: Select item/area/bill"),
        Line::from("←→/h/l: Select table in area"),
        Line::from("1-9: Jump to table area"),
        Line::from("/: Focus menu search"),
        Line::from("Enter: Add item / Open table / Pay"),
        Line::from("Delete/x: Remove cart item"),
        Line::from("=/-: Adjust item quantity"),
        Line::from("p/b: Generate and save bill"),
        Line::from("t: Open take-out order"),
        Line::from("s: Advance order stage"),
        Line::from("c: Cancel/Close order"),
        Line::from("r: Mark cleaning table as Ready"),
        Line::from("e/i: Export/Import config (Menu/Area/Offers)"),
        Line::from("q/Esc: Quit/Close popups"),
        Line::from("?: Toggle help"),
    ];
    let p = Paragraph::new(text).block(block).alignment(Alignment::Left);
    f.render_widget(p, area);
}
