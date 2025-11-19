use anyhow::Result;
use hush::cli::{Cli, CommandDispatcher};
use hush::logging::{LoggingConfig, LoggingSystem};
use std::collections::HashMap;
use tracing::Level;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse_args();

    // Initialize comprehensive logging system
    let logging_result = initialize_logging(&cli);
    match logging_result {
        Ok(_logging_system) => {
            info!("🤫 Hush - Voice-to-Text for Linux Developers - Logging Initialized");
        },
        Err(e) => {
            // Fall back to simple logging if comprehensive logging fails
            eprintln!("⚠️  Failed to initialize comprehensive logging: {}", e);
            eprintln!("   Falling back to basic logging...");
            cli.init_logging();
            warn!(
                "Comprehensive logging initialization failed, using fallback: {}",
                e
            );
        },
    }

    // Log startup information
    info!(
        session_id = %LoggingSystem::session_id(),
        verbose_level = %cli.verbose,
        config_file = ?cli.config_file,
        notifications_enabled = %(!cli.no_notifications),
        "🚀 Hush application starting"
    );

    // Create command dispatcher with global configuration
    let dispatcher = CommandDispatcher::new(cli.config_file.clone(), !cli.no_notifications);

    // Dispatch the command with error handling
    let result = dispatcher.dispatch(cli.command).await;

    match &result {
        Ok(()) => {
            info!(
                session_id = %LoggingSystem::session_id(),
                "✅ Hush application completed successfully"
            );
        },
        Err(e) => {
            error!(
                session_id = %LoggingSystem::session_id(),
                error = %e,
                error_chain = ?e.chain().collect::<Vec<_>>(),
                "❌ Hush application failed"
            );
        },
    }

    result
}

fn initialize_logging(cli: &Cli) -> Result<LoggingSystem> {
    // Create logging configuration based on CLI arguments
    let mut component_levels = HashMap::new();

    // Set component-specific log levels based on verbosity
    match cli.verbose {
        0 => {
            // Production mode: minimal logging
            component_levels.insert("audio".to_string(), Level::WARN);
            component_levels.insert("transcription".to_string(), Level::WARN);
            component_levels.insert("text".to_string(), Level::WARN);
            component_levels.insert("cli".to_string(), Level::INFO);
        },
        1 => {
            // Normal verbose: key operations
            component_levels.insert("audio".to_string(), Level::INFO);
            component_levels.insert("transcription".to_string(), Level::INFO);
            component_levels.insert("text".to_string(), Level::INFO);
            component_levels.insert("cli".to_string(), Level::INFO);
        },
        2 => {
            // Debug mode: detailed information
            component_levels.insert("audio".to_string(), Level::DEBUG);
            component_levels.insert("transcription".to_string(), Level::DEBUG);
            component_levels.insert("text".to_string(), Level::DEBUG);
            component_levels.insert("cli".to_string(), Level::DEBUG);
        },
        _ => {
            // Trace mode: everything
            component_levels.insert("audio".to_string(), Level::TRACE);
            component_levels.insert("transcription".to_string(), Level::TRACE);
            component_levels.insert("text".to_string(), Level::TRACE);
            component_levels.insert("cli".to_string(), Level::TRACE);
        },
    }

    let config = LoggingConfig {
        component_levels,
        json_output: true,                    // Always enable structured logging
        console_output: true,                 // Human-readable console output
        performance_logging: cli.verbose > 0, // Enable performance metrics if verbose
        request_tracing: true,                // Always enable request correlation
        ..Default::default()
    };

    let logging_system = LoggingSystem::new(config)?;
    logging_system.initialize()
}
