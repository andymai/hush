// CLI module for unified command structure
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hush")]
#[command(about = "🤫 Hush - local voice-to-text for Linux")]
#[command(version)]
#[command(author = "Andy")]
#[command(long_about = LONG_ABOUT)]
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

// Hard line breaks, because clap does not wrap help text without its wrap_help feature.
const LONG_ABOUT: &str = "\
Local voice-to-text for Linux. Hold the hotkey, speak, and release: Hush types
the text at the cursor.

By default, speech recognition runs on this machine. With the default
llm.polish = true and llm.provider = \"auto\", Hush polishes the text with a
reachable Ollama server. Without Ollama, it uses Anthropic when
ANTHROPIC_API_KEY is set (in the environment or ~/.config/hush/.env). When
neither is there, Hush makes no LLM call. The LLM gets only text, never audio.";

/// A `--lang` value: a code that Whisper knows, or `auto`.
fn language_code(code: &str) -> Result<String, String> {
    crate::transcription::Language::from_code(code)
        .map(|language| language.code().to_string())
        .map_err(|e| e.to_string())
}

/// A `--style` value: a valid style name. Whether the style exists is a
/// question for the config, which `hush start` reads.
fn style_name(name: &str) -> Result<String, String> {
    crate::text_processing::StyleName::from_name(name)
        .map(|name| name.to_string())
        .map_err(|e| e.to_string())
}

#[derive(Subcommand)]
pub enum Commands {
    /// Record once and exit (no hotkeys)
    Record {
        /// Maximum recording duration in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,

        /// Print the text on stdout and type nothing. Prompts and status lines go to stderr
        #[arg(short, long)]
        print_only: bool,

        /// Save audio to file for debugging
        #[arg(long)]
        save_audio: Option<PathBuf>,
    },

    /// Manual recording mode (interactive prompts)
    Manual {
        /// Number of recordings to make
        #[arg(short = 'n', long, default_value = "1")]
        count: u32,
    },

    /// Run the daemon in this terminal (same as `daemon start --foreground`)
    Listen {
        /// Editing mode for text processing
        #[arg(short, long, default_value = "light")]
        editing_mode: String,

        /// Disable text processing (use raw transcription)
        #[arg(long)]
        no_processing: bool,

        /// Hide overlay button when idle
        #[arg(long)]
        no_button: bool,
    },

    /// Run Hush in the background and control it over its socket
    Daemon {
        #[command(subcommand)]
        daemon_command: DaemonCommands,
    },

    /// Start recording if idle, otherwise stop and transcribe (talks to the daemon)
    Toggle {
        /// Language of the take it starts: a Whisper code such as de, or auto
        #[arg(long, value_name = "CODE", value_parser = language_code)]
        lang: Option<String>,

        /// Style of the take it starts: verbatim, or a style from the styles table
        #[arg(long, value_name = "NAME", value_parser = style_name)]
        style: Option<String>,
    },

    /// Start recording (talks to the daemon)
    Start {
        /// Language of this take: a Whisper code such as de, or auto
        #[arg(long, value_name = "CODE", value_parser = language_code)]
        lang: Option<String>,

        /// Style of this take: verbatim, or a style from the styles table
        #[arg(long, value_name = "NAME", value_parser = style_name)]
        style: Option<String>,
    },

    /// Stop recording and transcribe (talks to the daemon)
    Stop {
        /// Wait for the final text and print it instead of typing it
        #[arg(long)]
        print: bool,

        /// Print one JSON object: status, text, chars, message
        #[arg(long)]
        json: bool,
    },

    /// Stop recording and discard the audio (talks to the daemon)
    Cancel,

    /// Check everything Hush needs and say how to fix what is missing
    Doctor,

    /// Open the setup and settings window
    Settings,

    /// Type the last transcript again (talks to the daemon)
    PasteLast,

    /// In portal mode, show the keys the desktop binds to Hush's shortcuts
    /// (talks to the daemon)
    Shortcuts {
        /// Open the desktop's dialog to change the keys
        #[arg(long)]
        configure: bool,
    },

    /// Add the selected text to the vocabulary with its exact spelling (talks to the daemon)
    Learn {
        /// Words to learn instead of the current selection
        text: Vec<String>,
    },

