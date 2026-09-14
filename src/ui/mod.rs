//! User interface layout and rendering orchestration.

pub mod bill;
pub mod floor_plan;
pub mod footer;
pub mod menu;
pub mod modals;
pub mod notifications;
pub mod recent_bills;
pub mod search;
pub mod table_info;

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    Frame,
};

use crate::{
    app::App,
    models::Focus,
    ui::{
        bill::render_bill,
        floor_plan::render_tabs,
        footer::render_footer,
        menu::render_menu,
        modals::{render_help, render_mobile_entry, render_offer_select, render_payment_mode},
        notifications::render_notification,
        recent_bills::render_recent_bills,
        search::render_search,
        table_info::render_table_info,
    },
};

pub fn ui(f: &mut Frame, app: &App) {
    let compact = f.area().height < 33;
    let tabs_height = (app.areas.len() + 4).clamp(7, 16) as u16;
    let [top_row, search_area, body, recent_bills_area, footer_area, notification_area] =
        Layout::vertical(if compact {
            [
                Constraint::Length(tabs_height),
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(0),
                Constraint::Length(1),
                Constraint::Length(2),
            ]
        } else {
            [
                Constraint::Length(tabs_height),
                Constraint::Length(3),
                Constraint::Fill(1),
                Constraint::Length(7),
                Constraint::Length(1),
                Constraint::Length(3),
            ]
        })
        .areas(f.area());

    let table_box_width = 34.min(f.area().width / 2);
    let [tabs_area, table_info_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(table_box_width)])
            .areas(top_row);

    let [menu_area, cart_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(body);

    render_tabs(f, app, tabs_area);
    render_table_info(f, app, table_info_area);
    render_search(f, app, search_area);
    render_menu(f, app, menu_area);
    render_bill(f, app, cart_area);
    if !compact {
        render_recent_bills(f, app, recent_bills_area);
    }
    render_notification(f, app, notification_area);
    render_footer(f, app, footer_area);

    if app.focus == Focus::MobileEntry {
        render_mobile_entry(f, app);
    }
    if app.focus == Focus::PaymentMode {
        render_payment_mode(f, app);
    }
    if app.focus == Focus::OfferSelect {
        render_offer_select(f, app);
    }
    if app.show_help {
        render_help(f, app);
    }
}

/// A fixed-size rectangle centred inside `outer`.
pub fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(outer);
    Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0])[0]
}
