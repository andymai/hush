use crate::tui::simple_app::{SimpleApp as App, AppScreen};
use crate::tui::components::{Dashboard};
use ratatui::{prelude::*, widgets::*};

/// Main UI rendering function
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    
    // Render based on current screen
    match app.current_screen {
        AppScreen::Dashboard => {
            Dashboard::render(f, app, area);
        }
        AppScreen::ModeSelection => {
            render_mode_selection(f, app, area);
        }
        AppScreen::AudioDeviceSelection => {
            render_audio_device_selection(f, app, area);
        }
        AppScreen::ModelSelection => {
            render_model_selection(f, app, area);
        }
        AppScreen::ConfigFileSelection => {
            render_config_file_selection(f, app, area);
        }
        AppScreen::HotkeyConfiguration => {
            render_hotkey_configuration(f, app, area);
        }
        AppScreen::Recording => {
            render_recording_interface(f, app, area);
        }
        AppScreen::ConfigEditor => {
            render_config_editor(f, app, area);
        }
        AppScreen::Help => {
            render_help(f, app, area);
        }
    }
    
    // Render help overlay if active
    if app.show_help_overlay {
        render_help_overlay(f, app);
    }
}

fn render_mode_selection(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Mode Selection")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let modes = vec![
        "Daemon - Global hotkey support",
        "One-Shot - Record once and exit", 
        "Manual - Manual recording control",
        "Status - Show system status",
    ];
    
    let items: Vec<ListItem> = modes
        .iter()
        .enumerate()
        .map(|(i, mode)| {
            let style = if i == app.selected_mode_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(*mode).style(style)
        })
        .collect();
    
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::Blue));
    
    f.render_widget(list, area);
    
    // Add help text
    let help_text = "Use ↑↓ to navigate, Enter to select, ESC to go back";
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    
    let help_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };
    f.render_widget(help, help_area);
}

fn render_audio_device_selection(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Audio Device Selection")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let items: Vec<ListItem> = app.available_audio_devices
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let style = if i == app.selected_audio_device_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(device.as_str()).style(style)
        })
        .collect();
    
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::Blue));
    
    f.render_widget(list, area);
}

fn render_model_selection(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Model Selection")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let items: Vec<ListItem> = app.available_models
        .iter()
        .enumerate()
        .map(|(i, model)| {
            let style = if i == app.selected_model_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else if model.is_available {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };
            
            let status = if model.is_available { "✓" } else { "✗" };
            let text = format!("{} {} ({}) {}", status, model.name, model.size, 
                             if model.is_available { "" } else { "(Not Downloaded)" });
            
            ListItem::new(text).style(style)
        })
        .collect();
    
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::Blue));
    
    f.render_widget(list, area);
}

fn render_config_file_selection(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Configuration File Selection")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let items: Vec<ListItem> = app.available_config_files
        .iter()
        .enumerate()
        .map(|(i, config_file)| {
            let style = if i == app.selected_config_file_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(config_file.display().to_string()).style(style)
        })
        .collect();
    
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::Blue));
    
    f.render_widget(list, area);
}

fn render_hotkey_configuration(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Hotkey Configuration")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let text = vec![
        Line::from("Current hotkey combination:"),
        Line::from(Span::styled(&app.temp_hotkey_combination, Style::default().fg(Color::Cyan))),
        Line::from(""),
        Line::from("Press Enter to keep current, ESC to cancel"),
    ];
    
    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    
    f.render_widget(paragraph, area);
}

fn render_recording_interface(f: &mut Frame, app: &App, area: Rect) {
    use std::sync::atomic::Ordering;
    
    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // Status
            Constraint::Min(8),      // Main area
            Constraint::Length(4),   // Controls
        ])
        .split(area);
    
    // Recording status
    let is_recording = app.is_recording.load(Ordering::Relaxed);
    let status_text = if is_recording {
        format!("🎤 RECORDING - {:.1}s", app.get_recording_duration().as_secs_f32())
    } else {
        "⏹️ READY TO RECORD".to_string()
    };
    
    let status_color = if is_recording { Color::Red } else { Color::Green };
    let status = Paragraph::new(status_text)
        .style(Style::default().fg(status_color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    
    f.render_widget(status, chunks[0]);
    
    // Main recording area
    let main_text = if is_recording {
        vec![
            Line::from("🎙️ Listening..."),
            Line::from(""),
            Line::from("Speak now, or press Space/Enter to stop recording"),
        ]
    } else if !app.last_transcription.is_empty() {
        vec![
            Line::from("Last transcription:"),
            Line::from(""),
            Line::from(Span::styled(&app.last_transcription, Style::default().fg(Color::Cyan))),
            Line::from(""),
            Line::from("Press Space/Enter to start new recording"),
        ]
    } else {
        vec![
            Line::from("Ready to record"),
            Line::from(""),
            Line::from("Press Space/Enter to start recording"),
        ]
    };
    
    let main_block = Paragraph::new(main_text)
        .alignment(Alignment::Center)
        .block(Block::default()
            .title("Recording Interface")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)));
    
    f.render_widget(main_block, chunks[1]);
    
    // Controls
    let controls_text = vec![
        Line::from(vec![
            Span::styled("Space/Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            if is_recording {
                Span::raw(" Stop Recording")
            } else {
                Span::raw(" Start Recording")
            }
        ]),
        Line::from(vec![
            Span::styled("ESC", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Back to Dashboard")
        ]),
    ];
    
    let controls = Paragraph::new(controls_text)
        .alignment(Alignment::Center)
        .block(Block::default()
            .title("Controls")
            .borders(Borders::ALL));
    
    f.render_widget(controls, chunks[2]);
}

fn render_config_editor(f: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .title("Configuration Editor")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let text = vec![
        Line::from("Configuration Editor"),
        Line::from(""),
        Line::from("(Under construction)"),
        Line::from(""),
        Line::from("Press ESC to go back"),
    ];
    
    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    
    f.render_widget(paragraph, area);
}

fn render_help(f: &mut Frame, _app: &App, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled("Hush TUI Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("Navigation:"),
        Line::from("  1-6    - Quick screen navigation"),
        Line::from("  Tab    - Navigate between widgets"),
        Line::from("  ↑↓     - Navigate lists (also j/k)"),
        Line::from("  Enter  - Select/confirm"),
        Line::from("  ESC    - Go back"),
        Line::from("  ?      - Show/hide this help"),
        Line::from("  q      - Quit application"),
        Line::from(""),
        Line::from("Recording:"),
        Line::from("  Space  - Toggle recording (global)"),
        Line::from(""),
        Line::from("Screens:"),
        Line::from("  1 - Dashboard (system status)"),
        Line::from("  2 - Mode selection"),
        Line::from("  3 - Recording interface"),
        Line::from("  4 - Audio device selection"),
        Line::from("  5 - Model selection"),
        Line::from("  6 - Configuration editor"),
    ];
    
    let help = Paragraph::new(help_text)
        .block(Block::default()
            .title("Help")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)))
        .wrap(Wrap { trim: true });
    
    f.render_widget(help, area);
}

fn render_help_overlay(f: &mut Frame, app: &App) {
    let area = f.area();
    
    // Create popup area (centered, 60% of screen)
    let popup_area = Rect {
        x: area.width / 5,
        y: area.height / 5,
        width: (area.width * 3) / 5,
        height: (area.height * 3) / 5,
    };
    
    // Clear background
    let clear = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(clear, popup_area);
    
    // Render help content
    render_help(f, app, popup_area);
}