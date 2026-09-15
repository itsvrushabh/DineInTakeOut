use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyEventKind};

use dinein_takeout_billing::{app::App, ui::ui};

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
        for effect in app.drain_effects() {
            if let dinein_takeout_billing::app::AppEffect::SaveFile { path, content } = effect {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&path, content);
            }
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
