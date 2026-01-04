//! Shell command execution tool with cross-platform support.
//!
//! This module provides a robust shell command execution tool that:
//! - Detects and uses the appropriate shell for each platform
//! - Supports configurable timeouts
//! - Captures stdout/stderr with proper encoding handling
//! - Works on Windows (PowerShell/cmd), macOS (zsh/bash), and Linux (bash/sh)

use super::{BoxFuture, ToolProvider};
use crate::llm::ToolCall;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

/// Default timeout for shell commands in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum output size in bytes before truncation
const MAX_OUTPUT_BYTES: usize = 100_000;

/// Shell types supported by the tool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Zsh,
    Bash,
    Sh,
    PowerShell,
    Cmd,
}

impl ShellType {
    /// Get the shell name for display
    pub fn name(&self) -> &'static str {
        match self {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::Sh => "sh",
            ShellType::PowerShell => "powershell",
            ShellType::Cmd => "cmd",
        }
    }
}

/// Shell configuration with path and type
#[derive(Debug, Clone)]
pub struct Shell {
    pub shell_type: ShellType,
    pub shell_path: PathBuf,
}

impl Shell {
    /// Derive the command arguments to execute a shell command
    pub fn derive_exec_args(&self, command: &str) -> Vec<String> {
        match self.shell_type {
            ShellType::Zsh | ShellType::Bash | ShellType::Sh => {
                vec![
                    self.shell_path.to_string_lossy().to_string(),
                    "-c".to_string(),
                    command.to_string(),
                ]
            }
            ShellType::PowerShell => {
                vec![
                    self.shell_path.to_string_lossy().to_string(),
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    command.to_string(),
                ]
            }
            ShellType::Cmd => {
                vec![
                    self.shell_path.to_string_lossy().to_string(),
                    "/c".to_string(),
                    command.to_string(),
                ]
            }
        }
    }
}

/// Detect shell type from a path
pub fn detect_shell_type(shell_path: &PathBuf) -> Option<ShellType> {
    let path_str = shell_path.as_os_str().to_str()?;

    // Check exact matches first
    match path_str {
        "zsh" => return Some(ShellType::Zsh),
        "bash" => return Some(ShellType::Bash),
        "sh" => return Some(ShellType::Sh),
        "cmd" | "cmd.exe" => return Some(ShellType::Cmd),
        "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe" => {
            return Some(ShellType::PowerShell)
        }
        _ => {}
    }

    // Check file stem (filename without extension)
    if let Some(stem) = shell_path.file_stem() {
        let stem_str = stem.to_string_lossy().to_lowercase();
        match stem_str.as_str() {
            "zsh" => return Some(ShellType::Zsh),
            "bash" => return Some(ShellType::Bash),
            "sh" => return Some(ShellType::Sh),
            "cmd" => return Some(ShellType::Cmd),
            "pwsh" | "powershell" => return Some(ShellType::PowerShell),
            _ => {}
        }
    }

    None
}

/// Get the default shell for the current platform
pub fn default_shell() -> Shell {
    #[cfg(windows)]
    {
        // Try PowerShell first, then cmd
        if let Ok(path) = which::which("pwsh") {
            return Shell {
                shell_type: ShellType::PowerShell,
                shell_path: path,
            };
        }
        if let Ok(path) = which::which("powershell") {
            return Shell {
                shell_type: ShellType::PowerShell,
                shell_path: path,
            };
        }
        Shell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd.exe"),
        }
    }

    #[cfg(not(windows))]
    {
        // Try to get user's shell from environment or passwd
        if let Ok(shell_env) = std::env::var("SHELL") {
            let path = PathBuf::from(&shell_env);
            if let Some(shell_type) = detect_shell_type(&path) {
                if std::fs::metadata(&path).is_ok() {
                    return Shell {
                        shell_type,
                        shell_path: path,
                    };
                }
            }
        }

        // Fallback: try common shells in order
        #[cfg(target_os = "macos")]
        let shells_to_try = [
            (ShellType::Zsh, "/bin/zsh"),
            (ShellType::Bash, "/bin/bash"),
            (ShellType::Sh, "/bin/sh"),
        ];

        #[cfg(not(target_os = "macos"))]
        let shells_to_try = [
            (ShellType::Bash, "/bin/bash"),
            (ShellType::Zsh, "/bin/zsh"),
            (ShellType::Sh, "/bin/sh"),
        ];

        for (shell_type, path) in shells_to_try {
            if std::fs::metadata(path).is_ok() {
                return Shell {
                    shell_type,
                    shell_path: PathBuf::from(path),
                };
            }
        }

        // Ultimate fallback
        Shell {
            shell_type: ShellType::Sh,
            shell_path: PathBuf::from("/bin/sh"),
        }
    }
}

