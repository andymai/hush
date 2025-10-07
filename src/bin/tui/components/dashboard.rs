use crate::tui::simple_app::{SimpleApp as App, AppMode, LogLevel};
use crate::tui::components::ComponentHelpers;
use ratatui::{
    prelude::*,
    widgets::*,
};
use std::sync::atomic::Ordering;

pub struct Dashboard;

impl Dashboard {
    pub fn render(f: &mut Frame, app: &App, area: Rect) {
        // Create main layout: header, body, footer
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),   // Header
                Constraint::Min(10),     // Body
                Constraint::Length(3),   // Footer
            ])
            .split(area);

        // Render header
        Self::render_header(f, app, main_chunks[0]);
        
        // Create body layout: left panel (status), right panel (controls)
        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Status panel
                Constraint::Percentage(40), // Controls panel
            ])
            .split(main_chunks[1]);

        // Render status panel
        Self::render_status_panel(f, app, body_chunks[0]);
        
        // Render controls panel
        Self::render_controls_panel(f, app, body_chunks[1]);
        
        // Render footer
        Self::render_footer(f, app, main_chunks[2]);
    }

    fn render_header(f: &mut Frame, app: &App, area: Rect) {
        let title = "🤫 Hush - Voice-to-Text TUI";
        
        // Create mode indicator
        let mode_text = match &app.mode {
            AppMode::Daemon { elevated } => {
                if *elevated { 
                    "DAEMON (Elevated)" 
                } else { 
                    "DAEMON" 
                }
            }
            AppMode::OneShot { duration, print_only } => {
                if *print_only {
                    &format!("ONE-SHOT ({}s, Print)", duration)
                } else {
                    &format!("ONE-SHOT ({}s)", duration)
                }
            }
            AppMode::Manual => "MANUAL",
            AppMode::Status => "STATUS",
        };

        let mode_color = match &app.mode {
            AppMode::Daemon { .. } => Color::Green,
            AppMode::OneShot { .. } => Color::Blue,
            AppMode::Manual => Color::Yellow,
            AppMode::Status => Color::Cyan,
        };

        // Create recording indicator
        let is_recording = app.is_recording.load(Ordering::Relaxed);
        let recording_text = if is_recording {
            format!("🎤 RECORDING ({:.1}s)", app.get_recording_duration().as_secs_f32())
        } else {
            "⏹️  READY".to_string()
        };
        
        let recording_color = if is_recording { Color::Red } else { Color::Green };

        let header_text = vec![
            Span::styled(title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled(mode_text, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled(recording_text, Style::default().fg(recording_color).add_modifier(Modifier::BOLD)),
        ];

        let header = Paragraph::new(Line::from(header_text))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)))
            .alignment(Alignment::Center);

        f.render_widget(header, area);
    }

    fn render_status_panel(f: &mut Frame, app: &App, area: Rect) {
        // Split status panel into sections
        let status_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),     // System status
                Constraint::Min(6),     // Recent activity
            ])
            .split(area);

        Self::render_system_status(f, app, status_chunks[0]);
        Self::render_recent_activity(f, app, status_chunks[1]);
    }

    fn render_system_status(f: &mut Frame, app: &App, area: Rect) {
        let status = &app.system_status;
        
        let status_items = vec![
            // Audio system
            Line::from(vec![
                Span::raw("Audio Device: "),
                Span::styled(
                    &status.audio_device_name,
                    Style::default().fg(ComponentHelpers::status_color(
                        status.audio_device_available,
                        false
                    ))
                ),
            ]),
            
            // Transcriber system
            Line::from(vec![
                Span::raw("Transcriber: "),
                Span::styled(
                    &status.transcriber_device,
                    Style::default().fg(ComponentHelpers::status_color(
                        status.transcriber_ready,
                        false
                    ))
                ),
                Span::raw(if status.transcriber_cuda_enabled { " (CUDA)" } else { " (CPU)" }),
            ]),
            
            // Hotkey system
            Line::from(vec![
                Span::raw("Hotkeys: "),
                Span::styled(
                    &status.hotkey_combination,
                    Style::default().fg(ComponentHelpers::status_color(
                        status.hotkey_available,
                        false
                    ))
                ),
                Span::styled(
                    if status.hotkey_available { " ✓" } else { " ✗" },
                    Style::default().fg(ComponentHelpers::status_color(
                        status.hotkey_available,
                        false
                    ))
                ),
            ]),
            
            // Text insertion system
            Line::from(vec![
                Span::raw("Text Insertion: "),
                Span::styled(
                    if status.text_insertion_available { "Available" } else { "Unavailable" },
                    Style::default().fg(ComponentHelpers::status_color(
                        status.text_insertion_available,
                        false
                    ))
                ),
            ]),
            
            // Notifications
            Line::from(vec![
                Span::raw("Notifications: "),
                Span::styled(
                    if app.notifications_enabled { "Enabled" } else { "Disabled" },
                    Style::default().fg(if app.notifications_enabled { 
                        Color::Green 
                    } else { 
                        Color::Gray 
                    })
                ),
            ]),
            
            // Last transcription (if available)
            if !app.last_transcription.is_empty() {
                Line::from(vec![
                    Span::raw("Last: "),
                    Span::styled(
                        &app.last_transcription[..app.last_transcription.len().min(40)],
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC)
                    ),
                    if app.last_transcription.len() > 40 {
                        Span::raw("...")
                    } else {
                        Span::raw("")
                    },
                ])
            } else {
                Line::from(Span::raw(""))
            },
        ];

        let system_status = Paragraph::new(status_items)
            .block(ComponentHelpers::create_block("System Status", false))
            .wrap(Wrap { trim: true });

        f.render_widget(system_status, area);
    }

    fn render_recent_activity(f: &mut Frame, app: &App, area: Rect) {
        // Show recent log entries
        let recent_logs: Vec<Line> = app.logs.iter()
            .rev()
            .take((area.height.saturating_sub(2)) as usize) // Account for borders
            .map(|log| {
                let level_color = match log.level {
                    LogLevel::Info => Color::White,
                    LogLevel::Warn => Color::Yellow,
                    LogLevel::Error => Color::Red,
                    LogLevel::Debug => Color::Gray,
                };
                
                let level_symbol = match log.level {
                    LogLevel::Info => "ℹ",
                    LogLevel::Warn => "⚠",
                    LogLevel::Error => "✗",
                    LogLevel::Debug => "🐛",
                };
                
                Line::from(vec![
                    Span::styled(level_symbol, Style::default().fg(level_color)),
                    Span::raw(" "),
                    Span::styled(&log.message, Style::default().fg(level_color)),
                ])
            })
            .collect();

        let activity = Paragraph::new(recent_logs)
            .block(ComponentHelpers::create_block("Recent Activity", false))
            .wrap(Wrap { trim: true });

        f.render_widget(activity, area);
    }

    fn render_controls_panel(f: &mut Frame, app: &App, area: Rect) {
        // Split controls panel into sections
        let control_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),     // Navigation shortcuts
                Constraint::Min(4),     // Quick actions
            ])
            .split(area);

        Self::render_navigation_shortcuts(f, app, control_chunks[0]);
        Self::render_quick_actions(f, app, control_chunks[1]);
    }

    fn render_navigation_shortcuts(f: &mut Frame, _app: &App, area: Rect) {
        let shortcuts = vec![
            Line::from(vec![
                Span::styled("1", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" Dashboard (current)"),
            ]),
            Line::from(vec![
                Span::styled("2", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" Mode Selection"),
            ]),
            Line::from(vec![
                Span::styled("3", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" Recording Interface"),
            ]),
            Line::from(vec![
                Span::styled("4", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" Audio Device Selection"),
            ]),
            Line::from(vec![
                Span::styled("5", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" Model Selection"),
            ]),
            Line::from(vec![
                Span::styled("6", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" Configuration Editor"),
            ]),
            Line::from(vec![
                Span::styled("Tab", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" Navigate widgets"),
            ]),
            Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" Select/Open"),
            ]),
        ];

        let navigation = Paragraph::new(shortcuts)
            .block(ComponentHelpers::create_block("Navigation", false));

        f.render_widget(navigation, area);
    }

    fn render_quick_actions(f: &mut Frame, app: &App, area: Rect) {
        let is_recording = app.is_recording.load(Ordering::Relaxed);
        
        let actions = vec![
            Line::from(vec![
                Span::styled("Space", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                if is_recording {
                    Span::raw(" Stop Recording")
                } else {
                    Span::raw(" Start Recording")
                }
            ]),
            Line::from(vec![
                Span::styled("?", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::raw(" Show Help"),
            ]),
            Line::from(vec![
                Span::styled("q", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" Quit"),
            ]),
        ];

        let quick_actions = Paragraph::new(actions)
            .block(ComponentHelpers::create_block("Quick Actions", false));

        f.render_widget(quick_actions, area);
    }

    fn render_footer(f: &mut Frame, _app: &App, area: Rect) {
        let footer_text = vec![
            Span::raw("Hush TUI v0.1.0 | "),
            Span::styled("ESC", Style::default().fg(Color::Yellow)),
            Span::raw(" Back | "),
            Span::styled("?", Style::default().fg(Color::Magenta)),
            Span::raw(" Help | "),
            Span::styled("Ctrl+C", Style::default().fg(Color::Red)),
            Span::raw(" Quit"),
        ];

        let footer = Paragraph::new(Line::from(footer_text))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)))
            .alignment(Alignment::Center);

        f.render_widget(footer, area);
    }
}