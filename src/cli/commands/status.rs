use super::utils::{show_config_status, show_device_status, show_system_info};
/// Status command implementation
///
/// Displays system status including configuration, devices, and component health.
use anyhow::Result;

/// Handle the status command
///
/// # Arguments
///
/// * `_config` - Reserved for future use to show only configuration details
/// * `_devices` - Reserved for future use to show only device details
/// * `full` - Follow with the `hush doctor` report
///
/// # Note
///
/// Currently, config and device status are always shown. The `_config` and `_devices`
/// parameters are reserved for future enhancement to allow selective display.
///
/// # Examples
///
/// ```no_run
/// # use hush::cli::commands::handle_status;
/// # async fn example() -> anyhow::Result<()> {
/// // Basic status
/// handle_status(false, false, false).await?;
///
/// // Followed by the doctor report
/// handle_status(false, false, true).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_status(_config: bool, _devices: bool, full: bool) -> Result<()> {
    println!("🤫 Hush System Status");
    println!();

    print!("Daemon: ");
    match crate::ipc::client::status().await {
        Some(state) => println!("✅ running ({})", state),
        None => println!("not running (start with `hush daemon start`)"),
    }
    println!();

    // Always show basic status
    show_system_info().await?;
    println!();
    show_config_status().await?;
    println!();
    show_device_status().await?;

    if full {
        println!();
        return super::doctor::handle_doctor().await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_status_command_runs() {
        // This is a basic smoke test - the command should not panic
        // Actual functionality depends on system configuration
        let result = handle_status(false, false, false).await;

        // We don't assert success since it depends on system setup
        // but we verify it doesn't panic
        let _ = result;
    }
}
