use crate::Result;
use crate::logging::{RequestContext, audio as logging};
use cpal::{Device, Host, Stream, StreamConfig, SampleRate};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::mpsc;
use tracing::{info, warn, error, debug, trace};

pub struct AudioCapture {
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<Mutex<bool>>,
    simulated_mode: bool,
    amplitude_tx: Option<mpsc::Sender<f32>>,
}

/// Calculate RMS (Root Mean Square) amplitude from audio samples
/// Returns a value between 0.0 and 1.0
fn calculate_rms_amplitude(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
    let rms = (sum_squares / samples.len() as f32).sqrt();

    // Normalize to 0.0-1.0 range (assuming typical speech is around 0.1-0.3 RMS)
    // Apply a multiplier and clamp to get good visual response
    (rms * 3.0).min(1.0)
}

impl AudioCapture {
    pub fn new(device_name: Option<&str>) -> Result<Self> {
        let ctx = RequestContext::new("audio_capture_init")
            .with_metadata("device_requested", device_name.unwrap_or("default"));
        
        info!(
            request_id = %ctx.request_id,
            device_name = ?device_name,
            "🎤 Initializing audio capture system"
        );
        
        // Get the default host (will use PipeWire/JACK on Linux)
        let host = cpal::default_host();
        let host_name = host.id().name();
        debug!(
            request_id = %ctx.request_id,
            host_name = %host_name,
            "Audio host selected"
        );
        
        // Select audio device
        let device = match device_name {
            Some(name) => {
                debug!(
                    request_id = %ctx.request_id,
                    device_name = %name,
                    "Looking for specific device"
                );
                Self::find_device_by_name(&host, name).map_err(|e| {
                    error!(
                        request_id = %ctx.request_id,
                        device_name = %name,
                        error = %e,
                        "Failed to find requested device"
                    );
                    e
                })?
            },
            None => {
                debug!(
                    request_id = %ctx.request_id,
                    "Using default input device"
                );
                match host.default_input_device() {
                    Some(device) => device,
                    None => {
                        warn!(
                            request_id = %ctx.request_id,
                            "No default input device available, trying alternatives"
                        );
                        Self::find_working_input_device(&host).map_err(|e| {
                            error!(
                                request_id = %ctx.request_id,
                                error = %e,
                                "No working input device found"
                            );
                            e
                        })?
                    }
                }
            }
        };
        
        let device_name_str = device.name().unwrap_or("Unknown".to_string());
        logging::log_device_initialization(&device_name_str, 16000, 1);
        
        info!(
            request_id = %ctx.request_id,
            device_name = %device_name_str,
            "✅ Audio device selected successfully"
        );
        
        // Get supported input config, with fallback to working device
        let (device, _supported_config) = match device.default_input_config() {
            Ok(config) => {
                info!("Default config: {:?}", config);
                (device, config)
            },
            Err(e) => {
                warn!("Failed to get default input config from {}: {}", 
                      device.name().unwrap_or("Unknown".to_string()), e);
                warn!("Trying to find alternative working device...");
                
                match Self::find_working_input_device(&host) {
                    Ok(working_device) => {
                        let config = working_device.default_input_config()
                            .map_err(|e| anyhow::anyhow!("Working device config failed: {}", e))?;
                        info!("Found working device: {}", working_device.name().unwrap_or("Unknown".to_string()));
                        info!("Working device config: {:?}", config);
                        (working_device, config)
                    },
                    Err(_) => {
                        warn!("No working audio devices found");
                        warn!("Audio capture will be simulated for development/testing");
                        return Ok(AudioCapture {
                            device,
                            config: StreamConfig {
                                channels: 1,
                                sample_rate: SampleRate(16000),
                                buffer_size: cpal::BufferSize::Fixed(1024),
                            },
                            stream: None,
                            buffer: Arc::new(Mutex::new(Vec::new())),
                            is_recording: Arc::new(Mutex::new(false)),
                            simulated_mode: true,
                            amplitude_tx: None,
                        });
                    }
                }
            }
        };
        
        // Create our desired config (16kHz mono for Whisper)
        let config = StreamConfig {
            channels: 1, // Mono for speech recognition
            sample_rate: SampleRate(16000), // Optimal for Whisper
            buffer_size: cpal::BufferSize::Fixed(1024),
        };
        
        // Verify the device supports our desired config
        if !Self::is_config_supported(&device, &config) {
            warn!("Desired config not supported, falling back to default");
            // Fall back to supported config but convert to our needs
        }
        
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let is_recording = Arc::new(Mutex::new(false));

        Ok(AudioCapture {
            device,
            config,
            stream: None,
            buffer,
            is_recording,
            simulated_mode: false,
            amplitude_tx: None,
        })
    }
    
