use crate::tui::app::{App, AppScreen, FocusedWidget};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tokio::time::sleep;

/// Handle terminal events and return true if app should quit
pub async fn handle_events(app: &mut App) -> crate::Result<bool> {
    // Check for hotkey events first
    app.check_hotkey_events();
    
    // Check for terminal events with timeout
    if event::poll(Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            handle_key_event(app, key).await?;
        }
    }
    
    // Check if app should quit
    Ok(app.should_quit)
}

async fn handle_key_event(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    // Global key handlers (work on any screen)
    match key {
        // Quit application
        KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            app.should_quit = true;
            return Ok(());
        }
        
        // Force quit with Ctrl+C
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            app.should_quit = true;
            return Ok(());
        }
        
        // Toggle help overlay
        KeyEvent {
            code: KeyCode::Char('?'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            app.show_help_overlay = !app.show_help_overlay;
            return Ok(());
        }
        
        // Close help overlay
        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } if app.show_help_overlay => {
            app.show_help_overlay = false;
            return Ok(());
        }
        
        // Navigate back
        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            app.navigate_back();
            return Ok(());
        }
        
        // Quick screen navigation
        KeyEvent {
            code: KeyCode::Char('1'),
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            app.navigate_to_screen(AppScreen::Dashboard);
            return Ok(());
        }
        
        KeyEvent {
            code: KeyCode::Char('2'),
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            app.navigate_to_screen(AppScreen::ModeSelection);
            return Ok(());
        }
        
        KeyEvent {
            code: KeyCode::Char('3'),
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            app.navigate_to_screen(AppScreen::Recording);
            return Ok(());
        }
        
        KeyEvent {
            code: KeyCode::Char('4'),
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            app.navigate_to_screen(AppScreen::AudioDeviceSelection);
            return Ok(());
        }
        
        KeyEvent {
            code: KeyCode::Char('5'),
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            app.navigate_to_screen(AppScreen::ModelSelection);
            return Ok(());
        }
        
        KeyEvent {
            code: KeyCode::Char('6'),
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            app.navigate_to_screen(AppScreen::ConfigEditor);
            return Ok(());
        }
        
        // Manual recording controls (global)
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            ..
        } if !app.show_help_overlay => {
            handle_manual_recording_toggle(app).await?;
            return Ok(());
        }
        
        _ => {}
    }
    
    // Don't handle other events when help is shown
    if app.show_help_overlay {
        return Ok(());
    }
    
    // Screen-specific key handlers
    match app.current_screen {
        AppScreen::Dashboard => handle_dashboard_keys(app, key).await?,
        AppScreen::ModeSelection => handle_mode_selection_keys(app, key).await?,
        AppScreen::AudioDeviceSelection => handle_audio_device_keys(app, key).await?,
        AppScreen::ModelSelection => handle_model_selection_keys(app, key).await?,
        AppScreen::ConfigFileSelection => handle_config_file_keys(app, key).await?,
        AppScreen::HotkeyConfiguration => handle_hotkey_config_keys(app, key).await?,
        AppScreen::Recording => handle_recording_keys(app, key).await?,
        AppScreen::ConfigEditor => handle_config_editor_keys(app, key).await?,
        AppScreen::Help => handle_help_keys(app, key).await?,
    }
    
    Ok(())
}

async fn handle_manual_recording_toggle(app: &mut App) -> crate::Result<()> {
    use std::sync::atomic::Ordering;
    
    if app.is_recording.load(Ordering::Relaxed) {
        app.stop_recording().await?;
    } else {
        app.start_recording().await?;
    }
    Ok(())
}

async fn handle_dashboard_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    match key.code {
        KeyCode::Enter => {
            match app.focused_widget {
                FocusedWidget::ModeList => app.navigate_to_screen(AppScreen::ModeSelection),
                FocusedWidget::AudioDeviceList => app.navigate_to_screen(AppScreen::AudioDeviceSelection),
                FocusedWidget::ModelList => app.navigate_to_screen(AppScreen::ModelSelection),
                FocusedWidget::ConfigFileList => app.navigate_to_screen(AppScreen::ConfigFileSelection),
                FocusedWidget::RecordingControls => app.navigate_to_screen(AppScreen::Recording),
                _ => {}
            }
        }
        KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
            cycle_dashboard_focus(app, true);
        }
        KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
            cycle_dashboard_focus(app, false);
        }
        KeyCode::Char('r') => {
            app.navigate_to_screen(AppScreen::Recording);
        }
        KeyCode::Char('c') => {
            app.navigate_to_screen(AppScreen::ConfigEditor);
        }
        _ => {}
    }
    Ok(())
}

async fn handle_mode_selection_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_mode_index > 0 {
                app.selected_mode_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected_mode_index < 3 { // 4 modes total
                app.selected_mode_index += 1;
            }
        }
        KeyCode::Enter => {
            select_mode(app);
            app.navigate_back();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            // Navigate to mode parameters if applicable
            match app.selected_mode_index {
                1 => { // OneShot mode
                    // Could open duration/print_only configuration
                }
                _ => {}
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Navigate to mode parameters if applicable
        }
        _ => {}
    }
    Ok(())
}

