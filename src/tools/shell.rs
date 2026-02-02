//! Shell command execution tool
//!
//! Provides a cross-platform tool that allows LLMs to execute shell commands safely.
//! Uses PowerShell on Windows and sh/bash on Unix-like systems.

use super::ToolProvider;
use crate::llm::{Tool, ToolCall};
use serde_json::json;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Shell command execution tool
///
/// This tool allows LLMs to execute shell commands and receive output.
/// It automatically uses the appropriate shell for the platform:
/// - Windows: PowerShell
/// - Unix/Linux/macOS: sh (POSIX-compliant)
#[derive(Clone)]
pub struct ShellTool {
    /// Maximum execution time in seconds (default: 120)
    timeout_secs: u64,
    /// Working directory for command execution (default: current directory)
    working_dir: Option<std::path::PathBuf>,
    /// Extra directory to prepend to PATH (for built-in binaries)
    extra_path: Option<std::path::PathBuf>,
}

impl ShellTool {
    /// Create a new ShellTool with default settings
    pub fn new() -> Self {
        Self {
            timeout_secs: 120,
            working_dir: None,
            extra_path: None,
        }
    }

    /// Set the timeout in seconds
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set extra PATH directory (prepended to system PATH)
    ///
    /// This is typically used for built-in binaries provided by the agent.
    pub fn with_extra_path(mut self, dir: std::path::PathBuf) -> Self {
        self.extra_path = Some(dir);
        self
    }

    /// Set extra PATH from BuiltinBinaries
    pub fn with_builtin_binaries(mut self, builtins: &super::BuiltinBinaries) -> Self {
        let bin_dir = builtins.bin_dir();
        if bin_dir.as_os_str().is_empty() {
            self.extra_path = None;
        } else {
            self.extra_path = Some(bin_dir.to_path_buf());
        }
        self
    }

    fn error_context(&self, command: &str) -> String {
        let shell = if cfg!(target_os = "windows") {
            "powershell"
        } else {
            "sh"
        };

        let cwd = if let Some(dir) = &self.working_dir {
            dir.display().to_string()
        } else {
            std::env::current_dir()
                .map(|dir| dir.display().to_string())
                .unwrap_or_else(|_| "(unknown)".to_string())
        };

        format!("shell={}\ncwd={}\ncommand={}", shell, cwd, command)
    }