/// Get platform information string
fn platform_info() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macOS"
    }
    #[cfg(target_os = "linux")]
    {
        "Linux"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "Unix"
    }
}

/// Shell command execution tool
#[derive(Clone)]
pub struct ShellTool {
    timeout_secs: u64,
    working_dir: Option<PathBuf>,
    shell: Shell,
}

impl ShellTool {
    /// Create a new ShellTool with default settings
    pub fn new() -> Self {
        Self {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            working_dir: None,
            shell: default_shell(),
        }
    }

    /// Set the timeout in seconds
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set a specific shell to use
    pub fn with_shell(mut self, shell: Shell) -> Self {
        self.shell = shell;
        self
    }

    /// Execute a command and return the output
    async fn execute_command(&self, command: &str) -> Result<String, String> {
        let args = self.shell.derive_exec_args(command);
        let (program, cmd_args) = args.split_first().ok_or("Empty command")?;

        let mut cmd = Command::new(program);
        cmd.args(cmd_args);

        // Set working directory
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        // Configure stdio
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Kill child process when handle is dropped
        cmd.kill_on_drop(true);

        // Spawn the process
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn process: {}", e))?;

        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

        // Read output with timeout
        let timeout_duration = Duration::from_secs(self.timeout_secs);

        let output_future = async {
            let mut stdout_reader = BufReader::new(stdout);
            let mut stderr_reader = BufReader::new(stderr);

            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();

            // Read stdout
            let mut line = String::new();
            while stdout_reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                stdout_buf.extend_from_slice(line.as_bytes());
                if stdout_buf.len() > MAX_OUTPUT_BYTES {
                    break;
                }
                line.clear();
            }

            // Read stderr
            line.clear();
            while stderr_reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                stderr_buf.extend_from_slice(line.as_bytes());
                if stderr_buf.len() > MAX_OUTPUT_BYTES {
                    break;
                }
                line.clear();
            }

            // Wait for process to complete
            let status = child.wait().await;

            (stdout_buf, stderr_buf, status)
        };

        match timeout(timeout_duration, output_future).await {
            Ok((stdout_buf, stderr_buf, status)) => {
                let stdout_str = String::from_utf8_lossy(&stdout_buf);
                let stderr_str = String::from_utf8_lossy(&stderr_buf);

                let mut output = String::new();

                // Add stdout if present
                if !stdout_str.is_empty() {
                    output.push_str(&stdout_str);
                }

                // Add stderr if present
                if !stderr_str.is_empty() {
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                    output.push_str(&stderr_str);
                }

                // Check truncation
                let truncated =
                    stdout_buf.len() > MAX_OUTPUT_BYTES || stderr_buf.len() > MAX_OUTPUT_BYTES;
                if truncated {
                    output.push_str("\n[output truncated]");
                }

                // Add exit code info if non-zero
                match status {
                    Ok(exit_status) => {
                        if !exit_status.success() {
                            let code = exit_status.code().unwrap_or(-1);
                            if output.is_empty() {
                                output = format!("Command exited with code {}", code);
                            } else {
                                output.push_str(&format!("\n[exit code: {}]", code));
                            }
                        }
                    }
                    Err(e) => {
                        return Err(format!("Process error: {}", e));
                    }
                }

                if output.is_empty() {
                    output = "(no output)".to_string();
                }

                Ok(output)
            }
            Err(_) => {
                // Timeout - child will be killed due to kill_on_drop
                Err(format!(
                    "Command timed out after {} seconds",
                    self.timeout_secs
                ))
            }
        }
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Arguments for the shell tool
#[derive(Debug, Deserialize)]
struct ShellArgs {
    command: String,
    #[serde(default)]
    timeout: Option<u64>,
}

