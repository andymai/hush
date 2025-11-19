/// Manual recording command implementation
///
/// Handles manual recording mode where the user can perform multiple recordings in sequence.
use anyhow::Result;
use tracing::info;

use super::handle_record;

/// Handle the manual recording command
///
/// Performs a specified number of sequential recordings, each with a fixed duration.
/// This is useful for batch recording sessions where multiple voice inputs are needed.
///
/// # Arguments
///
/// * `count` - Number of recordings to perform sequentially
///
/// # Examples
///
/// ```no_run
/// // Perform 3 sequential recordings
/// handle_manual(3).await?;
///
/// // Perform single manual recording
/// handle_manual(1).await?;
/// ```
///
/// # Workflow
///
/// 1. For each recording (1 to count):
///    - Print progress indicator (if count > 1)
///    - Call handle_record with 30-second duration
///    - Transcribe and insert text automatically
/// 2. Print completion message
///
/// # Note
///
/// Each recording uses a fixed 30-second maximum duration and automatically
/// transcribes and inserts the text (not print-only mode).
pub async fn handle_manual(count: u32) -> Result<()> {
    info!("🎯 Starting manual recording mode ({} recordings)", count);

    for i in 1..=count {
        if count > 1 {
            println!("\n📹 Recording {}/{}", i, count);
        }

        // Use the record handler for each iteration
        // Fixed parameters: 30s duration, not print-only, no audio save
        handle_record(30, false, None).await?;
    }

    println!("✅ All recordings completed!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration test would require mocking the recording pipeline
    // Unit testing the loop logic is straightforward, but the actual recording
    // requires hardware (microphone, transcription model, text insertion)
}
