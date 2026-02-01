//! File editing tool with multiple operation modes
//!
//! Provides intuitive search-and-replace operations designed for LLM usage.
//! Supports: replace, insert_before, insert_after, delete, append, prepend.

use super::{BoxFuture, ToolProvider};
use crate::llm::ToolCall;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;

/// File editing tool supporting multiple operation modes
#[derive(Clone)]
pub struct EditorEditTool;

impl EditorEditTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EditorEditTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Request format for basic edits mode (simple replace)
#[derive(Debug, Deserialize)]
struct BasicEditRequest {
    file_path: String,
    edits: Vec<BasicEdit>,
}

#[derive(Debug, Deserialize)]
struct BasicEdit {
    old_text: String,
    new_text: String,
    #[serde(default)]
    replace_all: bool,
}

/// Request format for extended operation modes
#[derive(Debug, Deserialize)]
struct ExtendedEditRequest {
    file_path: String,
    operation: Operation,
    #[serde(default)]
    anchor: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Replace,
    InsertBefore,
    InsertAfter,
    Delete,
    Append,
    Prepend,
}

impl ToolProvider for EditorEditTool {
    fn name(&self) -> &str {
        "editor__Edit"
    }

    fn brief(&self) -> &str {
        "Edit text files, insert text, delete text, replace text. "
    }

    fn full_description(&self) -> String {
        r#"
# Editor Edit
Edit files using intuitive search-and-replace operations.
Designed for LLM usage - no regex, no escape sequences, just literal text matching.

## Key Rules
- `old_text` must match EXACTLY (whitespace, indentation matter)
- `old_text` must appear exactly ONCE in the file (for safety), unless `replace_all` is set to true
- Include enough context lines to ensure uniqueness
- No regex - literal text matching only

## How to Use

1. Multiple Replacements:
Use case: Renaming functions, updating strings, fixing typos across a file.

```json
{
  "file_path": "/path/to/main.rs",
  "edits": [
    {
      "old_text": "fn old_name()",
      "new_text": "fn new_name()"
    },
    {
      "old_text": "println!(\"test\")",
      "new_text": "println!(\"updated\")",
    }
  ]
}
```

2. Replace All Occurrences
Use case: Renaming variables that appear multiple times. batch replacements.
```json
{
  "file_path": "/path/to/main.rs",
  "edits": [
    {
      "old_text": "old_var",
      "new_text": "new_var",
      "replace_all": true
    }
  ]
}
```
3. Insert After
Use case: Adding imports, inserting new text after anchor.
```json
{
    "file_path": "/path/to/file",
    "operation": "insert_after",
    "anchor": "use std::io;",
    "content": "use std::fs;"
}
```
4. Insert Before
Use case: Adding documentation, inserting headers before functions.
```json
{
    "file_path": "/path/to/file",
    "operation": "insert_before",
    "anchor": "use std::io;",
    "content": "use std::fs;"
}
```
5. Delete
Use case: Removing unnecessary code, deleting lines.
```json
{
  "file_path": "/src/main.rs",
  "operation": "delete",
  "anchor": "    // TODO: remove this\n"
}
```
6. Append
Use case: Adding new content at the end of the file.
```json
{
  "file_path": "/src/main.rs",
  "operation": "append",
  "content": "\nfn new_function() {\n    // implementation\n}\n"
}
```
7. Prepend
Use case: Adding new content at the beginning of the file.
```json
{
  "file_path": "/src/main.rs",
  "operation": "prepend",
  "content": "\nfn new_function() {\n    // implementation\n}\n"
}
```
## Whitespace Preservation
The tool uses exact string matching, so whitespace matters:
Won't Match:
```
{"old_text": "fn test() {"}  // Missing indentation
```
File contains:
```
    fn test() {  // Has 4 spaces before
```
Correct:
```
{"old_text": "    fn test() {"}  // Includes indentation
```
"#
        .to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "description": "IMPORTANT: This tool has two mutually exclusive modes:\n\
                            1. Basic mode: Provide 'edits' array for multiple search-and-replace operations\n\
                            2. Extended mode: Provide 'operation' field for single insert/delete/append/prepend\n\
                            You MUST use exactly one mode per call.",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit"
                },
                "edits": {
                    "type": "array",
                    "description": "List of edit operations (BASIC MODE ONLY - do not combine with 'operation')",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_text": {
                                "type": "string",
                                "description": "Exact text to find (must be unique unless replace_all is true)"
                            },
                            "new_text": {
                                "type": "string",
                                "description": "Text to replace with (empty string to delete)"
                            },
                            "replace_all": {
                                "type": "boolean",
                                "description": "If true, replace all occurrences. If false (default), old_text must be unique.",
                                "default": false
                            }
                        },
                        "required": ["old_text", "new_text"]
                    }
                },
                "operation": {
                    "type": "string",
                    "enum": ["replace", "insert_before", "insert_after", "delete", "append", "prepend"],
                    "description": "Operation type (EXTENDED MODE ONLY - do not combine with 'edits')"
                },
                "anchor": {
                    "type": "string",
                    "description": "Text to locate (for replace/insert/delete operations in extended mode)"
                },
                "content": {
                    "type": "string",
                    "description": "New content (for replace/insert/append/prepend operations in extended mode)"
                }
            },
            "required": ["file_path"]
        })
    }

    fn execute<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            // Try to parse as basic mode first
            if let Ok(request) = serde_json::from_value::<BasicEditRequest>(call.arguments.clone())
            {
                return execute_basic_edits(&request).await;
            }

            // Try extended mode
            if let Ok(request) =
                serde_json::from_value::<ExtendedEditRequest>(call.arguments.clone())
            {
                return execute_extended_operation(&request).await;
            }

            Err("Invalid request format. Must provide either 'edits' array (basic mode) or 'operation' field (extended mode).".to_string())
        })
    }
}

