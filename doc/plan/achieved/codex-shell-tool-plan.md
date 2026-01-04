# Feature Plan: codex-shell-tool

## Overview

Replace the current BashTool with a more robust shell command tool inspired by OpenAI Codex. The new implementation provides better cross-platform support, improved output handling, and more robust process management.

## Scope

- Port Codex shell command execution logic to Rust
- Replace BashTool with new ShellTool
- Maintain ToolProvider trait compatibility
- Update registry and examples

## Non-Goals

- Sandboxing (separate feature)
- Approval workflow (separate feature)

## References

- Codex CLI source: `D:\code\codex\codex-rs\core\src\shell.rs`
- Codex exec module: `D:\code\codex\codex-rs\core\src\exec.rs`
- Current BashTool: `src/tools/bash.rs`

---

## TODO

(All tasks completed)

---

## DONE

### Research (2025-01-04)
- [x] Research Codex shell implementation
  - Reviewed `shell.rs` for shell detection logic
  - Reviewed `exec.rs` for process spawning and output handling
  - Reviewed `spawn.rs` for command execution patterns
  - Identified key features: ShellType enum, derive_exec_args, default shell detection

### Implementation (2025-01-04)
- [x] Implement ShellTool (`src/tools/shell.rs`)
  - ShellType enum (Zsh, Bash, Sh, PowerShell, Cmd)
  - Shell struct with derive_exec_args()
  - detect_shell_type() function
  - default_shell() with platform-specific logic
  - Cross-platform shell detection using `which` crate
  - Process spawning with configurable timeout
  - Output capture with MAX_OUTPUT_BYTES truncation
  - Working directory support via with_working_dir()
  - kill_on_drop for proper process cleanup

### Tool Definition (2025-01-04)
- [x] Tool definition
  - JSON schema for command and optional timeout parameters
  - Platform-aware full_description() showing current shell
  - Brief description for token efficiency

### Integration (2025-01-04)
- [x] Integration
  - Added shell module to src/tools/mod.rs
  - Exported ShellTool
  - Updated all_tools() to use ShellTool instead of BashTool
  - Added deprecated all_tools_with_bash() for backwards compatibility
  - Updated registry tests to expect "shell" instead of "bash"

### Testing (2025-01-04)
- [x] Testing (86 tests passing)
  - test_detect_shell_type
  - test_shell_derive_exec_args
  - test_default_shell
  - test_simple_command
  - test_command_with_error
  - test_timeout
  - test_tool_definition

### Dependencies (2025-01-04)
- [x] Added `which = "7.0"` to Cargo.toml for shell detection

---

## Acceptance Criteria

- [x] ShellTool passes all unit tests
- [x] Works on Windows (cmd/powershell) and Unix (bash/sh)
- [x] Timeout handling works correctly
- [x] Output capture is reliable
- [x] Existing examples continue to work (uses registry which now has ShellTool)
- [x] BashTool deprecated (still available but not in default all_tools())

---

## Design Notes

### Shell Detection Priority

**Windows:**
1. PowerShell (pwsh if available via `which`, otherwise powershell)
2. cmd.exe as fallback

**Unix/macOS:**
1. $SHELL environment variable
2. /bin/zsh (macOS) or /bin/bash (Linux) 
3. /bin/sh as ultimate fallback

### Output Handling

- Combine stdout/stderr in order of arrival
- Truncate at MAX_OUTPUT_BYTES (100KB) with "[output truncated]" message
- UTF-8 lossy conversion for binary output
- Show exit code if non-zero: "[exit code: N]"
- Show "(no output)" for empty output

### Key Differences from BashTool

| Feature | BashTool | ShellTool |
|---------|----------|-----------|
| Shell detection | Hardcoded per platform | Dynamic via `which` + env |
| Shell types | bash/PowerShell | zsh/bash/sh/powershell/cmd |
| Output handling | Simple capture | Truncation + encoding |
| Timeout | Basic | Configurable per-call |
| Process cleanup | Manual | kill_on_drop |

---

## Completion

**Status:** COMPLETE  
**Date:** 2025-01-04  
**Summary:** Successfully implemented ShellTool as a replacement for BashTool with improved cross-platform support, shell detection, and output handling. All 86 tests passing.
