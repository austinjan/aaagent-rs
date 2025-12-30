//! Bash command execution tool
//!
//! Provides a tool that allows LLMs to execute bash/shell commands safely.

use crate::llm::{Tool, ToolCall};
use serde_json::json;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Bash command execution tool
///
/// This tool allows LLMs to execute shell commands and receive output.
/// It includes timeout support and error handling.
#[derive(Clone)]
pub struct BashTool {
    /// Maximum execution time in seconds (default: 30)
    timeout_secs: u64,
    /// Working directory for command execution (default: current directory)
    working_dir: Option<std::path::PathBuf>,
}

impl BashTool {
    /// Create a new BashTool with default settings
    pub fn new() -> Self {
        Self {
            timeout_secs: 30,
            working_dir: None,
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

        let (shell_name, rules, examples) = match os {
            "windows" => (
                "PowerShell",
                "On Windows, you MUST generate PowerShell commands. \
                 DO NOT use bash syntax or Unix utilities (ls, grep, awk, sed, cat, rm, etc.).",
                r#"Examples:
    - List files: Get-ChildItem
    - Read file: Get-Content file.txt
    - Search text: Select-String "foo" file.txt"#,
            ),
            _ => (
                "bash",
                "On Linux/macOS, you MUST generate bash-compatible shell commands.",
                r#"Examples:
    - List files: ls
    - Read file: cat file.txt
    - Search text: grep foo file.txt"#,
            ),
        };

        let description = format!(
            "Execute a shell command and return the output.\n\
             {rules}\n\
             Current OS: {os}\n\
             Shell: {shell_name}\n\
             {examples}"
        );

        Tool {
            name: "bash".to_string(), // keep stable name for LLM
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

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_command() {
        let tool = BashTool::new();
        let result = tool.execute_command("echo hello").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_command_with_error() {
        let tool = BashTool::new();
        let result = tool.execute_command("exit 1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_timeout() {
        let tool = BashTool::new().with_timeout(1);
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
        let tool = BashTool::new();
        let tool_def = tool.as_tool();
        assert_eq!(tool_def.name, "bash");
        assert!(tool_def.description.contains("Execute"));
    }
}
