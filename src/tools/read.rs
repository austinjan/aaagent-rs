//! Large file reading tool with chunking support
//!
//! Provides intelligent file reading for outputs that exceed size thresholds.

use super::{BoxFuture, ToolProvider};
use crate::llm::ToolCall;
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

// CRITICAL: Must be SMALLER than DEFAULT_OUTPUT_THRESHOLD (4096)
// Account for metadata overhead: headers, navigation hints, file info (~300-500 bytes)
// Formula: MAX_CHUNK_SIZE + overhead < 4096
// Safe calculation: 3200 bytes content + ~400 bytes metadata = ~3600 bytes total (< 4096)
// Note: format_output() has auto-truncation safety check if this is too large
const MAX_CHUNK_SIZE: usize = 3200; // Content size limit (with auto-truncation fallback)

#[derive(Clone)]
pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }

    /// Read first chunk of file
    fn read_chunk(&self, path: &str, chunk_size: usize) -> Result<String, String> {
        let path = PathBuf::from(path);
        let metadata =
            fs::metadata(&path).map_err(|e| format!("Failed to read file metadata: {}", e))?;

        let total_size = metadata.len() as usize;
        let chunk_size = chunk_size.min(MAX_CHUNK_SIZE).min(total_size);

        let mut file = fs::File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;

        let mut buffer = vec![0u8; chunk_size];
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        buffer.truncate(bytes_read);

        let content = String::from_utf8_lossy(&buffer).to_string();
        let remaining = total_size.saturating_sub(bytes_read);

        Ok(self.format_output(
            path.display().to_string(),
            total_size,
            0,
            bytes_read,
            remaining,
            content,
        ))
    }

    /// Read from head (first N bytes or lines)
    fn read_head(&self, path: &str, offset: Option<usize>) -> Result<String, String> {
        let lines = offset.unwrap_or(100); // Default 100 lines
        let path = PathBuf::from(path);
        let metadata =
            fs::metadata(&path).map_err(|e| format!("Failed to read file metadata: {}", e))?;
        let total_size = metadata.len() as usize;

        let file = fs::File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let reader = BufReader::new(file);

        let mut content = String::new();
        let mut bytes_read = 0;

        for (i, line) in reader.lines().enumerate() {
            if i >= lines {
                break;
            }
            if let Ok(line) = line {
                let line_with_newline = format!("{}\n", line);
                if bytes_read + line_with_newline.len() > MAX_CHUNK_SIZE {
                    break;
                }
                content.push_str(&line_with_newline);
                bytes_read += line_with_newline.len();
            }
        }

        let remaining = total_size.saturating_sub(bytes_read);

        Ok(self.format_output(
            path.display().to_string(),
            total_size,
            0,
            bytes_read,
            remaining,
            content,
        ))
    }

    /// Read from tail (last N bytes or lines)
    fn read_tail(&self, path: &str, offset: Option<usize>) -> Result<String, String> {
        let lines = offset.unwrap_or(100);
        let path = PathBuf::from(path);
        let metadata =
            fs::metadata(&path).map_err(|e| format!("Failed to read file metadata: {}", e))?;
        let total_size = metadata.len() as usize;

        let file = fs::File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let reader = BufReader::new(file);

        let all_lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

        let start_idx = all_lines.len().saturating_sub(lines);
        let tail_lines: Vec<&String> = all_lines.iter().skip(start_idx).collect();

        let mut content = String::new();
        let mut bytes_read = 0;

        for line in tail_lines {
            let line_with_newline = format!("{}\n", line);
            if bytes_read + line_with_newline.len() > MAX_CHUNK_SIZE {
                break;
            }
            content.push_str(&line_with_newline);
            bytes_read += line_with_newline.len();
        }

        let position = total_size.saturating_sub(bytes_read);
        let remaining = 0; // Reading from end, no remaining

        Ok(self.format_output(
            path.display().to_string(),
            total_size,
            position,
            bytes_read,
            remaining,
            content,
        ))
    }

    /// Read from offset (byte position)
    fn read_offset(&self, path: &str, offset: usize, chunk_size: usize) -> Result<String, String> {
        let path = PathBuf::from(path);
        let metadata =
            fs::metadata(&path).map_err(|e| format!("Failed to read file metadata: {}", e))?;
        let total_size = metadata.len() as usize;

        let mut file = fs::File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;

        file.seek(SeekFrom::Start(offset as u64))
            .map_err(|e| format!("Failed to seek: {}", e))?;

        let chunk_size = chunk_size.min(MAX_CHUNK_SIZE);
        let mut buffer = vec![0u8; chunk_size];
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read: {}", e))?;
        buffer.truncate(bytes_read);

        let content = String::from_utf8_lossy(&buffer).to_string();
        let remaining = total_size.saturating_sub(offset + bytes_read);

        Ok(self.format_output(
            path.display().to_string(),
            total_size,
            offset,
            bytes_read,
            remaining,
            content,
        ))
    }

    /// Search within file (grep-like) with pagination support
    fn read_search(
        &self,
        path: &str,
        pattern: &str,
        context: usize,
        skip: usize,
    ) -> Result<String, String> {
        let path = PathBuf::from(path);
        let metadata =
            fs::metadata(&path).map_err(|e| format!("Failed to read file metadata: {}", e))?;
        let total_size = metadata.len() as usize;

        let file = fs::File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let reader = BufReader::new(file);

        let regex =
            regex::Regex::new(pattern).map_err(|e| format!("Invalid regex pattern: {}", e))?;

        let all_lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

        // First pass: collect all matching line indices
        let matching_indices: Vec<usize> = all_lines
            .iter()
            .enumerate()
            .filter(|(_, line)| regex.is_match(line))
            .map(|(i, _)| i)
            .collect();

        let total_matches = matching_indices.len();

        if total_matches == 0 {
            return Ok(format!(
                "===== READ TOOL (SEARCH) =====\n\
                File: {}\n\
                Total Size: {} bytes\n\
                Pattern: {:?}\n\
                \n\
                No matches found.",
                path.display(),
                total_size,
                pattern
            ));
        }

        // Apply skip offset for pagination
        if skip >= total_matches {
            return Ok(format!(
                "===== READ TOOL (SEARCH) =====\n\
                File: {}\n\
                Total Size: {} bytes\n\
                Pattern: {:?}\n\
                Total Matches: {}\n\
                \n\
                Error: skip={} exceeds total matches ({})\n\
                \n\
                ===== NAVIGATION =====\n\
                First page: read(path=\"{}\", mode=\"search\", pattern=\"{}\", skip=0)",
                path.display(),
                total_size,
                pattern,
                total_matches,
                skip,
                total_matches,
                path.display(),
                pattern
            ));
        }

        let matches_to_show = &matching_indices[skip..];
        let mut result_blocks = Vec::new();
        let mut total_result_bytes = 0;
        let mut matches_shown = 0;

        // Build match blocks until we hit size limit
        for &match_idx in matches_to_show {
            let start = match_idx.saturating_sub(context);
            let end = (match_idx + context + 1).min(all_lines.len());

            let match_block: Vec<String> = all_lines[start..end]
                .iter()
                .enumerate()
                .map(|(idx, l)| {
                    let line_num = start + idx + 1;
                    if start + idx == match_idx {
                        format!("{}:> {}", line_num, l) // Mark matched line
                    } else {
                        format!("{}:  {}", line_num, l)
                    }
                })
                .collect();

            let block_str = match_block.join("\n");
            let block_bytes = block_str.len() + 2; // +2 for separator

            if total_result_bytes + block_bytes > MAX_CHUNK_SIZE {
                break; // Stop before exceeding size limit
            }

            result_blocks.push(block_str);
            result_blocks.push("---".to_string());
            total_result_bytes += block_bytes;
            matches_shown += 1;
        }

        let content = result_blocks.join("\n");
        let current_start = skip + 1; // 1-indexed for display
        let current_end = skip + matches_shown;
        let remaining_matches = total_matches - current_end;

        // Build navigation hints
        let nav_hints = if remaining_matches > 0 {
            format!(
                "===== NAVIGATION =====\n\
                Next page: read(path=\"{}\", mode=\"search\", pattern=\"{}\", skip={})\n\
                First page: read(path=\"{}\", mode=\"search\", pattern=\"{}\", skip=0)",
                path.display(),
                pattern,
                current_end,
                path.display(),
                pattern
            )
        } else if skip > 0 {
            format!(
                "===== NAVIGATION =====\n\
                First page: read(path=\"{}\", mode=\"search\", pattern=\"{}\", skip=0)\n\
                All matches shown",
                path.display(),
                pattern
            )
        } else {
            "===== NAVIGATION =====\nAll matches shown in single page".to_string()
        };

        Ok(format!(
            "===== READ TOOL (SEARCH) =====\n\
            File: {}\n\
            Total Size: {} bytes\n\
            Pattern: {:?}\n\
            Total Matches: {}\n\
            Current: {}-{} (showing {} matches)\n\
            Remaining: {} matches\n\
            \n\
            {}\n\
            \n\
            {}",
            path.display(),
            total_size,
            pattern,
            total_matches,
            current_start,
            current_end,
            matches_shown,
            remaining_matches,
            content,
            nav_hints
        ))
    }

    fn format_output(
        &self,
        path: String,
        total_size: usize,
        position: usize,
        bytes_read: usize,
        remaining: usize,
        content: String,
    ) -> String {
        let chunk_num = (position / MAX_CHUNK_SIZE) + 1;
        let total_chunks = (total_size + MAX_CHUNK_SIZE - 1) / MAX_CHUNK_SIZE;

        // Keep navigation hints concise to minimize overhead
        let nav_hints = if remaining > 0 {
            format!(
                "===== NAVIGATION =====\n\
                Remaining: {} bytes\n\
                Next: read(path=\"{}\", mode=\"offset\", offset={})\n\
                Last: read(path=\"{}\", mode=\"tail\")\n\
                Search: read(path=\"{}\", mode=\"search\", pattern=\"...\")",
                remaining,
                path,
                position + bytes_read,
                path,
                path
            )
        } else {
            "===== END OF FILE =====".to_string()
        };

        let output = format!(
            "===== READ TOOL OUTPUT =====\n\
            File: {}\n\
            Total: {} bytes | Position: {}-{} | Chunk {}/{} | Remaining: {}\n\n\
            {}\n\n\
            {}",
            path,
            total_size,
            position,
            position + bytes_read,
            chunk_num,
            total_chunks,
            remaining,
            content,
            nav_hints
        );

        // CRITICAL SAFETY CHECK: Ensure total output is under threshold
        // If output exceeds threshold (metadata overhead was larger than expected),
        // gracefully truncate content instead of panicking
        let output_size = output.len();
        if output_size >= crate::tools::DEFAULT_OUTPUT_THRESHOLD {
            // Calculate how much we need to trim
            let metadata_overhead = output_size - bytes_read;
            let safe_content_size =
                crate::tools::DEFAULT_OUTPUT_THRESHOLD - metadata_overhead - 200; // 200 byte safety margin

            // Truncate content to safe size
            let truncated_content = if content.len() > safe_content_size {
                format!(
                    "{}...[truncated {} bytes to fit]",
                    &content[..safe_content_size],
                    content.len() - safe_content_size
                )
            } else {
                content
            };

            // Rebuild output with truncated content
            let nav_hints = if remaining > 0 {
                format!(
                    "===== NAVIGATION =====\n\
                    Remaining: {} bytes\n\
                    Next: read(path=\"{}\", mode=\"offset\", offset={})",
                    remaining,
                    path,
                    position + bytes_read
                )
            } else {
                "===== END OF FILE =====".to_string()
            };

            format!(
                "===== READ TOOL OUTPUT (AUTO-TRUNCATED) =====\n\
                File: {}\n\
                Total: {} bytes | Position: {}-{} | Remaining: {}\n\
                Note: Content was truncated to stay under 4KB limit\n\n\
                {}\n\n\
                {}",
                path,
                total_size,
                position,
                position + bytes_read,
                remaining,
                truncated_content,
                nav_hints
            )
        } else {
            output
        }
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolProvider for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn brief(&self) -> &str {
        "Read large files with chunking, head/tail, offset, and search support."
    }

    fn full_description(&self) -> String {
        r#"Read large files intelligently with multiple access modes and pagination support.

MODES:
- read: Return first ~3.2KB chunk with navigation hints
- head: Read first N lines (default 100, max ~3.2KB)
- tail: Read last N lines (default 100, max ~3.2KB)
- offset: Read from byte position with specified chunk size
- search: Grep-like pattern search with context lines (SUPPORTS PAGINATION)

PARAMETERS:
- path: File path (required) - typically data/temp/ files from tool outputs
- mode: Access mode (default: "read")
- chunk_size: Bytes to read (max 3200)
- offset: Byte position or line count depending on mode
- pattern: Regex pattern for search mode
- context_lines: Lines of context around matches (default: 2)
- skip: Number of matches to skip for pagination (search mode only, default: 0)

PAGINATION IN SEARCH MODE:
The search mode supports pagination via the 'skip' parameter:
- skip=0: First page of matches (default)
- skip=N: Skip first N matches, show remaining matches
- Each response shows "Current: X-Y" and "Remaining: Z matches"
- Navigation hints provided: "Next page: read(..., skip=Y)"

CHUNK SIZE LIMIT: 3200 bytes content maximum per response
IMPORTANT: Content limit (3200) + metadata overhead (~300-400 bytes) = total < 4096 bytes
This prevents read tool responses from triggering handle_large_output() recursively.

USE CASES:
✓ Read tool outputs that exceeded size threshold
✓ Navigate large log files
✓ Search within large outputs with pagination
✓ View beginning/end of files without loading everything

EXAMPLES:
read(path="data/temp/shell_123.txt") → ~3KB content + metadata (total <4KB)
read(path="data/temp/shell_123.txt", mode="tail") → Last ~3KB content
read(path="app.log", mode="search", pattern="ERROR") → First page of ERROR matches
read(path="app.log", mode="search", pattern="ERROR", skip=10) → Skip first 10 matches, show next page
read(path="data/temp/shell_123.txt", mode="offset", offset=3200) → Next ~3KB chunk
"#
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path to read"
                },
                "mode": {
                    "type": "string",
                    "enum": ["read", "head", "tail", "offset", "search"],
                    "description": "Reading mode",
                    "default": "read"
                },
                "chunk_size": {
                    "type": "integer",
                    "default": 3200,
                    "maximum": 3200,
                    "description": "Content bytes (max 3200 - auto-truncates if total >4KB)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Byte offset for 'offset' mode or line count for head/tail"
                },
                "pattern": {
                    "type": "string",
                    "description": "Search pattern for 'search' mode (regex)"
                },
                "context_lines": {
                    "type": "integer",
                    "default": 2,
                    "description": "Context lines around matches in search mode"
                },
                "skip": {
                    "type": "integer",
                    "default": 0,
                    "description": "Number of matches to skip (for pagination in search mode)"
                }
            },
            "required": ["path"]
        })
    }

    fn execute<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let path = call
                .arguments
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing 'path' parameter".to_string())?;

            let mode = call
                .arguments
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("read");

            let chunk_size = call
                .arguments
                .get("chunk_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(3200) as usize;

            let offset = call
                .arguments
                .get("offset")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            let pattern = call
                .arguments
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let context = call
                .arguments
                .get("context_lines")
                .and_then(|v| v.as_u64())
                .unwrap_or(2) as usize;

            let skip = call
                .arguments
                .get("skip")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            // Validate chunk_size doesn't exceed MAX_CHUNK_SIZE
            let chunk_size = chunk_size.min(MAX_CHUNK_SIZE);

            match mode {
                "read" => self.read_chunk(path, chunk_size),
                "head" => self.read_head(path, offset),
                "tail" => self.read_tail(path, offset),
                "offset" => {
                    let offset = offset
                        .ok_or_else(|| "Missing 'offset' parameter for offset mode".to_string())?;
                    self.read_offset(path, offset, chunk_size)
                }
                "search" => {
                    if pattern.is_empty() {
                        return Err("Missing 'pattern' parameter for search mode".to_string());
                    }
                    self.read_search(path, pattern, context, skip)
                }
                _ => Err(format!("Invalid mode: {}", mode)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn setup_test_file(content: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let temp_dir = std::env::temp_dir();
        let count = COUNTER.fetch_add(1, Ordering::SeqCst);
        let file_path = temp_dir.join(format!("read_tool_test_{}.txt", count));
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file_path.display().to_string()
    }

    #[test]
    fn test_read_chunk_small_file() {
        let content = "Hello, world!";
        let path = setup_test_file(content);

        let tool = ReadTool::new();
        let result = tool.read_chunk(&path, 3200).unwrap();

        assert!(result.contains("Hello, world!"));
        assert!(result.contains("Total: 13 bytes"));
        assert!(result.contains("Remaining: 0"));
        assert!(result.contains("END OF FILE"));
    }

    #[test]
    fn test_read_chunk_large_file() {
        let content = "X".repeat(10000);
        let path = setup_test_file(&content);

        let tool = ReadTool::new();
        let result = tool.read_chunk(&path, 3200).unwrap();

        assert!(result.contains("Total: 10000 bytes"));
        assert!(result.contains("Remaining:")); // Has remaining bytes
        assert!(result.contains("NAVIGATION"));
        assert!(result.contains("Next:"));
    }

    #[test]
    fn test_read_head() {
        let content = (1..=200)
            .map(|i| format!("Line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let path = setup_test_file(&content);

        let tool = ReadTool::new();
        let result = tool.read_head(&path, Some(10)).unwrap();

        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 10"));
        assert!(!result.contains("Line 11")); // Should stop at 10 lines
    }

    #[test]
    fn test_read_tail() {
        let content = (1..=200)
            .map(|i| format!("Line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let path = setup_test_file(&content);

        let tool = ReadTool::new();
        let result = tool.read_tail(&path, Some(10)).unwrap();

        assert!(result.contains("Line 191"));
        assert!(result.contains("Line 200"));
        assert!(!result.contains("Line 190"));
    }

    #[test]
    fn test_read_offset() {
        let content = "X".repeat(10000);
        let path = setup_test_file(&content);

        let tool = ReadTool::new();
        let result = tool.read_offset(&path, 3200, 3200).unwrap();

        assert!(result.contains("Position: 3200-"));
        assert!(result.contains("Remaining:")); // Has remaining data
    }

    #[test]
    fn test_read_search() {
        let content =
            "Line 1: INFO\nLine 2: ERROR\nLine 3: INFO\nLine 4: ERROR found\nLine 5: DEBUG";
        let path = setup_test_file(content);

        let tool = ReadTool::new();
        let result = tool.read_search(&path, "ERROR", 1).unwrap();

        assert!(result.contains("SEARCH"));
        assert!(result.contains(":>")); // Matched line marker
        assert!(result.contains("ERROR")); // Pattern in results
        assert!(result.contains("Pattern: \"ERROR\""));
    }
}
