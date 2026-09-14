//! End-of-Day Sales & Tax Analytics Modal Dashboard.

use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::{app::App, receipts::money, ui::centered_rect};

pub fn render_analytics(f: &mut Frame, app: &App) {
    let outer = centered_rect(80, 24, f.area());
    f.render_widget(Clear, outer);

    let analytics = match &app.sales_analytics {
        Some(a) => a,
        None => return,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" 📊 Sales & Tax Analytics — {} ", analytics.date),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(outer);
    f.render_widget(block, outer);

    let [kpi_area, mid_area, footer_area] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    // 1. KPI Cards Row
    let [kpi0, kpi1_rect, kpi2_rect, kpi3_rect] = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .areas(kpi_area);

    // KPI 1: Net Sales
    let kpi1 = vec![
        Line::styled("NET REVENUE", Style::default().fg(Color::DarkGray)),
        Line::styled(
            money(analytics.net_sales),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            format!("Gross: {}", money(analytics.gross_sales)),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    f.render_widget(
        Paragraph::new(kpi1).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        ),
        kpi0,
    );

    // KPI 2: Total Orders
    let kpi2 = vec![
        Line::styled("TOTAL ORDERS", Style::default().fg(Color::DarkGray)),
        Line::styled(
            format!("{}", analytics.total_orders),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            format!("Avg: {}", money(analytics.avg_bill_value)),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    f.render_widget(
        Paragraph::new(kpi2).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        kpi1_rect,
    );

    // KPI 3: Tax Collected
    let kpi3 = vec![
        Line::styled("GST TAX COLLECTED", Style::default().fg(Color::DarkGray)),
        Line::styled(
            money(analytics.total_tax),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            format!(
                "CGST: {} · SGST: {}",
                money(analytics.cgst),
                money(analytics.sgst)
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    f.render_widget(
        Paragraph::new(kpi3).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        kpi2_rect,
    );

    // KPI 4: Discounts
    let kpi4 = vec![
        Line::styled("DISCOUNTS GIVEN", Style::default().fg(Color::DarkGray)),
        Line::styled(
            money(analytics.total_discounts),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled("Savings provided", Style::default().fg(Color::DarkGray)),
    ];
    f.render_widget(
        Paragraph::new(kpi4).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        ),
        kpi3_rect,
    );

    // 2. Middle Area: Payment Mode Breakdown (Left) & Top 5 Dishes (Right)
    let [left_table_area, right_table_area] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
            .areas(mid_area);

    // Payment Mode Table
    let p_rows: Vec<Row> = analytics
        .payment_breakdown
        .iter()
        .map(|(mode, count, total)| {
            let pct = if analytics.net_sales > 0.0 {
                (total / analytics.net_sales) * 100.0
            } else {
                0.0
            };
            Row::new(vec![
                Cell::from(mode.clone()),
                Cell::from(Text_align_right(format!("{count}"))),
                Cell::from(Text_align_right(money(*total))),
                Cell::from(Text_align_right(format!("{pct:.1}%"))),
            ])
        })
        .collect();

    let p_table = Table::new(
        p_rows,
        [
            Constraint::Length(14),
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Tender", "Bills", "Total", "%"])
            .bold()
            .underlined(),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Payment Tender Distribution ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(p_table, left_table_area);

    // Top 5 Dishes Table
    let max_qty = analytics
        .top_items
        .first()
        .map(|(_, q, _)| *q)
        .unwrap_or(1)
        .max(1);

    let item_rows: Vec<Row> = analytics
        .top_items
        .iter()
        .map(|(name, qty, total)| {
            let bar_len = ((*qty as f64 / max_qty as f64) * 8.0).round() as usize;
            let bar = "█".repeat(bar_len);
            Row::new(vec![
                Cell::from(name.clone()),
                Cell::from(Text_align_right(format!("{qty}"))),
                Cell::from(Text_align_right(money(*total))),
                Cell::from(Span::styled(bar, Style::default().fg(Color::Yellow))),
            ])
        })
        .collect();

    let item_table = Table::new(
        item_rows,
        [
            Constraint::Fill(1),
            Constraint::Length(5),
            Constraint::Length(11),
            Constraint::Length(9),
        ],
    )
    .header(
        Row::new(vec!["Top Item", "Qty", "Revenue", "Trend"])
            .bold()
            .underlined(),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Best Selling Dishes ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(item_table, right_table_area);

    // 3. Footer
    let footer = Line::styled(
        "Esc / A / Enter: Close Analytics Dashboard",
        Style::default().fg(Color::DarkGray),
    );
    f.render_widget(
        Paragraph::new(footer).alignment(Alignment::Center),
        footer_area,
    );
}

#[allow(non_snake_case)]
fn Text_align_right(text: String) -> ratatui::text::Text<'static> {
    ratatui::text::Text::from(text).alignment(Alignment::Right)
}
