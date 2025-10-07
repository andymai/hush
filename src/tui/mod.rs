pub mod simple_app;
pub mod ui;
pub mod simple_handlers;
pub mod components;

pub use simple_app::{SimpleApp as App, AppMode, AppScreen};
pub use simple_handlers::handle_events;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, Terminal};
use std::io;

/// Initialize the terminal for TUI use
pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

/// Run the TUI application
pub async fn run_tui() -> Result<()> {
    let mut terminal = init_terminal()?;
    let mut app = App::new()?;
    
    let result = run_app(&mut terminal, &mut app).await;
    
    restore_terminal(&mut terminal)?;
    result
}

/// Main application loop
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;
        
        if handle_events(app).await? {
            break;
        }
    }
    Ok(())
}