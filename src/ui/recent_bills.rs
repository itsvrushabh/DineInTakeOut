//! Recent completed bills and kitchen order tickets (KOT) panels rendering.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    app::App,
    models::{Focus, RecentTab},
    receipts::money,
};

pub fn render_recent_bills(f: &mut Frame, app: &App, area: Rect) {
    let [bills_area, kots_area] =
        Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)]).areas(area);

    let is_recent_focus = app.focus == Focus::RecentBills;
    let bills_active = is_recent_focus && app.recent_tab == RecentTab::Bills;
    let kots_active = is_recent_focus && app.recent_tab == RecentTab::Kots;

    // --- Box 1: Recent Bills ---
    let bills_title_style = if bills_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else if is_recent_focus {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    };

    let bills_border_style = if bills_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let bills_title = Span::styled(
        if bills_active {
            " Recent Bills [Active] "
        } else {
            " Recent Bills "
        },
        bills_title_style,
    );

    let mut bill_lines = Vec::new();
    if app.recent_bills.is_empty() {
        bill_lines.push(Line::styled(
            " No completed bills yet.",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        bill_lines.extend(app.recent_bills.iter().enumerate().map(|(index, bill)| {
            let mode_str = match bill.payment_mode {
                Some(m) => format!("[{}]", m.label()),
                None => String::new(),
            };
            let line = format!(
                " #{:<4} {:<9} {:<8} {:<7} {}",
                bill.id,
                bill.label,
                bill.service.label(),
                mode_str,
                money(bill.total)
            );
            if bills_active && index == app.recent_bill_index {
                Line::styled(
                    line,
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::raw(line)
            }
        }));
    }

    f.render_widget(
        Paragraph::new(bill_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(bills_title)
                .border_style(bills_border_style),
        ),
        bills_area,
    );

    // --- Box 2: KOT Bills ---
    let kots_title_style = if kots_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else if is_recent_focus {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    };

    let kots_border_style = if kots_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let kots_title = Span::styled(
        if kots_active {
            " KOT Bills [Active] "
        } else {
            " KOT Bills "
        },
        kots_title_style,
    );

    let mut kot_lines = Vec::new();
    if app.recent_kots.is_empty() {
        kot_lines.push(Line::styled(
            " No KOT tickets sent yet.",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        kot_lines.extend(app.recent_kots.iter().enumerate().map(|(index, kot)| {
            let reprint_tag = if kot.is_reprint { " [REPRINT]" } else { "" };
            let time_str = if kot.created_at.contains(' ') {
                kot.created_at.split(' ').nth(1).unwrap_or(&kot.created_at)
            } else {
                &kot.created_at
            };
            let line = format!(
                " KOT #{:<3} {:<7} {:>2} itm {:<8}{}",
                kot.id, kot.label, kot.item_count, time_str, reprint_tag
            );
            if kots_active && index == app.recent_kot_index {
                Line::styled(
                    line,
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::raw(line)
            }
        }));
    }

    f.render_widget(
        Paragraph::new(kot_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(kots_title)
                .border_style(kots_border_style),
        ),
        kots_area,
    );
}
