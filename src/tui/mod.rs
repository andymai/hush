pub mod simple_app;
pub mod ui;
pub mod simple_handlers;
pub mod components;

pub use simple_app::{SimpleApp as App, AppMode, AppScreen};
pub use simple_handlers::handle_events;

use anyhow::Result;
use crossterm::{
    execute,
    cursor::Show,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, Terminal};
use std::io;

/// RAII Terminal guard that ensures cleanup on drop
struct TerminalGuard {
    _guard: (),
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        Ok(Self { _guard: () })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Always restore terminal state, ignore any errors
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = execute!(io::stdout(), Show);
    }
}

/// Initialize the terminal for TUI use
pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Run the TUI application
pub async fn run_tui() -> Result<()> {
    // Set up panic hook to ensure terminal cleanup on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Try to restore terminal state on panic
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = execute!(io::stdout(), Show);
        
        // Call the original panic hook
        original_hook(panic_info);
    }));
    
    let result = {
        // TerminalGuard ensures cleanup on drop, even if panic occurs
        let _guard = TerminalGuard::new()?;
        let mut terminal = init_terminal()?;
        let mut app = App::new()?;
        run_app(&mut terminal, &mut app).await
    }; // TerminalGuard is dropped here, automatically cleaning up
    
    // Restore original panic hook
    let _ = std::panic::take_hook();
    
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
