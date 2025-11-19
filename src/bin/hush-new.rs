use clap::{Parser, Subcommand};
use hush::adapters::{CpalAudioAdapter, HotkeyTriggerAdapter, WhisperAdapter};
#[cfg(target_os = "linux")]
use hush::adapters::text::X11TextAdapter;
#[cfg(target_os = "macos")]
use hush::adapters::text::MacOSTextAdapter;
use hush::application::hush_app::AppMode;
/// New Hush binary using trait-based architecture
///
/// This demonstrates the refactored architecture in action with real components
use hush::application::HushAppBuilder;
use hush::core::mocks::{MockAudioSource, MockInputTrigger, MockTextOutput, MockTranscriber};
use hush::{Config, Result};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(name = "hush-new")]
#[command(about = "🤫 Hush - Voice-to-Text (New Architecture)")]
#[command(version = "0.2.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Use mock components for testing
    #[arg(long)]
    mock: bool,

    /// Disable notifications
    #[arg(long)]
    no_notifications: bool,

    /// Custom config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Verbose logging
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Run in daemon mode with global hotkey
    Daemon,

    /// Run in one-shot mode (record once and exit)
    OneShot {
        /// Recording duration in seconds
        #[arg(short, long, default_value = "10")]
        duration: u64,

        /// Print to stdout instead of inserting
        #[arg(long)]
        print_only: bool,
    },

    /// Manual mode (stdin control)
    Manual,

    /// Show system information
    Info,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let log_level = match cli.verbose {
        0 => "hush=info",
        1 => "hush=debug",
        _ => "hush=trace",
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .init();

    info!("🤫 Hush Voice-to-Text (New Architecture) v0.2.0");

    // Load configuration
    let config = if let Some(path) = cli.config {
        Config::load_from_file(&path)?
    } else {
        Config::load()?
    };

    // Determine mode
    let mode = match cli.command {
        Some(Commands::Daemon) => AppMode::Daemon,
        Some(Commands::OneShot {
            duration,
            print_only,
        }) => AppMode::OneShot {
            duration_secs: duration,
            print_only,
        },
        Some(Commands::Manual) => AppMode::Manual,
        Some(Commands::Info) => {
            show_system_info(&config, cli.mock).await?;
            return Ok(());
        },
        None => AppMode::Daemon, // Default
    };

    // Build application with appropriate components
    let mut app = if cli.mock {
        info!("🧪 Using mock components for testing");
        create_mock_app(mode, !cli.no_notifications)?
    } else {
        info!("🔧 Using real hardware components");
        create_real_app(&config, mode, !cli.no_notifications).await?
    };

    // Run the application
    info!("🚀 Starting Hush application");
    app.run().await?;

    info!("👋 Hush exiting");
    Ok(())
}

/// Create app with mock components (for testing without hardware)
fn create_mock_app(mode: AppMode, notifications: bool) -> Result<hush::application::HushApp> {
    HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "This is a test transcription from mock components".to_string(),
            "Mock transcriber is working correctly".to_string(),
            "Hardware-free testing is amazing".to_string(),
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .with_mode(mode)
        .with_notifications(notifications)
        .build()
}

/// Create app with real hardware components
async fn create_real_app(
    config: &Config,
    mode: AppMode,
    notifications: bool,
) -> Result<hush::application::HushApp> {
    info!("Initializing real hardware components...");

    // Audio
    info!(
        "  📢 Audio: {} ({}Hz, {} channels)",
        config.audio.device.as_deref().unwrap_or("default"),
        config.audio.sample_rate,
        config.audio.channels
    );
    let audio = Box::new(CpalAudioAdapter::new(config.audio.device.as_deref())?);

    // Transcription
    info!(
        "  🤖 Transcriber: {} (GPU auto-detect)",
        config.transcription.model_size
    );
    // WhisperAdapter now auto-detects GPU (CUDA on Linux, Metal on macOS)
    let transcriber = Box::new(WhisperAdapter::new(&config.transcription.model_path).await?);

    // Text output
    #[cfg(target_os = "linux")]
    let text_output = {
        info!("  ⌨️  Text Output: X11");
        Box::new(X11TextAdapter::new()?)
    };

    #[cfg(target_os = "macos")]
    let text_output = {
        info!("  ⌨️  Text Output: macOS CGEvent");
        Box::new(MacOSTextAdapter::new()?)
    };

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    compile_error!("Unsupported platform");

    // Input trigger
    info!("  🎯 Input Trigger: {}", config.hotkey.combination);
    let input_trigger = Box::new(HotkeyTriggerAdapter::new(&config.hotkey.combination)?);

    // Build app
    HushAppBuilder::new()
        .with_audio(audio)
        .with_transcriber(transcriber)
        .with_text_output(text_output)
        .with_input_trigger(input_trigger)
        .with_mode(mode)
        .with_notifications(notifications)
        .build()
}

/// Show system information
async fn show_system_info(config: &Config, use_mock: bool) -> Result<()> {
    println!("🤫 Hush Voice-to-Text System Information");
    println!("========================================");
    println!();
    println!("Architecture: Trait-Based (v0.2.0)");
    println!(
        "Mock Mode: {}",
        if use_mock { "Enabled" } else { "Disabled" }
    );
    println!();

    if use_mock {
        println!("📦 Mock Components:");
        println!("  Audio: MockAudioSource");
        println!("  Transcriber: MockTranscriber");
        println!("  Text Output: MockTextOutput");
        println!("  Input Trigger: MockInputTrigger");
    } else {
        println!("🔧 Real Components:");
        println!(
            "  Audio: {} ({}Hz, {} ch)",
            config.audio.device.as_deref().unwrap_or("default"),
            config.audio.sample_rate,
            config.audio.channels
        );
        println!(
            "  Transcriber: Whisper {} (CUDA: {})",
            config.transcription.model_size, config.transcription.use_cuda
        );
        println!("  Text Output: X11 (enigo + xclip)");
        println!(
            "  Input Trigger: {} (global-hotkey)",
            config.hotkey.combination
        );
    }

    println!();
    println!("📊 Configuration:");
    println!("  Config File: config/default.toml");
    println!(
        "  Model Path: {}",
        config.transcription.model_path.display()
    );
    println!("  Language: {}", config.transcription.language);
    println!("  Beam Size: {}", config.transcription.beam_size);
    println!();

    println!("✨ Features:");
    println!("  ✅ Dependency Injection (trait-based)");
    println!("  ✅ Hardware-free testing (--mock flag)");
    println!("  ✅ Centralized state machine");
    println!("  ✅ Structured error handling");
    println!("  ✅ Runtime component swapping");
    println!();

    println!("🚀 Usage:");
    println!("  hush-new                    # Run in daemon mode");
    println!("  hush-new --mock             # Test without hardware");
    println!("  hush-new one-shot           # Record once and exit");
    println!("  hush-new manual             # Manual control mode");
    println!("  hush-new info               # Show this information");
    println!();

    Ok(())
}
