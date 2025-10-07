use anyhow::Result;
use hush::cli::{Cli, CommandDispatcher};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse_args();
    
    // Initialize logging based on verbosity
    cli.init_logging();
    
    info!("🤫 Hush - Voice-to-Text for Linux Developers");
    
    // Create command dispatcher with global configuration
    let dispatcher = CommandDispatcher::new(
        cli.config_file.clone(),
        !cli.no_notifications,
    );
    
    // Dispatch the command
    dispatcher.dispatch(cli.command).await?;
    
    Ok(())
}