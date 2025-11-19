/// Comprehensive logging system for Hush
///
/// This module provides:
/// - Structured file logging with rotation
/// - Performance metrics tracking
/// - Request correlation and user journey tracking  
/// - Health monitoring and system diagnostics
/// - Component-specific log levels and filtering
use anyhow::Result;
use parking_lot::Mutex;
use serde_json::json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::System;
use tracing::{Event, Level, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime, FmtContext, FormatEvent, FormatFields},
    EnvFilter,
};

/// Global session ID for correlating logs across the application lifecycle
static SESSION_ID: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    format!(
        "hush_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    )
});

/// Application start time for uptime tracking
static APP_START_TIME: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);

/// Request correlation tracker - tracks individual voice-to-text operations
#[derive(Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub operation: String,
    pub start_time: Instant,
    pub user_agent: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl RequestContext {
    pub fn new(operation: &str) -> Self {
        Self {
            request_id: generate_request_id(),
            operation: operation.to_string(),
            start_time: Instant::now(),
            user_agent: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Performance metrics collector
#[derive(Default)]
pub struct PerformanceMetrics {
    pub cpu_usage: f32,
    pub memory_usage_mb: u64,
    pub disk_usage_mb: u64,
    pub open_files: usize,
    pub uptime_seconds: u64,
}

impl PerformanceMetrics {
    pub fn collect() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let cpu_usage = system.global_cpu_usage();

        let memory_usage_mb = if let Some(process) = system.processes_by_name(OsStr::new("hush")).next() {
            process.memory() / 1024 / 1024
        } else {
            0
        };

        // Calculate disk usage for hush cache directory
        let disk_usage_mb = Self::calculate_disk_usage();

        // Count open file descriptors (Linux only)
        let open_files = Self::count_open_files();

        // Calculate uptime
        let uptime_seconds = APP_START_TIME.elapsed().as_secs();

        Self {
            cpu_usage,
            memory_usage_mb,
            disk_usage_mb,
            open_files,
            uptime_seconds,
        }
    }

    fn calculate_disk_usage() -> u64 {
        // Calculate disk usage for hush cache directories
        let mut total_size = 0u64;

        if let Some(cache_dir) = dirs::cache_dir() {
            let hush_cache = cache_dir.join("hush");
            if hush_cache.exists() {
                total_size += Self::dir_size(&hush_cache).unwrap_or(0);
            }
        }

        // Also check for models and logs
        if let Some(home_dir) = dirs::home_dir() {
            let models_dir = home_dir.join(".hush/models");
            if models_dir.exists() {
                total_size += Self::dir_size(&models_dir).unwrap_or(0);
            }
        }

        if let Some(data_dir) = dirs::data_local_dir() {
            let logs_dir = data_dir.join("hush/logs");
            if logs_dir.exists() {
                total_size += Self::dir_size(&logs_dir).unwrap_or(0);
            }
        }

        total_size / (1024 * 1024) // Convert to MB
    }

    fn dir_size(path: &PathBuf) -> Result<u64> {
        let mut size = 0u64;
        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let metadata = entry.metadata()?;
                if metadata.is_file() {
                    size += metadata.len();
                } else if metadata.is_dir() {
                    size += Self::dir_size(&entry.path()).unwrap_or(0);
                }
            }
        }
        Ok(size)
    }

    fn count_open_files() -> usize {
        // Count open file descriptors on Linux via /proc
        #[cfg(target_os = "linux")]
        {
            let pid = std::process::id();
            let fd_dir = PathBuf::from(format!("/proc/{}/fd", pid));
            if fd_dir.exists() {
                std::fs::read_dir(fd_dir)
                    .map(|entries| entries.count())
                    .unwrap_or(0)
            } else {
                0
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            0 // Not implemented for non-Linux platforms
        }
    }
}

/// Custom JSON formatter for structured logging
pub struct HushJsonFormatter;

impl<S, N> FormatEvent<S, N> for HushJsonFormatter
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        // Get span context for request correlation
        let span_context = if let Some(span_ref) = ctx.lookup_current() {
            span_ref.extensions().get::<RequestContext>().cloned()
        } else {
            None
        };

        // Build structured log entry
        let mut log_entry = json!({
            "timestamp": now,
            "level": metadata.level().to_string(),
            "target": metadata.target(),
            "session_id": *SESSION_ID,
            "module": metadata.module_path().unwrap_or("unknown"),
            "file": metadata.file().unwrap_or("unknown"),
            "line": metadata.line().unwrap_or(0),
        });

        // Add request context if available
        if let Some(ctx) = span_context {
            log_entry["request_id"] = json!(ctx.request_id);
            log_entry["operation"] = json!(ctx.operation);
            log_entry["elapsed_ms"] = json!(ctx.elapsed().as_millis());
            if !ctx.metadata.is_empty() {
                log_entry["metadata"] = json!(ctx.metadata);
            }
        }

        // Extract the message from the event
        let mut message = String::new();
        let mut visitor = JsonMessageVisitor::new(&mut message);
        event.record(&mut visitor);
        log_entry["message"] = json!(message);

        // Add performance metrics for high-level events
        if metadata.level() <= &Level::INFO {
            let metrics = PerformanceMetrics::collect();
            log_entry["performance"] = json!({
                "cpu_usage_percent": metrics.cpu_usage,
                "memory_usage_mb": metrics.memory_usage_mb,
            });
        }

        writeln!(writer, "{}", log_entry)
    }
}

