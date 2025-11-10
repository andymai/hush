/// Application context detection for adaptive text processing
use tracing::{debug, warn};
use std::process::Command;
use regex::Regex;
use once_cell::sync::Lazy;

/// The type of application the user is currently in
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationContext {
    /// Code editor or IDE
    Code(CodeContext),
    /// Communication app (email, chat, etc.)
    Communication(CommunicationType),
    /// Documentation or note-taking
    Documentation,
    /// Terminal/console
    Terminal,
    /// Web browser
    Browser(BrowserContext),
    /// Unknown application
    Unknown,
}

/// Code editor context details
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeContext {
    /// Programming language (detected from file extension if available)
    pub language: Option<String>,
    /// Editor name (VS Code, Vim, IntelliJ, etc.)
    pub editor: String,
}

/// Communication app type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationType {
    Email,
    Chat,      // Slack, Discord, Teams
    Social,    // Twitter, LinkedIn, etc.
}

/// Browser context details
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserContext {
    /// Website domain
    pub domain: Option<String>,
    /// Site type (GitHub, Gmail, docs, etc.)
    pub site_type: BrowserSiteType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSiteType {
    CodeRepository,  // GitHub, GitLab
    Documentation,   // MDN, docs.rs, etc.
    Email,           // Gmail, Outlook
    Chat,            // Slack web, Discord web
    SocialMedia,
    Other,
}

/// Patterns for detecting code editors
static CODE_EDITOR_PATTERNS: &[(&str, &str)] = &[
    ("code", "VS Code"),
    ("vscode", "VS Code"),
    ("visual studio code", "VS Code"),
    ("vim", "Vim"),
    ("nvim", "Neovim"),
    ("neovim", "Neovim"),
    ("emacs", "Emacs"),
    ("intellij", "IntelliJ IDEA"),
    ("pycharm", "PyCharm"),
    ("webstorm", "WebStorm"),
    ("sublime", "Sublime Text"),
    ("atom", "Atom"),
    ("gedit", "gedit"),
    ("kate", "Kate"),
];

/// Patterns for detecting terminals
static TERMINAL_PATTERNS: &[&str] = &[
    "terminal", "konsole", "gnome-terminal", "xterm", "alacritty",
    "kitty", "wezterm", "terminator", "tilix", "bash", "zsh", "fish",
];

/// Patterns for detecting browsers
static BROWSER_PATTERNS: &[&str] = &[
    "firefox", "chrome", "chromium", "brave", "edge", "safari", "opera",
];

/// File extension to language mapping
static LANGUAGE_EXTENSIONS: &[(&str, &str)] = &[
    ("rs", "Rust"),
    ("py", "Python"),
    ("js", "JavaScript"),
    ("ts", "TypeScript"),
    ("go", "Go"),
    ("java", "Java"),
    ("cpp", "C++"),
    ("c", "C"),
    ("rb", "Ruby"),
    ("php", "PHP"),
    ("swift", "Swift"),
    ("kt", "Kotlin"),
];

static FILE_EXTENSION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\.([a-z0-9]+)\s*[-–—]").unwrap()
});

/// Application context detector
pub struct ContextDetector {
    last_context: Option<ApplicationContext>,
}

impl ContextDetector {
    /// Create a new context detector
    pub fn new() -> Self {
        Self {
            last_context: None,
        }
    }

    /// Detect current application context
    pub fn detect(&mut self) -> ApplicationContext {
        match self.detect_active_window() {
            Some(window_info) => {
                let context = self.parse_window_info(&window_info);
                debug!("Detected context: {:?}", context);
                self.last_context = Some(context.clone());
                context
            }
            None => {
                debug!("Could not detect active window, using last known context");
                self.last_context.clone().unwrap_or(ApplicationContext::Unknown)
            }
        }
    }

    /// Get last detected context without re-detecting
    pub fn last_context(&self) -> Option<&ApplicationContext> {
        self.last_context.as_ref()
    }