async fn handle_audio_device_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_audio_device_index > 0 {
                app.selected_audio_device_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected_audio_device_index < app.available_audio_devices.len().saturating_sub(1) {
                app.selected_audio_device_index += 1;
            }
        }
        KeyCode::Enter => {
            // Apply audio device selection
            let selected_index = app.selected_audio_device_index;
            if let Some(device_name) = app.available_audio_devices.get(selected_index).cloned() {
                // Reinitialize audio capture with new device
                match crate::AudioCapture::new(Some(&device_name)) {
                    Ok(audio_capture) => {
                        app.audio_capture = Some(audio_capture);
                        app.update_system_status();
                        app.add_log(crate::tui::app::LogLevel::Info, 
                                  format!("Audio device changed to: {}", device_name));
                    }
                    Err(e) => {
                        app.add_log(crate::tui::app::LogLevel::Error, 
                                  format!("Failed to change audio device: {}", e));
                    }
                }
            }
            app.navigate_back();
        }
        _ => {}
    }
    Ok(())
}

async fn handle_model_selection_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_model_index > 0 {
                app.selected_model_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected_model_index < app.available_models.len().saturating_sub(1) {
                app.selected_model_index += 1;
            }
        }
        KeyCode::Enter => {
            // Apply model selection
            if let Some(model) = app.available_models.get(app.selected_model_index) {
                if model.is_available {
                    // Reinitialize transcriber with new model
                    match crate::WhisperTranscriber::new(&model.path, app.config.transcription.use_cuda).await {
                        Ok(transcriber) => {
                            app.transcriber = Some(transcriber);
                            app.config.transcription.model_path = model.path.to_string_lossy().to_string();
                            app.update_system_status();
                            app.add_log(crate::tui::app::LogLevel::Info, 
                                      format!("Model changed to: {} ({})", model.name, model.size));
                        }
                        Err(e) => {
                            app.add_log(crate::tui::app::LogLevel::Error, 
                                      format!("Failed to change model: {}", e));
                        }
                    }
                } else {
                    app.add_log(crate::tui::app::LogLevel::Warn, 
                              format!("Model {} is not available", model.name));
                }
            }
            app.navigate_back();
        }
        _ => {}
    }
    Ok(())
}

async fn handle_config_file_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_config_file_index > 0 {
                app.selected_config_file_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected_config_file_index < app.available_config_files.len().saturating_sub(1) {
                app.selected_config_file_index += 1;
            }
        }
        KeyCode::Enter => {
            // Load selected config file
            if let Some(config_path) = app.available_config_files.get(app.selected_config_file_index) {
                match crate::Config::load_from_file(config_path) {
                    Ok(new_config) => {
                        app.config = new_config;
                        // Reinitialize components with new config would go here
                        app.add_log(crate::tui::app::LogLevel::Info, 
                                  format!("Configuration loaded from: {}", config_path.display()));
                    }
                    Err(e) => {
                        app.add_log(crate::tui::app::LogLevel::Error, 
                                  format!("Failed to load config: {}", e));
                    }
                }
            }
            app.navigate_back();
        }
        _ => {}
    }
    Ok(())
}

async fn handle_hotkey_config_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    // This would handle hotkey combination input
    // For now, just navigate back
    match key.code {
        KeyCode::Enter => {
            app.navigate_back();
        }
        _ => {}
    }
    Ok(())
}

async fn handle_recording_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    match key.code {
        KeyCode::Char('r') | KeyCode::Enter => {
            handle_manual_recording_toggle(app).await?;
        }
        KeyCode::Char('s') => {
            // Stop recording if active
            use std::sync::atomic::Ordering;
            if app.is_recording.load(Ordering::Relaxed) {
                app.stop_recording().await?;
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_config_editor_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.config_tree_selected > 0 {
                app.config_tree_selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.config_tree_selected += 1; // Would need bounds checking based on config tree size
        }
        KeyCode::Enter => {
            // Enter edit mode for selected config item
        }
        _ => {}
    }
    Ok(())
}

async fn handle_help_keys(app: &mut App, key: KeyEvent) -> crate::Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.navigate_back();
        }
        _ => {}
    }
    Ok(())
}

fn cycle_dashboard_focus(app: &mut App, forward: bool) {
    let widgets = vec![
        FocusedWidget::ModeList,
        FocusedWidget::AudioDeviceList,
        FocusedWidget::ModelList,
        FocusedWidget::RecordingControls,
    ];
    
    let current_index = widgets.iter().position(|w| {
        std::mem::discriminant(w) == std::mem::discriminant(&app.focused_widget)
    }).unwrap_or(0);
    
    let new_index = if forward {
        (current_index + 1) % widgets.len()
    } else {
        if current_index == 0 {
            widgets.len() - 1
        } else {
            current_index - 1
        }
    };
    
    app.focused_widget = widgets[new_index].clone();
}

fn select_mode(app: &mut App) {
    use crate::tui::app::AppMode;
    
    app.mode = match app.selected_mode_index {
        0 => AppMode::Daemon { elevated: app.daemon_elevated },
        1 => AppMode::OneShot { 
            duration: app.oneshot_duration, 
            print_only: app.oneshot_print_only 
        },
        2 => AppMode::Manual,
        3 => AppMode::Status,
        _ => AppMode::Daemon { elevated: false },
    };
}