/// Create a "fingerprint" by normalizing whitespace (all whitespace sequences -> single space)
fn create_fingerprint(s: &str) -> String {
    let mut result = String::new();
    let mut prev_whitespace = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_whitespace {
                result.push(' ');
            }
            prev_whitespace = true;
        } else {
            result.push(c);
            prev_whitespace = false;
        }
    }
    result.trim().to_string()
}

/// Find the actual segment in the file that matches a fingerprint
/// Returns (start_pos, end_pos, actual_text) if found
fn find_segment_by_fingerprint(search: &str, content: &str) -> Option<(usize, usize, String)> {
    let search_fp = create_fingerprint(search);
    if search_fp.is_empty() {
        return None;
    }

    // We need to find a segment in content whose fingerprint matches search_fp
    // Strategy: Find anchor points (first and last non-whitespace tokens) and extract between them

    // Get first and last "words" from the fingerprint as anchors
    let fp_words: Vec<&str> = search_fp.split_whitespace().collect();
    if fp_words.is_empty() {
        return None;
    }

    let first_word = fp_words[0];
    let last_word = fp_words[fp_words.len() - 1];

    // Find all occurrences of first_word in content
    let mut search_start = 0;
    while let Some(first_pos) = content[search_start..].find(first_word) {
        let first_pos = search_start + first_pos;

        // From this position, find last_word
        if let Some(last_offset) = content[first_pos..].rfind(last_word) {
            let last_pos = first_pos + last_offset + last_word.len();

            // Extract this segment
            let segment = &content[first_pos..last_pos];

            // Check if this segment's fingerprint matches
            if create_fingerprint(segment) == search_fp {
                return Some((first_pos, last_pos, segment.to_string()));
            }
        }

        // Try next occurrence
        search_start = first_pos + 1;
        if search_start >= content.len() {
            break;
        }
    }

    None
}

