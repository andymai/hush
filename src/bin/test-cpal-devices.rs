use cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    println!("Testing CPAL audio devices:");
    
    let host = cpal::default_host();
    println!("Host: {}", host.id().name());
    
    println!("\nInput devices:");
    match host.input_devices() {
        Ok(devices) => {
            for (i, device) in devices.enumerate() {
                match device.name() {
                    Ok(name) => println!("  {}: {}", i, name),
                    Err(e) => println!("  {}: Failed to get name: {}", i, e),
                }
                
                match device.default_input_config() {
                    Ok(config) => println!("      Config: {:?}", config),
                    Err(e) => println!("      Config error: {}", e),
                }
            }
        }
        Err(e) => println!("Failed to enumerate input devices: {}", e),
    }
    
    println!("\nDefault input device:");
    match host.default_input_device() {
        Some(device) => {
            match device.name() {
                Ok(name) => println!("  Device: {}", name),
                Err(e) => println!("  Failed to get name: {}", e),
            }
            
            match device.default_input_config() {
                Ok(config) => println!("  Config: {:?}", config),
                Err(e) => println!("  Config error: {}", e),
            }
        }
        None => println!("  No default input device found"),
    }
}