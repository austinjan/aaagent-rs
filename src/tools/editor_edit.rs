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
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit"
                },
                "edits": {
                    "type": "array",
                    "description": "List of edit operations (basic mode)",
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
                    "description": "Operation type for extended mode"
                },
                "anchor": {
                    "type": "string",
                    "description": "Text to locate (for replace/insert/delete operations)"
                },
                "content": {
                    "type": "string",
                    "description": "New content (for replace/insert/append/prepend operations)"
                }
            },
            "required": ["file_path"],
            "oneOf": [
                {
                    "required": ["edits"],
                    "description": "Basic mode: multiple search-and-replace edits"
                },
                {
                    "required": ["operation"],
                    "description": "Extended mode: single operation with anchor/content"
                }
            ]
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
            return Err(format!(
                "Edit #{}: old_text not found in file.\nSearching for:\n{}\n",
                idx + 1,
                edit.old_text
            ));
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
                return Err(format!(
                    "Anchor not found in file.\nSearching for:\n{}\n",
                    request.anchor
                ));
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
}