/// Generate a simple line-by-line diff between two strings
fn generate_diff(expected: &str, actual: &str) -> String {
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();

    let mut diff = String::new();
    let max_lines = expected_lines.len().max(actual_lines.len()).min(20); // Limit output

    for i in 0..max_lines {
        let exp = expected_lines.get(i).copied().unwrap_or("");
        let act = actual_lines.get(i).copied().unwrap_or("");

        if exp == act {
            diff.push_str(&format!("  {}: {}\n", i + 1, escape_whitespace(exp)));
        } else {
            diff.push_str(&format!("- {}: {}\n", i + 1, escape_whitespace(exp)));
            diff.push_str(&format!("+ {}: {}\n", i + 1, escape_whitespace(act)));
        }
    }

    if expected_lines.len() > 20 || actual_lines.len() > 20 {
        diff.push_str(&format!(
            "... ({} more lines)\n",
            expected_lines.len().max(actual_lines.len()) - 20
        ));
    }

    diff
}

/// Make whitespace visible for debugging
fn escape_whitespace(s: &str) -> String {
    s.replace('\t', "→")
        .replace(' ', "·")
}

/// Analyze whitespace differences between search text and file content
/// Returns a diagnostic message if a whitespace mismatch is likely
fn diagnose_whitespace_mismatch(search: &str, content: &str) -> Option<String> {
    // Step 1: Try to find segment by fingerprint
    if let Some((_start, _end, actual_segment)) = find_segment_by_fingerprint(search, content) {
        // Found the text with different whitespace!
        let search_lines = search.lines().count();
        let actual_lines = actual_segment.lines().count();
        let search_blank = count_blank_lines(search);
        let actual_blank = count_blank_lines(&actual_segment);

        let mut msg = String::from("Whitespace mismatch detected!\n\n");

        msg.push_str(&format!(
            "Your old_text: {} lines ({} blank)\n",
            search_lines, search_blank
        ));
        msg.push_str(&format!(
            "File contains: {} lines ({} blank)\n\n",
            actual_lines, actual_blank
        ));

        msg.push_str("Diff (- your text, + file text, ·=space, →=tab):\n");
        msg.push_str(&generate_diff(search, &actual_segment));

        msg.push_str("\nHint: Copy the exact text from the file, preserving all whitespace.\n");

        return Some(msg);
    }

    None
}

/// Count total blank lines in text
fn count_blank_lines(text: &str) -> usize {
    text.lines().filter(|l| l.trim().is_empty()).count()
}

async fn execute_basic_edits(request: &BasicEditRequest) -> Result<String, String> {
    // Read file
    let file_path = Path::new(&request.file_path);
    let mut content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file '{}': {}", request.file_path, e))?;

    // Apply each edit
    let mut edits_applied = 0;
    let mut total_replacements = 0;

    for (idx, edit) in request.edits.iter().enumerate() {
        // Count occurrences
        let count = content.matches(&edit.old_text).count();

        if count == 0 {
            // Try to diagnose the issue
            let diagnosis = diagnose_whitespace_mismatch(&edit.old_text, &content);

            let mut error_msg = format!(
                "Edit #{}: old_text not found in file.\n",
                idx + 1
            );

            if let Some(diag) = diagnosis {
                error_msg.push_str(&format!("\n{}\n", diag));
            }

            error_msg.push_str(&format!("\nSearching for:\n{}\n", edit.old_text));

            return Err(error_msg);
        }

        // Check uniqueness only if replace_all is false
        if !edit.replace_all && count > 1 {
            return Err(format!(
                "Edit #{}: old_text appears {} times (must be unique).\nSearching for:\n{}\n\nInclude more context to make it unique, or set replace_all: true.",
                idx + 1,
                count,
                edit.old_text
            ));
        }

        // Apply replacement
        content = content.replace(&edit.old_text, &edit.new_text);
        edits_applied += 1;
        total_replacements += count;
    }

    // Write back
    fs::write(file_path, &content)
        .map_err(|e| format!("Failed to write file '{}': {}", request.file_path, e))?;

    Ok(format!(
        "Successfully applied {} edit(s) ({} replacement(s)) to '{}'",
        edits_applied, total_replacements, request.file_path
    ))
}

