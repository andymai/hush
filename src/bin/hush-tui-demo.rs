use clap::Parser;
use anyhow::Result;

mod tui;

#[derive(Parser)]
#[command(name = "hush-tui-demo")]
#[command(about = "🤫 Hush TUI Demo - Terminal User Interface Demonstration")]
#[command(version = "0.1.0")]
struct Cli {
    /// Verbose logging
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _cli = Cli::parse();
    
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🤫 Starting Hush TUI Demo...");
    println!("This is a demonstration of the Terminal User Interface for Hush.");
    
    // Run TUI mode
    tui::run_tui().await
}