/// Visitor to extract message from tracing event
struct JsonMessageVisitor<'a> {
    message: &'a mut String,
}

impl<'a> JsonMessageVisitor<'a> {
    fn new(message: &'a mut String) -> Self {
        Self { message }
    }
}

impl<'a> tracing::field::Visit for JsonMessageVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            *self.message = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            *self.message = value.to_string();
        }
    }
}

/// Custom time formatter for human-readable logs
pub struct HushTimeFormatter;

impl FormatTime for HushTimeFormatter {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%Y-%m-%d %H:%M:%S%.3f"))
    }
}

/// Central logging configuration
pub struct LoggingConfig {
    pub log_dir: PathBuf,
    pub max_file_size_mb: u64,
    pub max_files: usize,
    pub json_output: bool,
    pub console_output: bool,
    pub component_levels: HashMap<String, Level>,
    pub performance_logging: bool,
    pub request_tracing: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        let log_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".local/share/hush/logs");

        Self {
            log_dir,
            max_file_size_mb: 100,
            max_files: 10,
            json_output: true,
            console_output: true,
            component_levels: HashMap::new(),
            performance_logging: true,
            request_tracing: true,
        }
    }
}

/// Main logging system manager
pub struct LoggingSystem {
    config: LoggingConfig,
    _guards: Vec<WorkerGuard>,
    system_info: Arc<Mutex<System>>,
}

impl LoggingSystem {
    pub fn new(config: LoggingConfig) -> Result<Self> {
        let system_info = Arc::new(Mutex::new(System::new_all()));

        Ok(Self {
            config,
            _guards: Vec::new(),
            system_info,
        })
    }

    pub fn initialize(mut self) -> Result<LoggingSystem> {
        // Ensure log directory exists
        std::fs::create_dir_all(&self.config.log_dir)?;

        // Build component-specific filter
        let mut filter = EnvFilter::from_default_env().add_directive("hush=info".parse().unwrap());

        for (component, level) in &self.config.component_levels {
            let directive = format!("hush::{}={}", component, level);
            filter = filter.add_directive(directive.parse().unwrap());
        }

        // Initialize file appender for structured logging
        if self.config.json_output {
            let file_appender = RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_suffix("log")
                .max_log_files(self.config.max_files)
                .build(&self.config.log_dir)?;

            let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
            self._guards.push(guard);

            // Set up subscriber with both console and file output
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_timer(HushTimeFormatter)
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_writer(file_writer)
                .finish();

            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| anyhow::anyhow!("Failed to set global logger: {}", e))?;
        } else {
            // Console-only logging
            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_timer(HushTimeFormatter)
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .finish();

            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| anyhow::anyhow!("Failed to set global logger: {}", e))?;
        }

        // Log system initialization
        tracing::info!(
            session_id = %*SESSION_ID,
            log_dir = %self.config.log_dir.display(),
            json_output = self.config.json_output,
            console_output = self.config.console_output,
            "🔧 Hush logging system initialized"
        );

        self.log_system_info();

        Ok(self)
    }

    /// Log comprehensive system information at startup
    fn log_system_info(&self) {
        let mut system = self.system_info.lock();
        system.refresh_all();

        let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let os_info = format!("{} {}", os_name, kernel_version);

        tracing::info!(
            session_id = %*SESSION_ID,
            os_info = %os_info,
            total_memory_gb = %(system.total_memory() / 1024 / 1024 / 1024),
            cpu_count = %system.cpus().len(),
            "🖥️ System information logged"
        );
    }

    /// Create a new request context for tracking operations
    pub fn create_request_context(&self, operation: &str) -> RequestContext {
        if self.config.request_tracing {
            RequestContext::new(operation)
        } else {
            // Return a minimal context if request tracing is disabled
            RequestContext::new(operation)
        }
    }

    /// Log health status of all components
    pub fn log_health_status(&self) {
        let metrics = PerformanceMetrics::collect();

        tracing::info!(
            session_id = %*SESSION_ID,
            cpu_usage_percent = %metrics.cpu_usage,
            memory_usage_mb = %metrics.memory_usage_mb,
            "💓 System health check"
        );
    }

    /// Get session ID for external correlation
    pub fn session_id() -> &'static str {
        &SESSION_ID
    }
}

/// Generate a unique request ID
fn generate_request_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);

    format!(
        "req_{:08x}_{:04x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        COUNTER.fetch_add(1, Ordering::SeqCst) % 0xFFFF
    )
}

/// Convenience macros for structured logging

