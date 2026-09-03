use anyhow::Result;
use hush::cli::{Cli, CommandDispatcher};
use hush::logging::{LoggingConfig, LoggingSystem};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tracing::Level;
use tracing::{error, info, warn};

/// Static logging levels for production mode (v=0)
static PRODUCTION_LEVELS: Lazy<HashMap<String, Level>> = Lazy::new(|| {
    [
        ("audio".to_string(), Level::WARN),
        ("transcription".to_string(), Level::WARN),
        ("text".to_string(), Level::WARN),
        ("cli".to_string(), Level::INFO),
    ]
    .into_iter()
    .collect()
});

/// Static logging levels for normal verbose mode (v=1)
static VERBOSE_LEVELS: Lazy<HashMap<String, Level>> = Lazy::new(|| {
    [
        ("audio".to_string(), Level::INFO),
        ("transcription".to_string(), Level::INFO),
        ("text".to_string(), Level::INFO),
        ("cli".to_string(), Level::INFO),
    ]
    .into_iter()
    .collect()
});

/// Static logging levels for debug mode (v=2)
static DEBUG_LEVELS: Lazy<HashMap<String, Level>> = Lazy::new(|| {
    [
        ("audio".to_string(), Level::DEBUG),
        ("transcription".to_string(), Level::DEBUG),
        ("text".to_string(), Level::DEBUG),
        ("cli".to_string(), Level::DEBUG),
    ]
    .into_iter()
    .collect()
});

/// Static logging levels for trace mode (v>=3)
static TRACE_LEVELS: Lazy<HashMap<String, Level>> = Lazy::new(|| {
    [
        ("audio".to_string(), Level::TRACE),
        ("transcription".to_string(), Level::TRACE),
        ("text".to_string(), Level::TRACE),
        ("cli".to_string(), Level::TRACE),
    ]
    .into_iter()
    .collect()
});

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse_args();
    if let Some(path) = &cli.config_file {
        std::env::set_var(hush::config::paths::CONFIG_ENV, path);
    }

    // Initialize comprehensive logging system
    let logging_result = initialize_logging(&cli);
    match logging_result {
        Ok(_logging_system) => {
            info!("Hush - Voice-to-Text for Linux Developers - Logging Initialized");
        },
        Err(e) => {
            // Fall back to simple logging if comprehensive logging fails
            eprintln!("Failed to initialize comprehensive logging: {}", e);
            eprintln!("Falling back to basic logging...");
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
        "Hush application starting"
    );

    // Create command dispatcher with global configuration
    let dispatcher = CommandDispatcher::new(cli.config_file.clone(), !cli.no_notifications);

    // Dispatch the command with error handling
    let result = dispatcher.dispatch(cli.command).await;

    match &result {
        Ok(()) => {
            info!(
                session_id = %LoggingSystem::session_id(),
                "Hush application completed successfully"
            );
        },
        Err(e) => {
            error!(
                session_id = %LoggingSystem::session_id(),
                error = %e,
                error_chain = ?e.chain().collect::<Vec<_>>(),
                "Hush application failed"
            );
        },
    }

    result
}

fn initialize_logging(cli: &Cli) -> Result<LoggingSystem> {
    // Select static component levels based on verbosity
    let component_levels = match cli.verbose {
        0 => PRODUCTION_LEVELS.clone(),
        1 => VERBOSE_LEVELS.clone(),
        2 => DEBUG_LEVELS.clone(),
        _ => TRACE_LEVELS.clone(),
    };

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