    /// Detect active window title and class (X11-specific for now)
    fn detect_active_window(&self) -> Option<String> {
        // Try xdotool first (works on X11)
        if let Ok(output) = Command::new("xdotool")
            .args(&["getactivewindow", "getwindowname"])
            .output()
        {
            if output.status.success() {
                let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !title.is_empty() {
                    debug!("Active window (xdotool): {}", title);
                    return Some(title);
                }
            }
        }

        // Try wmctrl as fallback
        if let Ok(output) = Command::new("wmctrl")
            .args(&["-lx"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse wmctrl output to find active window
                // Format: <window-id> <desktop> <class> <client-machine> <title>
                if let Some(line) = stdout.lines().next() {
                    debug!("Active window (wmctrl): {}", line);
                    return Some(line.to_string());
                }
            }
        }

        // Wayland: Try swaymsg for Sway/i3
        if let Ok(output) = Command::new("swaymsg")
            .args(&["-t", "get_tree"])
            .output()
        {
            if output.status.success() {
                // Parse JSON to find focused window
                // This is complex, so we'll just return the raw output for now
                let info = String::from_utf8_lossy(&output.stdout);
                if info.contains("focused") {
                    debug!("Active window (swaymsg): detected");
                    // TODO: Proper JSON parsing
                    return Some(info.to_string());
                }
            }
        }

        warn!("Could not detect active window (install xdotool, wmctrl, or use Sway)");
        None
    }

    /// Parse window information to determine context
    fn parse_window_info(&self, info: &str) -> ApplicationContext {
        let info_lower = info.to_lowercase();

        // Check for code editors
        for (pattern, editor_name) in CODE_EDITOR_PATTERNS {
            if info_lower.contains(pattern) {
                let language = self.detect_language_from_window(info);
                return ApplicationContext::Code(CodeContext {
                    language,
                    editor: editor_name.to_string(),
                });
            }
        }

        // Check for terminals
        for pattern in TERMINAL_PATTERNS {
            if info_lower.contains(pattern) {
                return ApplicationContext::Terminal;
            }
        }

        // Check for browsers
        for pattern in BROWSER_PATTERNS {
            if info_lower.contains(pattern) {
                let domain = self.extract_domain(info);
                let site_type = self.classify_website(&domain);
                return ApplicationContext::Browser(BrowserContext {
                    domain,
                    site_type,
                });
            }
        }

        // Check for communication apps
        if info_lower.contains("slack") || info_lower.contains("discord")
            || info_lower.contains("teams") || info_lower.contains("telegram") {
            return ApplicationContext::Communication(CommunicationType::Chat);
        }

        if info_lower.contains("thunderbird") || info_lower.contains("evolution")
            || info_lower.contains("gmail") || info_lower.contains("outlook") {
            return ApplicationContext::Communication(CommunicationType::Email);
        }

        // Check for documentation apps
        if info_lower.contains("notion") || info_lower.contains("obsidian")
            || info_lower.contains("logseq") || info_lower.contains("joplin") {
            return ApplicationContext::Documentation;
        }

        ApplicationContext::Unknown
    }

