# EditorEditTool Documentation

## Overview

The `EditorEditTool` is a file editing tool designed specifically for LLM (Large Language Model) usage. It provides intuitive search-and-replace operations with multiple operation modes, using literal text matching without regex complexity.

**Module**: `km-tools::tools::editor_edit`  
**File**: `src/tools/editor_edit.rs`

## Key Features

- **No Regex Required**: Simple literal text matching - what you see is what you match
- **Safety First**: Uniqueness validation prevents accidental bulk replacements (optional for batch operations)
- **Flexible Replacement**: Support both single-match (default) and replace-all modes
- **Whitespace Aware**: Preserves exact indentation and formatting
- **Multiple Modes**: Support for replace, insert, delete, append, and prepend operations
- **LLM Optimized**: Clear error messages and intuitive API designed for AI agents

## Architecture

### Core Components

#### 1. EditorEditTool Struct

```rust
pub struct EditorEditTool;
```

A zero-sized type implementing the `ToolProvider` trait. Stateless and can be cloned freely.

**Methods**:
- `new() -> Self` - Create a new instance
- Implements `Default` trait

#### 2. Request Types

##### BasicEditRequest (Multiple Edits Mode)

```rust
struct BasicEditRequest {
    file_path: String,      // Absolute path to file
    edits: Vec<BasicEdit>,  // List of edit operations
}

struct BasicEdit {
    old_text: String,   // Exact text to find
    new_text: String,   // Replacement text
    replace_all: bool,  // If true, replace all occurrences (default: false)
}
```

Used for applying multiple search-and-replace operations in sequence.

**replace_all behavior**:
- `false` (default): `old_text` must be unique in the file (safety check)
- `true`: Replaces ALL occurrences of `old_text` (batch mode)

##### ExtendedEditRequest (Single Operation Mode)

```rust
struct ExtendedEditRequest {
    file_path: String,     // Absolute path to file
    operation: Operation,  // Operation type
    anchor: String,        // Text to locate (optional for some ops)
    content: String,       // New content (optional for some ops)
}
```

Used for single operations with more flexibility.

#### 3. Operation Types

```rust
enum Operation {
    Replace,       // Replace anchor with content
    InsertBefore,  // Insert content before anchor
    InsertAfter,   // Insert content after anchor
    Delete,        // Remove anchor
    Append,        // Add content to end of file
    Prepend,       // Add content to beginning of file
}
```

## ToolProvider Implementation

The tool implements the `ToolProvider` trait required by the tool registry system:

### name()
```rust
fn name(&self) -> &str
```
Returns: `"mcp__editor__Edit"`

### brief()
```rust
fn brief(&self) -> &str
```
Returns a token-efficient description shown to LLMs initially:
> "Edit files using search-and-replace. Supports replace, insert, delete, append, prepend. No regex - exact text matching only."

### full_description()
```rust
fn full_description(&self) -> String
```
Returns comprehensive documentation with:
- Operation mode explanations
- Safety rules
- Usage examples for each mode
- Common patterns

### parameters()
```rust
fn parameters(&self) -> serde_json::Value
```
Returns JSON Schema defining valid request formats using `oneOf` to support both basic and extended modes.

### execute()
```rust
fn execute<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<String, String>>
```
Async execution handler that:
1. Attempts to parse request as `BasicEditRequest`
2. Falls back to `ExtendedEditRequest`
3. Returns error if neither format matches
4. Delegates to appropriate execution function

## Core Algorithms

### execute_basic_edits()

```rust
async fn execute_basic_edits(request: &BasicEditRequest) -> Result<String, String>
```

**Process**:
1. Read file content into memory
2. For each edit operation:
   - Count occurrences of `old_text`
   - If `replace_all` is false: Validate it appears exactly once (safety check)
   - If `replace_all` is true: Skip uniqueness check
   - Apply replacement using `str::replace()`
   - Track total number of replacements
3. Write modified content back to file
4. Return success message with edit count and total replacements

**Error Cases**:
- File not found or unreadable
- `old_text` not found (0 occurrences)
- `old_text` not unique when `replace_all` is false (>1 occurrences)
- File write failure

**Example Error**:
```
Edit #2: old_text appears 3 times (must be unique).
Searching for:
fn test()

Include more context to make it unique, or set replace_all: true.
```

**Success Message**:
```
Successfully applied 2 edit(s) (5 replacement(s)) to '/path/to/file.rs'
```
Shows both the number of edit operations and total text replacements.

### execute_extended_operation()

