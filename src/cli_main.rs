// CLI module for unified command structure
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hush")]
#[command(about = "🤫 Hush - Voice-to-Text for Linux Developers")]
#[command(version = "0.1.0")]
#[command(author = "Andy")]
#[command(long_about = "Fast, accurate voice-to-text for Linux developers using local Whisper models.\nSupports global hotkeys, multiple input methods, and UInput text insertion.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging (-v, -vv, -vvv for increasing verbosity)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Custom configuration file path
    #[arg(short = 'c', long = "config-file", global = true)]
    pub config_file: Option<PathBuf>,

    /// Disable desktop notifications
    #[arg(long, global = true)]
    pub no_notifications: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start voice-to-text with TUI interface (default mode)
    Start {
        /// Run in background as daemon
        #[arg(short, long)]
        daemon: bool,

        /// Try to run with elevated privileges for better hotkey access
        #[arg(long)]
        elevated: bool,

        /// Use CLI/hotkey mode instead of TUI interface
        #[arg(long)]
        cli: bool,
    },

    /// Record once and exit (no hotkeys)
    Record {
        /// Maximum recording duration in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,

        /// Don't insert text, just print to stdout
        #[arg(short, long)]
        print_only: bool,

        /// Save audio to file for debugging
        #[arg(long)]
        save_audio: Option<PathBuf>,
    },

    /// Manual recording mode (interactive prompts)
    Manual {
        /// Number of recordings to make
        #[arg(short, long, default_value = "1")]
        count: u32,
    },

    /// Setup and configuration commands
    Setup {
        #[command(subcommand)]
        setup_command: SetupCommands,
    },

    /// Testing and diagnostic commands
    Test {
        #[command(subcommand)]
        test_command: TestCommands,
    },

    /// Model management commands
    Models {
        #[command(subcommand)]
        model_command: ModelCommands,
    },

    /// Show application status and configuration
    Status {
        /// Show detailed configuration
        #[arg(long)]
        config: bool,

        /// Show device information
        #[arg(long)]
        devices: bool,

        /// Check all systems
        #[arg(long)]
        full: bool,
    },

    /// Install desktop integration (autostart, .desktop file)
    Install {
        /// Create autostart entry
        #[arg(long)]
        autostart: bool,

        /// Create desktop application entry
        #[arg(long)]
        desktop: bool,

        /// Install to system-wide location (requires sudo)
        #[arg(long)]
        system: bool,
    },

    /// Remove desktop integration
    Uninstall {
        /// Remove autostart entry
        #[arg(long)]
        autostart: bool,

        /// Remove desktop application entry
        #[arg(long)]
        desktop: bool,

        /// Remove from system-wide location (requires sudo)
        #[arg(long)]
        system: bool,
    },
}

#[derive(Subcommand)]
pub enum SetupCommands {
    /// Show UInput setup guide for optimal text insertion
    Uinput {
        /// Show quick setup commands only
        #[arg(short, long)]
        quick: bool,

        /// Apply recommended fixes automatically (requires sudo)
        #[arg(long)]
        auto_fix: bool,
    },

    /// Diagnose UInput setup issues
    DiagnoseUinput,

    /// Audio system setup
    Audio {
        /// List available audio devices
        #[arg(short, long)]
        list: bool,

        /// Test specific device
        #[arg(short, long)]
        test: Option<String>,
    },

    /// Hotkey system setup
    Hotkeys {
        /// Test hotkey combination
        #[arg(short, long)]
        test: Option<String>,

        /// List available key combinations
        #[arg(short, long)]
        list: bool,
    },

    /// Complete system setup wizard
    Wizard {
        /// Skip interactive prompts and use defaults
        #[arg(long)]
        auto: bool,
    },
}

#[derive(Subcommand)]
pub enum TestCommands {
    /// Test audio capture system
    Audio {
        /// Duration to test in seconds
        #[arg(short, long, default_value = "3")]
        duration: u64,

        /// Show available devices
        #[arg(long)]
        list_devices: bool,

        /// Test specific device
        #[arg(long)]
        device: Option<String>,

        /// Save test audio to file
        #[arg(long)]
        save: Option<PathBuf>,
    },

    /// Test transcription system
    Transcription {
        /// Audio file to transcribe
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Test with different model sizes
        #[arg(long)]
        all_models: bool,

        /// Show detailed timing information
        #[arg(long)]
        timing: bool,
    },

    /// Test text insertion system
    TextInsertion {
        /// Text to insert for testing
        #[arg(short, long, default_value = "Hello from Hush! 🤫")]
        text: String,

        /// Test all insertion methods
        #[arg(long)]
        all_methods: bool,

        /// Test UInput specifically
        #[arg(long)]
        uinput: bool,
    },

    /// Test hotkey system
    Hotkeys {
        /// Hotkey combination to test
        #[arg(short, long)]
        combination: Option<String>,

        /// Test duration in seconds
        #[arg(short, long, default_value = "10")]
        duration: u64,
    },

    /// Test complete voice-to-text pipeline
    Pipeline {
        /// Number of test recordings
        #[arg(short, long, default_value = "1")]
        count: u32,

        /// Skip text insertion (just transcribe)
        #[arg(long)]
        transcribe_only: bool,
    },

    /// Run all system tests
    All {
        /// Include performance benchmarks
        #[arg(long)]
        benchmarks: bool,

        /// Save test results to file
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum ModelCommands {
    /// List available models
    List {
        /// Show downloaded models only
        #[arg(short, long)]
        downloaded: bool,

        /// Show model details (size, accuracy, speed)
        #[arg(long)]
        details: bool,
    },

    /// Download a Whisper model
    Download {
        /// Model size to download (tiny, base, small, medium, large)
        model_size: String,

        /// Force re-download even if model exists
        #[arg(short, long)]
        force: bool,
    },

    /// Remove downloaded models
    Remove {
        /// Model size to remove, or "all" for all models
        model_size: String,

        /// Don't prompt for confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Show model cache information
    Info {
        /// Clear cache statistics
        #[arg(long)]
        clear_stats: bool,
    },

    /// Verify model integrity
    Verify {
        /// Model size to verify, or "all" for all models
        model_size: Option<String>,

        /// Fix corrupted models by re-downloading
        #[arg(long)]
        fix: bool,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn init_logging(&self) {
        use tracing_subscriber::{EnvFilter, fmt};
        use tracing::Level;

        let level = match self.verbose {
            0 => Level::WARN,
            1 => Level::INFO,
            2 => Level::DEBUG,
            _ => Level::TRACE,
        };

        let filter = EnvFilter::from_default_env()
            .add_directive(format!("hush={}", level).parse().unwrap());

        fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .init();
    }
}