async fn execute_extended_operation(request: &ExtendedEditRequest) -> Result<String, String> {
    let file_path = Path::new(&request.file_path);

    match request.operation {
        Operation::Append => {
            // Append to end of file
            let mut content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read file '{}': {}", request.file_path, e))?;

            content.push_str(&request.content);

            fs::write(file_path, &content)
                .map_err(|e| format!("Failed to write file '{}': {}", request.file_path, e))?;

            Ok(format!(
                "Successfully appended {} bytes to '{}'",
                request.content.len(),
                request.file_path
            ))
        }

        Operation::Prepend => {
            // Prepend to beginning of file
            let content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read file '{}': {}", request.file_path, e))?;

            let new_content = format!("{}{}", request.content, content);

            fs::write(file_path, &new_content)
                .map_err(|e| format!("Failed to write file '{}': {}", request.file_path, e))?;

            Ok(format!(
                "Successfully prepended {} bytes to '{}'",
                request.content.len(),
                request.file_path
            ))
        }

        Operation::Replace
        | Operation::InsertBefore
        | Operation::InsertAfter
        | Operation::Delete => {
            // These operations require an anchor
            if request.anchor.is_empty() {
                return Err(format!(
                    "Operation '{:?}' requires 'anchor' field",
                    request.operation
                ));
            }

            let content = fs::read_to_string(file_path)
                .map_err(|e| format!("Failed to read file '{}': {}", request.file_path, e))?;

            // Check anchor uniqueness
            let count = content.matches(&request.anchor).count();

            if count == 0 {
                // Try to diagnose the issue
                let diagnosis = diagnose_whitespace_mismatch(&request.anchor, &content);

                let mut error_msg = "Anchor not found in file.\n".to_string();

                if let Some(diag) = diagnosis {
                    error_msg.push_str(&format!("\n{}\n", diag));
                }

                error_msg.push_str(&format!("\nSearching for:\n{}\n", request.anchor));

                return Err(error_msg);
            }

            if count > 1 {
                return Err(format!(
                    "Anchor appears {} times (must be unique).\nSearching for:\n{}\n\nInclude more context to make it unique.",
                    count,
                    request.anchor
                ));
            }

            // Apply operation
            let new_content = match request.operation {
                Operation::Replace => content.replace(&request.anchor, &request.content),
                Operation::InsertBefore => content.replace(
                    &request.anchor,
                    &format!("{}{}", request.content, request.anchor),
                ),
                Operation::InsertAfter => content.replace(
                    &request.anchor,
                    &format!("{}{}", request.anchor, request.content),
                ),
                Operation::Delete => content.replace(&request.anchor, ""),
                _ => unreachable!(),
            };

            fs::write(file_path, &new_content)
                .map_err(|e| format!("Failed to write file '{}': {}", request.file_path, e))?;

            Ok(format!(
                "Successfully applied {:?} operation to '{}'",
                request.operation, request.file_path
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_basic_replace() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "Hello, world!\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = BasicEditRequest {
            file_path: path.clone(),
            edits: vec![BasicEdit {
                old_text: "world".to_string(),
                new_text: "Rust".to_string(),
                replace_all: false,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_ok(), "Edit should succeed: {:?}", result);

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "Hello, Rust!\n");
    }

    #[tokio::test]
    async fn test_multiple_edits() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "fn old_name() {{\n    println!(\"test\");\n}}\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = BasicEditRequest {
            file_path: path.clone(),
            edits: vec![
                BasicEdit {
                    old_text: "old_name".to_string(),
                    new_text: "new_name".to_string(),
                    replace_all: false,
                },
                BasicEdit {
                    old_text: "\"test\"".to_string(),
                    new_text: "\"updated\"".to_string(),
                    replace_all: false,
                },
            ],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("new_name"));
        assert!(content.contains("\"updated\""));
    }

    #[tokio::test]
    async fn test_non_unique_anchor_fails() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "test\ntest\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = BasicEditRequest {
            file_path: path,
            edits: vec![BasicEdit {
                old_text: "test".to_string(),
                new_text: "replaced".to_string(),
                replace_all: false,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("appears 2 times"));
    }

    #[tokio::test]
    async fn test_anchor_not_found_fails() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "Hello, world!\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = BasicEditRequest {
            file_path: path,
            edits: vec![BasicEdit {
                old_text: "nonexistent".to_string(),
                new_text: "replaced".to_string(),
                replace_all: false,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_insert_after() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "use std::io;\n\nfn main() {{}}\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = ExtendedEditRequest {
            file_path: path.clone(),
            operation: Operation::InsertAfter,
            anchor: "use std::io;\n".to_string(),
            content: "use std::fs;\n".to_string(),
        };

        let result = execute_extended_operation(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "use std::io;\nuse std::fs;\n\nfn main() {}\n");
    }

    #[tokio::test]
    async fn test_insert_before() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "fn main() {{}}\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = ExtendedEditRequest {
            file_path: path.clone(),
            operation: Operation::InsertBefore,
            anchor: "fn main()".to_string(),
            content: "/// Main function\n".to_string(),
        };

        let result = execute_extended_operation(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("/// Main function\nfn main()"));
    }

    #[tokio::test]
    async fn test_delete() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "line1\nline2\nline3\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = ExtendedEditRequest {
            file_path: path.clone(),
            operation: Operation::Delete,
            anchor: "line2\n".to_string(),
            content: String::new(),
        };

        let result = execute_extended_operation(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "line1\nline3\n");
    }

    #[tokio::test]
    async fn test_append() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "existing content\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = ExtendedEditRequest {
            file_path: path.clone(),
            operation: Operation::Append,
            anchor: String::new(),
            content: "appended content\n".to_string(),
        };

        let result = execute_extended_operation(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "existing content\nappended content\n");
    }

    #[tokio::test]
    async fn test_prepend() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "existing content\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = ExtendedEditRequest {
            file_path: path.clone(),
            operation: Operation::Prepend,
            anchor: String::new(),
            content: "// Header\n".to_string(),
        };

        let result = execute_extended_operation(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "// Header\nexisting content\n");
    }

    #[tokio::test]
    async fn test_whitespace_preservation() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "    indented line\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = BasicEditRequest {
            file_path: path.clone(),
            edits: vec![BasicEdit {
                old_text: "    indented line".to_string(),
                new_text: "    still indented".to_string(),
                replace_all: false,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "    still indented\n");
    }

    #[tokio::test]
    async fn test_replace_all_multiple_occurrences() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "test test test\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = BasicEditRequest {
            file_path: path.clone(),
            edits: vec![BasicEdit {
                old_text: "test".to_string(),
                new_text: "replaced".to_string(),
                replace_all: true,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("3 replacement(s)"));

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "replaced replaced replaced\n");
    }

    #[tokio::test]
    async fn test_replace_all_false_with_duplicates_fails() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "test test\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = BasicEditRequest {
            file_path: path,
            edits: vec![BasicEdit {
                old_text: "test".to_string(),
                new_text: "replaced".to_string(),
                replace_all: false,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("appears 2 times"));
        assert!(err_msg.contains("replace_all: true"));
    }

    #[tokio::test]
    async fn test_replace_all_with_zero_occurrences_fails() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "nothing here\n").unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let request = BasicEditRequest {
            file_path: path,
            edits: vec![BasicEdit {
                old_text: "missing".to_string(),
                new_text: "replaced".to_string(),
                replace_all: true,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    /// Test that documents a footgun: replace operation only replaces the exact anchor text.
    /// If you replace just a section header with new content, the OLD section body remains
    /// and appears AFTER your new content, causing duplication.
    ///
    /// This test demonstrates the bug observed in session 01KGBFGZV57E7C7VFESQNFW7SR:
    /// - LLM used anchor: "## Section Header"
    /// - LLM provided content: "## Section Header\n\nNew body content..."
    /// - Result: The old body content remained after the new content = duplication
    ///
    /// CORRECT USAGE: The anchor must include ALL text you want to replace,
    /// not just a header line.
    #[tokio::test]
    async fn test_replace_anchor_only_replaces_exact_match_not_section() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // Original file with a section header and body
        write!(
            temp_file,
            "## Section One\n\nOld body content.\nMore old content.\n\n## Section Two\n\nOther stuff.\n"
        )
        .unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        // WRONG: Only replacing the header, not the whole section
        let request = ExtendedEditRequest {
            file_path: path.clone(),
            operation: Operation::Replace,
            anchor: "## Section One".to_string(),
            // New content includes the header and new body
            content: "## Section One\n\nNew body content.\n".to_string(),
        };

        let result = execute_extended_operation(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();

        // BUG DEMONSTRATION: The old body content is STILL there after the new content!
        // This is because replace only replaced "## Section One" literally.
        assert!(
            content.contains("Old body content"),
            "Old body should still exist (this demonstrates the footgun)"
        );
        assert!(
            content.contains("New body content"),
            "New body should exist"
        );

        // The file now has BOTH old and new content - duplication!
        let expected_buggy_result =
            "## Section One\n\nNew body content.\n\n\nOld body content.\nMore old content.\n\n## Section Two\n\nOther stuff.\n";
        assert_eq!(
            content, expected_buggy_result,
            "Replace only substitutes the anchor text, old section body remains"
        );
    }

    /// Test that whitespace mismatch produces helpful diagnostic
    #[tokio::test]
    async fn test_whitespace_mismatch_diagnostic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // File has 4 blank lines between sections
        write!(
            temp_file,
            "Section One\n\n\n\n\nSection Two\n"
        )
        .unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        // Search with only 1 blank line - should fail with helpful message
        let request = BasicEditRequest {
            file_path: path,
            edits: vec![BasicEdit {
                old_text: "Section One\n\nSection Two".to_string(), // Only 1 blank line
                new_text: "Replaced".to_string(),
                replace_all: false,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err();

        // Should detect whitespace mismatch
        assert!(
            err_msg.contains("Whitespace mismatch") || err_msg.contains("whitespace"),
            "Error should mention whitespace mismatch. Got:\n{}",
            err_msg
        );
    }

    /// Test that the diff output shows actual vs expected
    #[tokio::test]
    async fn test_whitespace_mismatch_shows_diff() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // File has specific whitespace
        write!(
            temp_file,
            "Header\n\n\nBody text here\n"
        )
        .unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        // Search with different whitespace
        let request = BasicEditRequest {
            file_path: path,
            edits: vec![BasicEdit {
                old_text: "Header\nBody text here".to_string(), // Missing blank lines
                new_text: "Replaced".to_string(),
                replace_all: false,
            }],
        };

        let result = execute_basic_edits(&request).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err();

        // Should show line counts
        assert!(
            err_msg.contains("lines") && err_msg.contains("blank"),
            "Error should show line counts. Got:\n{}",
            err_msg
        );

        // Should show diff
        assert!(
            err_msg.contains("Diff") || err_msg.contains("-") && err_msg.contains("+"),
            "Error should show diff. Got:\n{}",
            err_msg
        );
    }

    /// Test the CORRECT way to replace a section: include the full section in the anchor
    #[tokio::test]
    async fn test_replace_full_section_correctly() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(
            temp_file,
            "## Section One\n\nOld body content.\nMore old content.\n\n## Section Two\n\nOther stuff.\n"
        )
        .unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        // CORRECT: Include the entire section in the anchor
        let request = ExtendedEditRequest {
            file_path: path.clone(),
            operation: Operation::Replace,
            anchor: "## Section One\n\nOld body content.\nMore old content.\n".to_string(),
            content: "## Section One\n\nNew body content.\n".to_string(),
        };

        let result = execute_extended_operation(&request).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();

        // No duplication - old content is fully replaced
        assert!(
            !content.contains("Old body content"),
            "Old body should be gone"
        );
        assert!(
            content.contains("New body content"),
            "New body should exist"
        );

        let expected = "## Section One\n\nNew body content.\n\n## Section Two\n\nOther stuff.\n";
        assert_eq!(content, expected);
    }
}
