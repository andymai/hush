use crate::Result;
use cpal::{Device, Host, Stream, StreamConfig, SampleRate};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{info, warn, error};

pub struct AudioCapture {
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<Mutex<bool>>,
    simulated_mode: bool,
}

impl AudioCapture {
    pub fn new(device_name: Option<&str>) -> Result<Self> {
        info!("Initializing audio capture system");
        
        // Get the default host (will use PipeWire/JACK on Linux)
        let host = cpal::default_host();
        info!("Audio host: {}", host.id().name());
        
        // Select audio device
        let device = match device_name {
            Some(name) => {
                info!("Looking for specific device: {}", name);
                Self::find_device_by_name(&host, name)?
            },
            None => {
                info!("Using default input device");
                match host.default_input_device() {
                    Some(device) => device,
                    None => {
                        warn!("No default input device available, trying alternatives");
                        Self::find_working_input_device(&host)?
                    }
                }
            }
        };
        
        info!("Selected audio device: {}", device.name().unwrap_or("Unknown".to_string()));
        
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
        })
    }
    
    pub fn start_recording(&mut self) -> Result<()> {
        info!("Starting audio recording (simulated: {})", self.simulated_mode);
        
        if self.stream.is_some() {
            warn!("Recording already in progress");
            return Ok(());
        }
        
        // Clear the buffer
        self.buffer.lock().clear();
        *self.is_recording.lock() = true;
        
        // Handle simulated mode for testing
        if self.simulated_mode {
            info!("Using simulated audio recording for testing");
            return Ok(());
        }
        
        let buffer_clone = Arc::clone(&self.buffer);
        let is_recording_clone = Arc::clone(&self.is_recording);
        
        // Build the input stream
        let stream = self.device.build_input_stream(
            &self.config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if *is_recording_clone.lock() {
                    buffer_clone.lock().extend_from_slice(data);
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
        info!("Stopping audio recording (simulated: {})", self.simulated_mode);
        
        *self.is_recording.lock() = false;
        
        // Handle simulated mode with fake audio data
        if self.simulated_mode {
            // Generate 3 seconds of simulated audio data (silence for now)
            let duration_samples = (3.0 * self.config.sample_rate.0 as f32) as usize;
            let simulated_data = vec![0.0f32; duration_samples];
            info!("Generated {} samples of simulated audio", simulated_data.len());
            return Ok(simulated_data);
        }
        
        if let Some(stream) = self.stream.take() {
            stream.pause()
                .map_err(|e| anyhow::anyhow!("Failed to stop audio stream: {}", e))?;
        }
        
        let recorded_data = {
            let mut buffer = self.buffer.lock();
            let data = buffer.clone();
            buffer.clear();
            data
        };
        
        info!("Recording stopped. Captured {} samples ({:.2}s)", 
              recorded_data.len(), 
              recorded_data.len() as f32 / self.config.sample_rate.0 as f32);
        
        Ok(recorded_data)
    }
    
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock()
    }
    
    pub fn get_device_name(&self) -> String {
        self.device.name().unwrap_or("Unknown Device".to_string())
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