```rust
async fn execute_extended_operation(request: &ExtendedEditRequest) -> Result<String, String>
```

**Process by Operation Type**:

#### Append
1. Read file content
2. Append `content` to end
3. Write back
4. No anchor validation needed

#### Prepend
1. Read file content
2. Prepend `content` to beginning
3. Write back
4. No anchor validation needed

#### Replace / InsertBefore / InsertAfter / Delete
1. Validate `anchor` is provided
2. Read file content
3. Count occurrences of `anchor`
4. Validate appears exactly once
5. Apply operation:
   - **Replace**: `content.replace(anchor, content)`
   - **InsertBefore**: `content.replace(anchor, content + anchor)`
   - **InsertAfter**: `content.replace(anchor, anchor + content)`
   - **Delete**: `content.replace(anchor, "")`
6. Write back

**Error Cases**:
- Operation requires anchor but none provided
- Anchor not found
- Anchor not unique
- File I/O errors

## Usage Examples

### Basic Mode: Multiple Replacements

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
      "new_text": "println!(\"updated\")"
    }
  ]
}
```

**Use Case**: Renaming functions, updating strings, fixing typos across a file.

### Basic Mode: Replace All Occurrences

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

**Use Case**: Renaming variables that appear multiple times, batch replacements.

**Example**: If `old_var` appears 5 times in the file, all 5 will be replaced with `new_var`.

### Extended Mode: Insert After

```json
{
  "file_path": "/path/to/main.rs",
  "operation": "insert_after",
  "anchor": "use std::io;\n",
  "content": "use std::fs;\n"
}
```

**Use Case**: Adding imports, inserting new code after specific markers.

### Extended Mode: Insert Before

```json
{
  "file_path": "/path/to/main.rs",
  "operation": "insert_before",
  "anchor": "fn main() {",
  "content": "/// Main entry point\n"
}
```

**Use Case**: Adding documentation, inserting headers before functions.

### Extended Mode: Delete

```json
{
  "file_path": "/path/to/main.rs",
  "operation": "delete",
  "anchor": "    // TODO: remove\n    old_code();\n"
}
```

**Use Case**: Removing commented code, cleaning up sections.

### Extended Mode: Append

```json
{
  "file_path": "/path/to/main.rs",
  "operation": "append",
  "content": "\n#[cfg(test)]\nmod tests {\n    // tests\n}\n"
}
```

**Use Case**: Adding test modules, appending new functions.

### Extended Mode: Prepend

```json
{
  "file_path": "/path/to/main.rs",
  "operation": "prepend",
  "content": "// Copyright 2024\n// Licensed under MIT\n\n"
}
```

**Use Case**: Adding file headers, copyright notices.

## Safety and Best Practices

### Uniqueness Requirement (Default Behavior)

By default (`replace_all: false`), the tool enforces that `old_text` and `anchor` must appear **exactly once** in the file. This prevents:
- Accidental bulk replacements
- Ambiguous edit locations
- Unintended side effects

**Solution for Non-Unique Matches**:

**Option 1**: Include more surrounding context to make the anchor unique.

❌ **Too Generic**:
```json
{"old_text": "return"}
```

✅ **Specific Context**:
```json
{
  "old_text": "fn calculate() {\n    return 42\n}"
}
```

**Option 2**: Use `replace_all: true` for intentional batch replacements.

```json
{
  "old_text": "old_var",
  "new_text": "new_var",
  "replace_all": true
}
```

### When to Use replace_all

**Use `replace_all: true` when**:
- Renaming variables/functions that appear multiple times
- Updating repeated strings throughout a file
- Making consistent changes across the entire file
- You're certain all occurrences should be changed

**Use `replace_all: false` (default) when**:
- Making surgical, precise edits
- You want safety validation
- Unsure how many times the text appears
- Making structural changes that should be unique

### Whitespace Preservation

The tool uses exact string matching, so whitespace matters:

❌ **Won't Match**:
```json
{"old_text": "fn test() {"}  // Missing indentation
```

File contains:
```rust
    fn test() {  // Has 4 spaces before
```

✅ **Correct**:
```json
{"old_text": "    fn test() {"}  // Includes indentation
```

**Tip**: Copy the exact text from the file, including all whitespace.

### Newline Handling

Include newlines in your anchors when needed:

```json
{
  "operation": "insert_after",
  "anchor": "use std::io;\n",
  "content": "use std::fs;\n"
}
```

This ensures the new content appears on a new line.

### Error Messages

The tool provides detailed error messages to help LLMs understand and fix issues:

```
Edit #1: old_text appears 2 times (must be unique).
Searching for:
fn test()

Include more context to make it unique, or set replace_all: true.
```

```
Anchor not found in file.
Searching for:
fn missing_function()
```

**Note**: When `replace_all: true` is set, the uniqueness check is bypassed, but the text must still exist in the file (at least 1 occurrence).

## Integration with Tool Registry

### Registration

The tool is automatically registered in `all_tools()`:

```rust
// src/tools/mod.rs
pub fn all_tools() -> Vec<Arc<dyn ToolProvider>> {
    vec![
        Arc::new(BashTool::new()),
        Arc::new(EditorEditTool::new()),
    ]
}
```

### Usage in Chat Loop

```rust
use km_tools::llm::registry::ToolRegistry;
use km_tools::tools;

// Create registry and register all tools
let mut registry = ToolRegistry::new();
for tool in tools::all_tools() {
    registry.register(tool);
}

// LLM can now use pick_tools to access mcp__editor__Edit
```

### Lazy Loading

The tool supports the registry's lazy loading pattern:
1. LLM initially sees only `brief()` description
2. LLM calls `pick_tools(["mcp__editor__Edit"])`
3. Registry provides full `parameters()` and `full_description()`
4. LLM can now use the tool effectively

## Testing

### Test Coverage

The module includes 13 comprehensive unit tests covering:

1. **test_basic_replace** - Basic search-and-replace operation
2. **test_multiple_edits** - Multiple edits in sequence
3. **test_non_unique_anchor_fails** - Validation for duplicate anchors
4. **test_anchor_not_found_fails** - Validation for missing anchors
5. **test_insert_after** - Insert operation after anchor
6. **test_insert_before** - Insert operation before anchor
7. **test_delete** - Delete operation
8. **test_append** - Append to end of file
9. **test_prepend** - Prepend to beginning of file
10. **test_whitespace_preservation** - Exact whitespace matching
11. **test_replace_all_multiple_occurrences** - Replace all occurrences with replace_all: true
12. **test_replace_all_false_with_duplicates_fails** - Verify uniqueness check with replace_all: false
13. **test_replace_all_with_zero_occurrences_fails** - Verify replace_all still requires text to exist

### Running Tests

```bash
cd km-tools
cargo test editor_edit --lib
```

**Expected Output**:
```
running 13 tests
test tools::editor_edit::tests::test_basic_replace ... ok
test tools::editor_edit::tests::test_multiple_edits ... ok
test tools::editor_edit::tests::test_non_unique_anchor_fails ... ok
test tools::editor_edit::tests::test_anchor_not_found_fails ... ok
test tools::editor_edit::tests::test_insert_after ... ok
test tools::editor_edit::tests::test_insert_before ... ok
test tools::editor_edit::tests::test_delete ... ok
test tools::editor_edit::tests::test_append ... ok
test tools::editor_edit::tests::test_prepend ... ok
test tools::editor_edit::tests::test_whitespace_preservation ... ok
test tools::editor_edit::tests::test_replace_all_multiple_occurrences ... ok
test tools::editor_edit::tests::test_replace_all_false_with_duplicates_fails ... ok
test tools::editor_edit::tests::test_replace_all_with_zero_occurrences_fails ... ok

test result: ok. 13 passed; 0 failed
```

### Test Structure

Tests use `tempfile` crate for temporary file handling:

```rust
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
    assert!(result.is_ok());

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "Hello, Rust!\n");
}
```

## Dependencies

### Runtime Dependencies
- `serde` - Deserialization of requests
- `serde_json` - JSON schema and parsing
- `tokio` - Async execution (already in project)
- `std::fs` - File I/O (standard library)

### Dev Dependencies
- `tempfile = "3"` - Temporary file creation for tests

All dependencies are already available in the project's `Cargo.toml`.

## Performance Considerations

### Memory Usage
- Entire file is loaded into memory
- Suitable for typical source code files (<10MB)
- Not optimized for very large files (>100MB)

### Algorithm Complexity
- `O(n)` for single replacement where n = file size
- Multiple edits: `O(n * m)` where m = number of edits
- Uniqueness check: `O(n)` per anchor

### Optimization Opportunities
For future enhancements:
- Stream-based processing for large files
- Batch uniqueness checking
- Line-based operations for better performance

## Design Decisions

### Choice 1: Literal Matching vs. Regex
**Decision**: Literal text matching only  
**Rationale**: 
- Simpler for LLMs to use correctly
- No escaping required
- Safer (prevents catastrophic backtracking)
- More predictable behavior

### Choice 2: Uniqueness Requirement
**Decision**: Strict uniqueness validation  
**Rationale**:
- Prevents accidental bulk changes
- Forces explicit, specific edits
- Better for LLM reasoning (clearer intent)
- User can always include more context

### Choice 3: Two API Modes
**Decision**: Support both basic (edits array) and extended (operation field) modes  
**Rationale**:
- Basic mode: Optimal for multiple replacements
- Extended mode: More intuitive for single operations
- Flexibility for different LLM prompting styles
- Backward compatibility if API evolves

### Choice 4: In-Memory Processing
**Decision**: Read entire file into memory  
**Rationale**:
- Simpler implementation
- Sufficient for typical source files
- Atomic write operation
- Can validate all anchors before any changes

## Common Patterns for LLMs

### Pattern 1: Renaming Variables (Single Occurrence)

```json
{
  "file_path": "/src/calculator.rs",
  "edits": [
    {
      "old_text": "let old_var = ",
      "new_text": "let new_var = "
    },
    {
      "old_text": "old_var.calculate()",
      "new_text": "new_var.calculate()"
    }
  ]
}
```

### Pattern 1b: Renaming Variables (Multiple Occurrences)

```json
{
  "file_path": "/src/calculator.rs",
  "edits": [
    {
      "old_text": "old_var",
      "new_text": "new_var",
      "replace_all": true
    }
  ]
}
```

**Use when**: The variable appears multiple times and you want to rename all occurrences at once.

### Pattern 2: Adding Imports

```json
{
  "file_path": "/src/main.rs",
  "operation": "insert_after",
  "anchor": "use std::io;\n",
  "content": "use std::fs;\nuse std::path::Path;\n"
}
```

### Pattern 3: Updating Function Signatures

```json
{
  "file_path": "/src/api.rs",
  "edits": [
    {
      "old_text": "pub fn process(data: &str)",
      "new_text": "pub fn process(data: &str, verbose: bool)"
    }
  ]
}
```

### Pattern 4: Adding Documentation

```json
{
  "file_path": "/src/utils.rs",
  "operation": "insert_before",
  "anchor": "pub fn helper() {",
  "content": "/// Helper function for processing data\n/// \n/// # Arguments\n/// * None\n"
}
```

## Troubleshooting

### Issue: "old_text appears 0 times"
**Cause**: Text doesn't exist in file or has different whitespace  
**Solution**: 
- Read the file first to get exact text
- Check for indentation differences
- Verify newline characters

### Issue: "old_text appears N times (must be unique)"
**Cause**: Text appears multiple times and `replace_all` is false (default)  
**Solution**: 
- **Option 1**: Include more surrounding context to make it unique
- **Option 2**: Add function name or unique identifiers to the match
- **Option 3**: Include line breaks to be more specific
- **Option 4**: Set `replace_all: true` if you want to replace all occurrences intentionally

### Issue: File write errors
**Cause**: Permission issues, file locked, disk full  
**Solution**: 
- Check file permissions
- Ensure file is not open in other programs
- Verify disk space

## Future Enhancements

Potential improvements for consideration:

1. **Line Range Operations**: Operate on specific line numbers
2. **Regex Support**: Optional regex mode for advanced users
3. **Backup Creation**: Automatic backup before edits
4. **Dry Run Mode**: Preview changes without applying
5. **Diff Output**: Show what changed
6. **Undo Support**: Maintain edit history
7. **Batch File Operations**: Edit multiple files in one call
8. **Streaming for Large Files**: Handle files >100MB efficiently

## Related Documentation

- [ToolProvider Trait](../src/tools/mod.rs) - Base trait interface
- [Tool Registry](REGISTRY_INTEGRATION_DESIGN.md) - Tool registration system
- [LLM Provider Architecture](LLM_PROVIDER.md) - Overall system design
- [BashTool](../src/tools/bash.rs) - Similar tool example

## Version History

- **v0.1.1** (2024) - Added replace_all feature
  - New `replace_all` flag for batch replacements
  - Enhanced error messages with suggestions
  - Success messages now show replacement counts
  - Added 3 new tests for replace_all functionality
  - Updated documentation with replace_all examples

- **v0.1.0** (2024) - Initial implementation
  - Basic and extended operation modes
  - Comprehensive test coverage
  - Integration with tool registry
  - All safety validations

---

**Module Path**: `km_tools::tools::editor_edit`  
**Maintainer**: km-tools project  
**License**: Same as parent project