    pub fn start_recording(&mut self) -> Result<()> {
        let ctx = RequestContext::new("audio_recording")
            .with_metadata("simulated", &self.simulated_mode.to_string())
            .with_metadata("device", &self.get_device_name());
        
        logging::log_recording_started(&ctx, &self.get_device_name());
        
        if self.stream.is_some() {
            warn!(
                request_id = %ctx.request_id,
                "Recording already in progress, ignoring start request"
            );
            return Ok(());
        }
        
        // Clear the buffer and mark as recording
        let buffer_len_before = self.buffer.lock().len();
        self.buffer.lock().clear();
        *self.is_recording.lock() = true;
        
        debug!(
            request_id = %ctx.request_id,
            buffer_cleared_samples = %buffer_len_before,
            "Audio buffer cleared and recording flag set"
        );
        
        // Handle simulated mode for testing
        if self.simulated_mode {
            info!(
                request_id = %ctx.request_id,
                "🧪 Using simulated audio recording for testing"
            );
            return Ok(());
        }
        
        let buffer_clone = Arc::clone(&self.buffer);
        let is_recording_clone = Arc::clone(&self.is_recording);
        let amplitude_tx_clone = self.amplitude_tx.clone();

        // Build the input stream
        let stream = self.device.build_input_stream(
            &self.config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if *is_recording_clone.lock() {
                    buffer_clone.lock().extend_from_slice(data);

                    // Calculate and send amplitude for waveform visualization
                    if let Some(ref tx) = amplitude_tx_clone {
                        let amplitude = calculate_rms_amplitude(data);
                        let _ = tx.send(amplitude); // Ignore send errors (non-blocking)
                    }
                }
            },
            |err| {
                error!("Audio stream error: {}", err);
            },
            None,
        )
        .map_err(|e| anyhow::anyhow!("Failed to build input stream: {}", e))?;
        
        // Start the stream
        stream.play()
            .map_err(|e| anyhow::anyhow!("Failed to start audio stream: {}", e))?;
        