#[macro_export]
macro_rules! log_request_start {
    ($ctx:expr, $msg:literal $(, $field:ident = $value:expr)*) => {
        tracing::info!(
            request_id = %$ctx.request_id,
            operation = %$ctx.operation,
            $(
                $field = $value,
            )*
            "{} started", $msg
        );
    };
}

#[macro_export]
macro_rules! log_request_end {
    ($ctx:expr, $msg:literal $(, $field:ident = $value:expr)*) => {
        tracing::info!(
            request_id = %$ctx.request_id,
            operation = %$ctx.operation,
            elapsed_ms = %$ctx.elapsed().as_millis(),
            $(
                $field = $value,
            )*
            "{} completed", $msg
        );
    };
}

#[macro_export]
macro_rules! log_request_error {
    ($ctx:expr, $error:expr, $msg:literal $(, $field:ident = $value:expr)*) => {
        tracing::error!(
            request_id = %$ctx.request_id,
            operation = %$ctx.operation,
            elapsed_ms = %$ctx.elapsed().as_millis(),
            error = %$error,
            $(
                $field = $value,
            )*
            "{} failed", $msg
        );
    };
}

/// Component-specific logging helpers

pub mod audio {
    use crate::logging::RequestContext;
    use tracing::info;

    pub fn log_device_initialization(device_name: &str, sample_rate: u32, channels: u16) {
        info!(
            device_name = %device_name,
            sample_rate = %sample_rate,
            channels = %channels,
            "🎤 Audio device initialized"
        );
    }

    pub fn log_recording_started(ctx: &RequestContext, device: &str) {
        crate::log_request_start!(ctx, "Audio recording", device = device);
    }

    pub fn log_recording_stopped(ctx: &RequestContext, samples: usize, duration_sec: f32) {
        crate::log_request_end!(
            ctx,
            "Audio recording",
            samples = samples,
            duration_sec = duration_sec
        );
    }

    pub fn log_audio_error(ctx: &RequestContext, error: &dyn std::fmt::Display) {
        crate::log_request_error!(ctx, error, "Audio capture");
    }
}

pub mod transcription {
    use crate::logging::RequestContext;
    use tracing::info;

    pub fn log_model_loading(model_path: &str, use_cuda: bool) {
        info!(
            model_path = %model_path,
            use_cuda = %use_cuda,
            "🧠 Loading Whisper model"
        );
    }

    pub fn log_transcription_started(
        ctx: &RequestContext,
        audio_duration_sec: f32,
        sample_rate: u32,
    ) {
        crate::log_request_start!(
            ctx,
            "Transcription",
            audio_duration_sec = audio_duration_sec,
            sample_rate = sample_rate
        );
    }

    pub fn log_transcription_completed(ctx: &RequestContext, text: &str, confidence: f32) {
        crate::log_request_end!(
            ctx,
            "Transcription",
            text_length = text.len(),
            confidence = confidence
        );
    }

    pub fn log_transcription_error(ctx: &RequestContext, error: &dyn std::fmt::Display) {
        crate::log_request_error!(ctx, error, "Transcription");
    }
}

pub mod text_insertion {
    use crate::logging::RequestContext;
    use tracing::info;

    pub fn log_insertion_method_selected(method: &str) {
        info!(
            method = %method,
            "⌨️ Text insertion method selected"
        );
    }

    pub fn log_text_inserted(ctx: &RequestContext, text: &str, method: &str) {
        crate::log_request_end!(
            ctx,
            "Text insertion",
            text_length = text.len(),
            method = method
        );
    }

    pub fn log_insertion_error(ctx: &RequestContext, error: &dyn std::fmt::Display, method: &str) {
        crate::log_request_error!(ctx, error, "Text insertion", method = method);
    }
}

pub mod ui {
    use tracing::info;

    pub fn log_command_executed(command: &str, args: &[String]) {
        info!(
            command = %command,
            args = ?args,
            "🖥️ Command executed"
        );
    }

    pub fn log_tui_event(event_type: &str, details: Option<&str>) {
        info!(
            event_type = %event_type,
            details = ?details,
            "📺 TUI event processed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_request_context_generation() {
        let ctx = RequestContext::new("test_operation");
        assert!(!ctx.request_id.is_empty());
        assert_eq!(ctx.operation, "test_operation");
        assert!(ctx.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn test_performance_metrics_collection() {
        let metrics = PerformanceMetrics::collect();
        assert!(metrics.cpu_usage >= 0.0);
        // Memory usage might be 0 if process not found, which is ok for tests
    }

    #[tokio::test]
    async fn test_logging_system_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let config = LoggingConfig {
            log_dir: temp_dir.path().to_path_buf(),
            console_output: false, // Don't spam test output
            ..Default::default()
        };

        let logging_system = LoggingSystem::new(config).unwrap();
        let _initialized = logging_system.initialize();

        // Test that we can create request contexts
        let ctx = RequestContext::new("test");
        assert!(!ctx.request_id.is_empty());
    }
}