    /// Try to detect programming language from window title
    fn detect_language_from_window(&self, title: &str) -> Option<String> {
        // Look for file extensions in window title
        if let Some(captures) = FILE_EXTENSION_RE.captures(title) {
            if let Some(ext) = captures.get(1) {
                let ext_str = ext.as_str();
                for (extension, language) in LANGUAGE_EXTENSIONS {
                    if ext_str == *extension {
                        return Some(language.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extract domain from browser window title
    fn extract_domain(&self, title: &str) -> Option<String> {
        // Common browser title formats:
        // "Page Title - Mozilla Firefox"
        // "Page Title - Google Chrome"
        // Often includes URL or domain in title

        if let Some(domain) = title.split(" - ").next() {
            // Simple domain extraction (this could be more sophisticated)
            if domain.contains("github") {
                return Some("github.com".to_string());
            }
            if domain.contains("gitlab") {
                return Some("gitlab.com".to_string());
            }
            if domain.contains("gmail") {
                return Some("gmail.com".to_string());
            }
            if domain.contains("slack") {
                return Some("slack.com".to_string());
            }
        }

        None
    }

    /// Classify website type based on domain
    fn classify_website(&self, domain: &Option<String>) -> BrowserSiteType {
        if let Some(domain) = domain {
            let domain_lower = domain.to_lowercase();

            if domain_lower.contains("github") || domain_lower.contains("gitlab") {
                return BrowserSiteType::CodeRepository;
            }
            if domain_lower.contains("gmail") || domain_lower.contains("outlook") {
                return BrowserSiteType::Email;
            }
            if domain_lower.contains("slack") || domain_lower.contains("discord") {
                return BrowserSiteType::Chat;
            }
            if domain_lower.contains("twitter") || domain_lower.contains("linkedin")
                || domain_lower.contains("facebook") {
                return BrowserSiteType::SocialMedia;
            }
            if domain_lower.contains("docs.") || domain_lower.contains("documentation")
                || domain_lower.contains("mdn") {
                return BrowserSiteType::Documentation;
            }
        }

        BrowserSiteType::Other
    }
}

impl Default for ContextDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplicationContext {
    /// Get a human-readable description of this context
    pub fn description(&self) -> String {
        match self {
            ApplicationContext::Code(ctx) => {
                if let Some(lang) = &ctx.language {
                    format!("{} - {} code", ctx.editor, lang)
                } else {
                    format!("{} - code editor", ctx.editor)
                }
            }
            ApplicationContext::Communication(comm_type) => match comm_type {
                CommunicationType::Email => "Email".to_string(),
                CommunicationType::Chat => "Chat".to_string(),
                CommunicationType::Social => "Social media".to_string(),
            },
            ApplicationContext::Documentation => "Documentation".to_string(),
            ApplicationContext::Terminal => "Terminal".to_string(),
            ApplicationContext::Browser(ctx) => {
                if let Some(domain) = &ctx.domain {
                    format!("Browser - {}", domain)
                } else {
                    "Browser".to_string()
                }
            }
            ApplicationContext::Unknown => "Unknown".to_string(),
        }
    }

    /// Check if this is a code-related context
    pub fn is_code(&self) -> bool {
        matches!(self, ApplicationContext::Code(_) | ApplicationContext::Terminal)
    }

    /// Check if this is a communication context
    pub fn is_communication(&self) -> bool {
        matches!(self, ApplicationContext::Communication(_))
            || matches!(
                self,
                ApplicationContext::Browser(BrowserContext {
                    site_type: BrowserSiteType::Email | BrowserSiteType::Chat,
                    ..
                })
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vscode_window() {
        let detector = ContextDetector::new();
        let context = detector.parse_window_info("main.rs - Visual Studio Code");

        match context {
            ApplicationContext::Code(ctx) => {
                assert_eq!(ctx.editor, "VS Code");
                assert_eq!(ctx.language, Some("Rust".to_string()));
            }
            _ => panic!("Expected Code context"),
        }
    }

    #[test]
    fn test_parse_terminal_window() {
        let detector = ContextDetector::new();
        let context = detector.parse_window_info("andy@laptop: ~/projects - Terminal");

        assert_eq!(context, ApplicationContext::Terminal);
    }

    #[test]
    fn test_parse_browser_github() {
        let detector = ContextDetector::new();
        let context = detector.parse_window_info("github.com/user/repo - Mozilla Firefox");

        match context {
            ApplicationContext::Browser(ctx) => {
                assert_eq!(ctx.site_type, BrowserSiteType::CodeRepository);
            }
            _ => panic!("Expected Browser context"),
        }
    }

    #[test]
    fn test_parse_slack() {
        let detector = ContextDetector::new();
        let context = detector.parse_window_info("Slack - #general");

        assert_eq!(
            context,
            ApplicationContext::Communication(CommunicationType::Chat)
        );
    }

    #[test]
    fn test_context_description() {
        let code_ctx = ApplicationContext::Code(CodeContext {
            language: Some("Rust".to_string()),
            editor: "VS Code".to_string(),
        });
        assert_eq!(code_ctx.description(), "VS Code - Rust code");

        let terminal_ctx = ApplicationContext::Terminal;
        assert_eq!(terminal_ctx.description(), "Terminal");
    }

    #[test]
    fn test_is_code() {
        let code_ctx = ApplicationContext::Code(CodeContext {
            language: None,
            editor: "Vim".to_string(),
        });
        assert!(code_ctx.is_code());

        let terminal_ctx = ApplicationContext::Terminal;
        assert!(terminal_ctx.is_code());

        let email_ctx = ApplicationContext::Communication(CommunicationType::Email);
        assert!(!email_ctx.is_code());
    }
}