impl ToolProvider for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn brief(&self) -> &str {
        "Execute shell commands on the system"
    }

    fn full_description(&self) -> String {
        let shell_name = self.shell.shell_type.name();
        let platform = platform_info();

        format!(
            r#"Execute shell commands on the system.

PLATFORM: {platform}
SHELL: {shell_name}

USAGE:
- Provide a command string to execute
- Optional timeout in seconds (default: {DEFAULT_TIMEOUT_SECS}s)

BEHAVIOR:
- Commands run in a non-interactive shell
- stdout and stderr are captured and returned
- Output is truncated if it exceeds ~100KB
- Process is killed if timeout is exceeded

RETURN FORMAT:
- Combined stdout/stderr output
- Exit code shown if non-zero
- Timeout message if command times out

CONSTRAINTS:
- No interactive commands (no stdin)
- Avoid long-running processes
- Be mindful of destructive commands

EXAMPLES:
- List files: {{"command": "ls -la"}}
- Check git status: {{"command": "git status"}}
- With timeout: {{"command": "sleep 5", "timeout": 10}}"#
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Optional timeout in seconds (default: 30)"
                }
            },
            "required": ["command"]
        })
    }

    fn execute<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let args: ShellArgs = serde_json::from_value(call.arguments.clone())
                .map_err(|e| format!("Invalid arguments: {}", e))?;

            // Create a modified tool with custom timeout if specified
            let tool = if let Some(timeout_secs) = args.timeout {
                ShellTool {
                    timeout_secs,
                    working_dir: self.working_dir.clone(),
                    shell: self.shell.clone(),
                }
            } else {
                self.clone()
            };

            tool.execute_command(&args.command).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell_type() {
        assert_eq!(
            detect_shell_type(&PathBuf::from("bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/bash")),
            Some(ShellType::Bash)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("/bin/zsh")),
            Some(ShellType::Zsh)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("powershell.exe")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("pwsh")),
            Some(ShellType::PowerShell)
        );
        assert_eq!(
            detect_shell_type(&PathBuf::from("cmd.exe")),
            Some(ShellType::Cmd)
        );
        assert_eq!(detect_shell_type(&PathBuf::from("fish")), None);
    }

    #[test]
    fn test_shell_derive_exec_args() {
        let bash = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };
        assert_eq!(
            bash.derive_exec_args("echo hello"),
            vec!["/bin/bash", "-c", "echo hello"]
        );

        let powershell = Shell {
            shell_type: ShellType::PowerShell,
            shell_path: PathBuf::from("pwsh.exe"),
        };
        assert_eq!(
            powershell.derive_exec_args("echo hello"),
            vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"]
        );

        let cmd = Shell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd.exe"),
        };
        assert_eq!(
            cmd.derive_exec_args("echo hello"),
            vec!["cmd.exe", "/c", "echo hello"]
        );
    }

    #[test]
    fn test_default_shell() {
        let shell = default_shell();
        // Just verify it returns something valid
        assert!(!shell.shell_path.as_os_str().is_empty());
    }

    #[tokio::test]
    async fn test_simple_command() {
        let tool = ShellTool::new();

        #[cfg(windows)]
        let result = tool.execute_command("echo hello").await;
        #[cfg(not(windows))]
        let result = tool.execute_command("echo hello").await;

        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_command_with_error() {
        let tool = ShellTool::new();

        #[cfg(windows)]
        let result = tool.execute_command("cmd /c exit 1").await;
        #[cfg(not(windows))]
        let result = tool.execute_command("exit 1").await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(
            output.contains("code 1") || output.contains("code: 1"),
            "Expected exit code in output, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_timeout() {
        let tool = ShellTool::new().with_timeout(1);

        #[cfg(windows)]
        let result = tool.execute_command("ping -n 10 127.0.0.1").await;
        #[cfg(not(windows))]
        let result = tool.execute_command("sleep 10").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timed out"));
    }

    #[test]
    fn test_tool_definition() {
        let tool = ShellTool::new();
        assert_eq!(tool.name(), "shell");
        assert!(!tool.brief().is_empty());
        assert!(!tool.full_description().is_empty());
    }
}
