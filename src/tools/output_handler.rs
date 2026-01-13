//! Output handler for large tool results

use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Default threshold for large output (2KB)
pub const DEFAULT_OUTPUT_THRESHOLD: usize = 2048;

/// Process tool output and write to temp file if too large
///
/// # Arguments
///
/// * `output` - The tool output string
/// * `tool_name` - Name of the tool (for filename)
/// * `threshold` - Optional size threshold (default: 2KB)
///
/// # Returns
///
/// Returns the content to send to LLM:
/// - If output <= threshold: returns original output
/// - If output > threshold: writes to file and returns message with file path
pub fn handle_large_output(
    output: &str,
    tool_name: &str,
    threshold: Option<usize>,
) -> Result<String, std::io::Error> {
    let threshold = threshold.unwrap_or(DEFAULT_OUTPUT_THRESHOLD);
    let size = output.len();

    // If small enough, return as-is
    if size <= threshold {
        return Ok(output.to_string());
    }

    // Create temp directory
    let temp_dir = PathBuf::from("data/temp");
    fs::create_dir_all(&temp_dir)?;

    // Generate filename with timestamp
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.txt", tool_name, timestamp);
    let file_path = temp_dir.join(filename);

    // Write to file
    let mut file = fs::File::create(&file_path)?;
    file.write_all(output.as_bytes())?;

    // Return message for LLM
    Ok(format!(
        "Output too large ({} bytes > {} threshold). Full output written to: {}\nYou can read this file to see the complete output.",
        size, threshold, file_path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_output() {
        let output = "small output";
        let result = handle_large_output(output, "test", Some(100)).unwrap();
        assert_eq!(result, output);
    }

    #[test]
    fn test_large_output() {
        let output = "X".repeat(200);
        let result = handle_large_output(&output, "test", Some(50)).unwrap();
        assert!(result.contains("Output too large"));
        assert!(result.contains("200 bytes"));
        assert!(result.contains("data/temp"));
    }
}