    /// Print the text of a WAV or FLAC file, with the daemon's model when it runs
    Transcribe {
        /// The audio file
        file: PathBuf,
    },

    /// Transcribe the audio of a take that Hush could not transcribe, then delete it
    Recover {
        /// Every kept recording, oldest first, instead of only the newest
        #[arg(long, conflicts_with = "list")]
        all: bool,

        /// List the kept recordings and transcribe nothing
        #[arg(long)]
        list: bool,

        /// Type the text at the cursor instead of printing it
        #[arg(long = "type", conflicts_with = "list")]
        type_text: bool,
    },

    /// Rewrite the selected text with a style's prompt (talks to the daemon)
    Rewrite {
        /// A style from the styles table with polish = true and a prompt
        #[arg(value_name = "STYLE", value_parser = style_name)]
        style: String,
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

    /// Move the settings and the vocabulary to another machine
    Config {
        #[command(subcommand)]
        config_command: ConfigCommands,
    },

    /// Show application status and configuration
    Status {
        /// Print only the effective settings (with --devices, also the input devices)
        #[arg(long)]
        config: bool,

        /// Print only the input devices (with --config, also the settings)
        #[arg(long)]
        devices: bool,

        /// Check all systems
        #[arg(long, conflicts_with = "json")]
        full: bool,

        /// Print one JSON object for scripts. It has the main settings and the input devices, so
        /// --config and --devices do not apply
        #[arg(long, conflicts_with_all = ["config", "devices"])]
        json: bool,

        /// With --json, print one Waybar line at once and on each state change
        #[arg(long, requires = "json")]
        follow: bool,
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
pub enum DaemonCommands {
    /// Start the daemon, detached from this terminal unless --foreground
    Start {
        /// Stay attached to this terminal
        #[arg(long)]
        foreground: bool,

        /// Editing mode for text processing
        #[arg(short, long, default_value = "light")]
        editing_mode: String,

        /// Disable text processing (use raw transcription)
        #[arg(long)]
        no_processing: bool,

        /// Hide overlay button when idle
        #[arg(long)]
        no_button: bool,
    },

    /// Ask the daemon to quit
    Stop,

    /// Stop the daemon if it runs, then start it detached
    Restart {
        /// Editing mode for text processing
        #[arg(short, long, default_value = "light")]
        editing_mode: String,

        /// Disable text processing (use raw transcription)
        #[arg(long)]
        no_processing: bool,

        /// Hide overlay button when idle
        #[arg(long)]
        no_button: bool,
    },

    /// Show what the daemon is doing
    Status,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Write the settings and the vocabulary to one TOML file, without API keys
    Export {
        /// The file to write
        file: PathBuf,

        /// Replace the file if it exists
        #[arg(short, long)]
        force: bool,
    },

    /// Load the settings and the vocabulary from a file that `config export` wrote
    Import {
        /// The file to read
        file: PathBuf,

        /// Keep the current values and add only the missing ones
        #[arg(long)]
        merge: bool,
    },
}

#[derive(Subcommand)]
pub enum SetupCommands {
    /// Initialize configuration file
    Init {
        /// Write the built-in defaults without prompting. The model is base, not the model that
        /// Hush suggests for this machine
        #[arg(long)]
        defaults: bool,

        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
    },

    /// Grant access to keyboards and uinput through one udev rule
    Permissions {
        /// Report the current access state without changing anything
        #[arg(long)]
        check: bool,

        /// Print the commands instead of running them (for containers or remote shells)
        #[arg(long)]
        print: bool,
    },

    /// Show UInput setup guide for optimal text insertion
    Uinput {
        /// Print the permission setup commands
        #[arg(short, long)]
        quick: bool,

        /// Install the udev rule (same as `setup permissions`)
        #[arg(long)]
        auto_fix: bool,
    },

    /// Diagnose UInput setup issues
    DiagnoseUinput,

    /// Keep the microphone awake between takes with a WirePlumber rule
    Mic {
        /// Remove the rule of Hush, so that the microphone sleeps again
        #[arg(long)]
        undo: bool,

        /// Restart WirePlumber without a question first (for scripts)
        #[arg(short, long)]
        yes: bool,
    },

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
        /// Skip the prompts. When the configured model is missing, the wizard downloads the model
        /// that Hush suggests for this machine
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
        #[arg(long)]
        combination: Option<String>,

        /// Test duration in seconds
        #[arg(short, long, default_value = "10")]
        duration: u64,
    },

    /// Show which window Hush sees as focused, and the tone profile it picks
    Window {
        /// Keep reporting for this many seconds so you can switch windows
        #[arg(short, long, default_value = "5")]
        duration: u64,
    },

    /// Test complete voice-to-text pipeline
    Pipeline {
        /// Number of test recordings
        #[arg(short = 'n', long, default_value = "1")]
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

    /// Download a speech model: Whisper, or the opt-in Parakeet
    Download {
        /// Model name (tiny, base, small, medium, large, large-v2, large-v3, parakeet-v3)
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

    /// Set the active model (downloads if needed and updates config)
    Set {
        /// Model to set as active (tiny, base, small, medium, large, large-v2, large-v3,
        /// parakeet-v3)
        model_size: String,
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
        use tracing::Level;
        use tracing_subscriber::{fmt, EnvFilter};

        let level = match self.verbose {
            0 => Level::WARN,
            1 => Level::INFO,
            2 => Level::DEBUG,
            _ => Level::TRACE,
        };

        let filter = match format!("hush={}", level).parse() {
            Ok(directive) => EnvFilter::from_default_env().add_directive(directive),
            Err(_) => EnvFilter::from_default_env(),
        };

        fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .init();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang_of(args: &[&str]) -> Option<String> {
        match Cli::try_parse_from(args).map(|cli| cli.command) {
            Ok(Commands::Start { lang, .. } | Commands::Toggle { lang, .. }) => lang,
            Ok(_) => panic!("{args:?} is not start or toggle"),
            Err(e) => panic!("{args:?}: {e}"),
        }
    }

    #[test]
    fn start_and_toggle_take_a_whisper_language() {
        assert_eq!(lang_of(&["hush", "start"]), None);
        assert_eq!(
            lang_of(&["hush", "start", "--lang", "de"]).as_deref(),
            Some("de")
        );
        assert_eq!(
            lang_of(&["hush", "toggle", "--lang", "auto"]).as_deref(),
            Some("auto")
        );
        for bad in ["xx", "DE", "german"] {
            let error = match Cli::try_parse_from(["hush", "start", "--lang", bad]) {
                Ok(_) => panic!("{bad} must fail"),
                Err(error) => error,
            };
            assert_eq!(error.exit_code(), 2, "a usage error");
        }
    }

    fn style_of(args: &[&str]) -> Option<String> {
        match Cli::try_parse_from(args).map(|cli| cli.command) {
            Ok(Commands::Start { style, .. } | Commands::Toggle { style, .. }) => style,
            Ok(_) => panic!("{args:?} is not start or toggle"),
            Err(e) => panic!("{args:?}: {e}"),
        }
    }

    #[test]
    fn rewrite_takes_one_style_name() {
        match Cli::try_parse_from(["hush", "rewrite", "formal"]).map(|cli| cli.command) {
            Ok(Commands::Rewrite { style }) => assert_eq!(style, "formal"),
            Ok(_) => panic!("not rewrite"),
            Err(e) => panic!("{e}"),
        }
        for args in [
            &["hush", "rewrite"][..],
            &["hush", "rewrite", "Formal"],
            &["hush", "rewrite", "formal", "casual"],
        ] {
            let error = match Cli::try_parse_from(args) {
                Ok(_) => panic!("{args:?} must fail"),
                Err(error) => error,
            };
            assert_eq!(error.exit_code(), 2, "{args:?} is a usage error");
        }
    }

    #[test]
    fn start_and_toggle_take_a_style_name() {
        assert_eq!(style_of(&["hush", "start"]), None);
        assert_eq!(
            style_of(&["hush", "start", "--style", "verbatim"]).as_deref(),
            Some("verbatim")
        );
        assert_eq!(
            style_of(&["hush", "toggle", "--lang", "de", "--style", "casual"]).as_deref(),
            Some("casual")
        );
        for bad in ["Casual", "two words", "-x"] {
            let error = match Cli::try_parse_from(["hush", "start", "--style", bad]) {
                Ok(_) => panic!("{bad} must fail"),
                Err(error) => error,
            };
            assert_eq!(error.exit_code(), 2, "a usage error");
        }
    }
}
