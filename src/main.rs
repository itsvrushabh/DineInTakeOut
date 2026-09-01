use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyEventKind};

mod app;
mod config;
mod db;
mod models;
mod receipts;
mod ui;

use app::App;
use ui::ui;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    loop {
        app.tick_notification();
        let cleaned = app.tick_cleaning();
        if cleaned > 0 {
            app.notify(format!("{cleaned} table(s) cleaned and back to Ready."));
        }
        terminal.draw(|f| ui(f, &app))?;

        // Poll so the 10-minute banner can expire even without keypresses.
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && app.handle_key(key.code) {
                    return Ok(());
                }
            }
        }
    }
}