        self.stream = Some(stream);
        info!("Audio recording started successfully");
        Ok(())
    }
    
    pub fn stop_recording(&mut self) -> Result<Vec<f32>> {
        let ctx = RequestContext::new("audio_recording_stop")
            .with_metadata("simulated", &self.simulated_mode.to_string())
            .with_metadata("device", &self.get_device_name());
        
        debug!(
            request_id = %ctx.request_id,
            simulated = %self.simulated_mode,
            "🛑 Stopping audio recording"
        );
        
        *self.is_recording.lock() = false;
        
        // Handle simulated mode with fake audio data
        if self.simulated_mode {
            // Generate 3 seconds of simulated audio data (silence for now)
            let duration_samples = (3.0 * self.config.sample_rate.0 as f32) as usize;
            let simulated_data = vec![0.0f32; duration_samples];
            
            info!(
                request_id = %ctx.request_id,
                samples_generated = %simulated_data.len(),
                duration_sec = %3.0,
                "🧪 Generated simulated audio data"
            );
            
            logging::log_recording_stopped(&ctx, simulated_data.len(), 3.0);
            return Ok(simulated_data);
        }
        
        // Stop the actual audio stream
        if let Some(stream) = self.stream.take() {
            if let Err(e) = stream.pause() {
                error!(
                    request_id = %ctx.request_id,
                    error = %e,
                    "Failed to stop audio stream cleanly"
                );
                return Err(anyhow::anyhow!("Failed to stop audio stream: {}", e));
            }
            
            debug!(
                request_id = %ctx.request_id,
                "Audio stream stopped successfully"
            );
        }
        
        // Extract recorded data from buffer
        let recorded_data = {
            let mut buffer = self.buffer.lock();
            let data = buffer.clone();
            let buffer_size = buffer.len();
            buffer.clear();
            
            trace!(
                request_id = %ctx.request_id,
                buffer_size = %buffer_size,
                "Audio buffer extracted and cleared"
            );
            
            data
        };
        
        let duration_sec = recorded_data.len() as f32 / self.config.sample_rate.0 as f32;
        
        logging::log_recording_stopped(&ctx, recorded_data.len(), duration_sec);
        
        info!(
            request_id = %ctx.request_id,
            samples = %recorded_data.len(),
            duration_sec = %duration_sec,
            sample_rate = %self.config.sample_rate.0,
            "✅ Audio recording completed successfully"
        );
        
        Ok(recorded_data)
    }
    
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock()
    }
    
    pub fn get_device_name(&self) -> String {
        self.device.name().unwrap_or("Unknown Device".to_string())
    }

    /// Enable amplitude monitoring and return the receiver channel
    /// Call this before start_recording() to receive amplitude updates
    pub fn enable_amplitude_monitoring(&mut self) -> mpsc::Receiver<f32> {
        let (tx, rx) = mpsc::channel();
        self.amplitude_tx = Some(tx);
        rx
    }
    
    fn find_device_by_name(host: &Host, name: &str) -> Result<Device> {
        let devices = host.input_devices()
            .map_err(|e| anyhow::anyhow!("Failed to enumerate input devices: {}", e))?;
        
        for device in devices {
            if let Ok(device_name) = device.name() {
                if device_name.contains(name) {
                    return Ok(device);
                }
            }
        }
        
        Err(anyhow::anyhow!("Device '{}' not found", name))
    }
    
    fn find_working_input_device(host: &Host) -> Result<Device> {
        let devices: Vec<Device> = host.input_devices()
            .map_err(|e| anyhow::anyhow!("Failed to enumerate input devices: {}", e))?
            .collect();
        
        // Preferred device names in order of preference
        let preferred_devices = ["pulse", "pipewire", "jack", "plughw:CARD=Microphone"];
        
        // First, try preferred devices
        for preferred in &preferred_devices {
            for device in &devices {
                if let Ok(device_name) = device.name() {
                    if device_name.to_lowercase().contains(&preferred.to_lowercase()) {
                        if device.default_input_config().is_ok() {
                            info!("Found preferred working device: {}", device_name);
                            return Ok(device.clone());
                        }
                    }
                }
            }
        }
        
        // If no preferred devices work, try any device that has a working config
        for device in &devices {
            if device.default_input_config().is_ok() {
                let device_name = device.name().unwrap_or("Unknown".to_string());
                info!("Found alternative working device: {}", device_name);
                return Ok(device.clone());
            }
        }
        
        Err(anyhow::anyhow!("No working input devices found"))
    }
    
    fn is_config_supported(device: &Device, config: &StreamConfig) -> bool {
        device.supported_input_configs()
            .map(|mut configs| {
                configs.any(|c| {
                    c.channels() == config.channels &&
                    c.min_sample_rate() <= config.sample_rate &&
                    c.max_sample_rate() >= config.sample_rate
                })
            })
            .unwrap_or(false)
    }

    /// List all available input devices
    pub fn list_devices() -> Result<Vec<String>> {
        let host = cpal::default_host();
        let devices = host.input_devices()
            .map_err(|e| anyhow::anyhow!("Failed to enumerate devices: {}", e))?;
        
        let mut device_names = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                device_names.push(name);
            }
        }
        
        Ok(device_names)
    }
}