    fn combine_output(stdout: &str, stderr: &str) -> String {
        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n---STDERR---\n");
            }
            result.push_str(stderr);
        }
        result
    }

    /// Get the tool definition for LLM
    pub fn as_tool(&self) -> Tool {
        let os = std::env::consts::OS;

        let (shell_name, platform_note) = match os {
            "windows" => ("PowerShell", "Use standard PowerShell cmdlets and syntax."),
            _ => ("bash", "Use standard bash/POSIX commands and syntax."),
        };

        let description = format!(
            r#"Execute a shell command and return the output.

PLATFORM INFORMATION:
Current OS: {os}
Shell: {shell_name}
{platform_note}

USAGE NOTES:
- Commands execute in the current working directory
- Both stdout and stderr are captured and returned combined
- Stderr is separated with "---STDERR---" marker if present
- Empty output returns: "(Command completed successfully with no output)"
- Successfully executed commands (exit code 0) return Ok(output)
- Failed commands return Err(message) with exit code and context

WHEN TO USE:
✓ Execute system commands
✓ Run scripts or CLI tools
✓ Check system state (files, processes, environment)
✓ Automate shell operations
✓ Run build/test commands

WHEN NOT TO USE:
✗ Simple file reading (use file I/O instead for better performance)
✗ Commands requiring user interaction (stdin not supported)
✗ Operations needing elevated permissions (no sudo/admin elevation)
✗ Long-running services (use background execution tools instead)
"#,
            os = os,
            shell_name = shell_name,
            platform_note = platform_note,
        );

        Tool {
            name: "shell".to_string(),
            description,
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute."
                    }
                },
                "required": ["command"]
            }),
            full_description: None,
        }
    }

    /// Execute a command from a ToolCall
    ///
    /// Returns the command output (stdout + stderr combined) or error message
    pub async fn execute(&self, tool_call: &ToolCall) -> Result<String, String> {
        // Extract command from arguments
        let command = tool_call
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "Missing 'command' argument\n{}",
                    self.error_context("<missing>")
                )
            })?;

        self.execute_command(command).await
    }

    /// Execute a raw command string
    pub async fn execute_command(&self, command: &str) -> Result<String, String> {
        if command.trim().is_empty() {
            return Err(format!(
                "Command cannot be empty\n{}",
                self.error_context(command)
            ));
        }

        // Build the command based on platform
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("powershell");
            c.args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        // Set working directory if specified
        if let Some(dir) = &self.working_dir {
            cmd.current_dir(dir);
        }

        // Inject extra PATH for built-in binaries
        if let Some(extra_path) = &self.extra_path {
            let path_separator = if cfg!(target_os = "windows") {
                ";"
            } else {
                ":"
            };
            let new_path = match std::env::var("PATH") {
                Ok(existing) if !existing.is_empty() => {
                    format!("{}{}{}", extra_path.display(), path_separator, existing)
                }
                _ => extra_path.display().to_string(),
            };
            cmd.env("PATH", new_path);
        }

        // Configure stdio
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "Failed to spawn command: {}\n{}",
                e,
                self.error_context(command)
            )
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("Failed to capture stdout\n{}", self.error_context(command)))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| format!("Failed to capture stderr\n{}", self.error_context(command)))?;

        let stdout_handle = tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut reader = tokio::io::BufReader::new(stdout);
            match reader.read_to_end(&mut buf).await {
                Ok(_) => Ok(buf),
                Err(e) => Err(e.to_string()),
            }
        });

        let stderr_handle = tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut reader = tokio::io::BufReader::new(stderr);
            match reader.read_to_end(&mut buf).await {
                Ok(_) => Ok(buf),
                Err(e) => Err(e.to_string()),
            }
        });

        // Wait with timeout (using tokio's async wait)
        let timeout = Duration::from_secs(self.timeout_secs);
        let status = match tokio::time::timeout(timeout, child.wait()).await {
            Ok(result) => result.map_err(|e| {
                format!(
                    "Command execution failed: {}\n{}",
                    e,
                    self.error_context(command)
                )
            })?,
            Err(_) => {
                let kill_result = match child.kill().await {
                    Ok(_) => "killed".to_string(),
                    Err(e) => format!("kill failed: {}", e),
                };
                let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;

                let stdout_text = match stdout_handle.await {
                    Ok(Ok(bytes)) => String::from_utf8_lossy(&bytes).to_string(),
                    Ok(Err(e)) => format!("(failed to read stdout: {})", e),
                    Err(e) => format!("(failed to join stdout reader: {})", e),
                };
                let stderr_text = match stderr_handle.await {
                    Ok(Ok(bytes)) => String::from_utf8_lossy(&bytes).to_string(),
                    Ok(Err(e)) => format!("(failed to read stderr: {})", e),
                    Err(e) => format!("(failed to join stderr reader: {})", e),
                };
                let output = Self::combine_output(&stdout_text, &stderr_text);

                return Err(format!(
                    "Command timed out after {} seconds (kill: {})\n{}\n{}",
                    self.timeout_secs,
                    kill_result,
                    if output.is_empty() {
                        "(no output)".to_string()
                    } else {
                        output
                    },
                    self.error_context(command)
                ));
            }
        };

        let stdout_bytes = stdout_handle
            .await
            .map_err(|e| {
                format!(
                    "Failed to join stdout reader: {}\n{}",
                    e,
                    self.error_context(command)
                )
            })?
            .map_err(|e| {
                format!(
                    "Failed to read stdout: {}\n{}",
                    e,
                    self.error_context(command)
                )
            })?;
        let stderr_bytes = stderr_handle
            .await
            .map_err(|e| {
                format!(
                    "Failed to join stderr reader: {}\n{}",
                    e,
                    self.error_context(command)
                )
            })?
            .map_err(|e| {
                format!(
                    "Failed to read stderr: {}\n{}",
                    e,
                    self.error_context(command)
                )
            })?;

        // Combine stdout and stderr
        let stdout = String::from_utf8_lossy(&stdout_bytes);
        let stderr = String::from_utf8_lossy(&stderr_bytes);
        let result = Self::combine_output(&stdout, &stderr);

        // Check exit status
        if status.success() {
            Ok(if result.is_empty() {
                "(Command completed successfully with no output)".to_string()
            } else {
                result
            })
        } else {
            let exit_code = status.code().unwrap_or(-1);
            Err(format!(
                "Command failed with exit code {}\n{}\n{}",
                exit_code,
                if result.is_empty() {
                    "(no output)".to_string()
                } else {
                    result
                },
                self.error_context(command)
            ))
        }
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolProvider for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn brief(&self) -> &str {
        "Execute shell commands and return output."
    }

    fn full_description(&self) -> String {
        // Reuse the description generation from as_tool()
        let os = std::env::consts::OS;
        let (shell_name, platform_note) = match os {
            "windows" => ("PowerShell", "Use standard PowerShell cmdlets and syntax."),
            _ => ("bash", "Use standard bash/POSIX commands and syntax."),
        };

        format!(
            r#"Execute a shell command and return the output.

PLATFORM INFORMATION:
Current OS: {os}
Shell: {shell_name}
{platform_note}

USAGE NOTES:
- Commands execute in the current working directory
- Both stdout and stderr are captured and returned combined
- Stderr is separated with "---STDERR---" marker if present
- Empty output returns: "(Command completed successfully with no output)"
- Successfully executed commands (exit code 0) return Ok(output)
- Failed commands return Err(message) with exit code and context

WHEN TO USE:
✓ Execute system commands
✓ Run scripts or CLI tools
✓ Check system state (files, processes, environment)
✓ Automate shell operations
✓ Run build/test commands

WHEN NOT TO USE:
✗ Simple file reading (use file I/O instead for better performance)
✗ Commands requiring user interaction (stdin not supported)
✗ Operations needing elevated permissions (no sudo/admin elevation)
✗ Long-running services (use background execution tools instead)

CONSTRAINTS:
- Timeout: {timeout} seconds maximum
- No stdin support (interactive commands will hang or fail)
- Runs with current user permissions only
"#,
            os = os,
            shell_name = shell_name,
            platform_note = platform_note,
            timeout = self.timeout_secs,
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute."
                }
            },
            "required": ["command"]
        })
    }

    fn execute<'a>(&'a self, call: &'a ToolCall) -> super::BoxFuture<'a, Result<String, String>> {
        Box::pin(async move { ShellTool::execute(self, call).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_command() {
        let tool = ShellTool::new();
        let result = tool.execute_command("echo hello").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_command_with_error() {
        let tool = ShellTool::new();
        let result = tool.execute_command("exit 1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timeout() {
        let tool = ShellTool::new().with_timeout(1);
        let result = if cfg!(target_os = "windows") {
            tool.execute_command("timeout /t 5").await
        } else {
            tool.execute_command("sleep 5").await
        };
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Windows timeout command may exit immediately on non-interactive sessions
        // Just verify we got an error
        assert!(!err.is_empty());
    }

    #[tokio::test]
    async fn test_as_tool() {
        let tool = ShellTool::new();
        let tool_def = tool.as_tool();
        assert_eq!(tool_def.name, "shell");
        assert!(tool_def.description.contains("Execute"));
    }
}
