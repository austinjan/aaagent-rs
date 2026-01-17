//! Output handler for large tool results

use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Default threshold for large output (4KB)
pub const DEFAULT_OUTPUT_THRESHOLD: usize = 4096;

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

    // Generate small preview (first 1KB max)
    let preview_size = 1024.min(size);
    let preview = &output[..preview_size];

    // Count lines for context
    let total_lines = output.lines().count();
    let preview_lines = preview.lines().count();

    // Calculate remaining data
    let remaining = size.saturating_sub(preview_size);

    // Return message for LLM with read tool instructions
    Ok(format!(
        "===== OUTPUT TOO LARGE =====\n\
        Total Size: {} bytes ({} lines)\n\
        Threshold: {} bytes\n\
        Remaining: {} bytes\n\n\
        Preview (first {} bytes, {} lines):\n\
        {}\n\
        [...{} more bytes truncated...]\n\n\
        ===== FILE SAVED =====\n\
        Full output saved to: {}\n\n\
        ===== HOW TO READ THIS FILE =====\n\
        Use the 'read' tool to access this file:\n\
        \n\
        1. First chunk (~3KB):    read(path=\"{}\")\n\
        2. Last chunk:            read(path=\"{}\", mode=\"tail\")\n\
        3. From offset:           read(path=\"{}\", mode=\"offset\", offset=3200)\n\
        4. Search for pattern:    read(path=\"{}\", mode=\"search\", pattern=\"your_keyword\")\n\
        5. First 100 lines:       read(path=\"{}\", mode=\"head\", offset=100)\n\
        \n\
        The 'read' tool automatically chunks large files (max ~3KB per response) and provides navigation hints.",
        size, total_lines,
        threshold,
        remaining,
        preview_size, preview_lines,
        preview,
        remaining,
        file_path.display(),
        file_path.display(),
        file_path.display(),
        file_path.display(),
        file_path.display(),
        file_path.display()
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
        assert!(result.contains("===== OUTPUT TOO LARGE ====="));
        assert!(result.contains("Total Size: 200 bytes"));
        assert!(result.contains("data/temp"));
        assert!(result.contains("===== HOW TO READ THIS FILE ====="));
        assert!(result.contains("Use the 'read' tool"));
        assert!(result.contains("read(path="));
    }
}
