//! Built-in binary management for shell execution and skill eligibility
//!
//! This module provides a registry of built-in binaries that are available
//! to shell commands and skill eligibility checks, independent of system PATH.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Registry of built-in binaries available to the agent
///
/// Built-in binaries are executables provided by the agent itself,
/// stored in a dedicated `bin/` directory. These binaries are:
/// - Available to shell commands via PATH injection
/// - Recognized by skill eligibility checks
#[derive(Debug, Clone)]
pub struct BuiltinBinaries {
    /// Directory containing built-in binaries
    bin_dir: PathBuf,
    /// Set of known binary names (without extension)
    known: HashSet<String>,
}

impl BuiltinBinaries {
    /// Create a new BuiltinBinaries from a bin directory
    ///
    /// Scans the directory for executable files and registers them.
    /// If the directory doesn't exist, returns an empty registry.
    pub fn from_dir(bin_dir: impl Into<PathBuf>) -> Self {
        let bin_dir = bin_dir.into();
        let known = Self::scan_binaries(&bin_dir);

        Self { bin_dir, known }
    }

    /// Create with explicit binary names (useful for testing or embedded binaries)
    pub fn with_names(bin_dir: impl Into<PathBuf>, names: impl IntoIterator<Item = String>) -> Self {
        Self {
            bin_dir: bin_dir.into(),
            known: names.into_iter().collect(),
        }
    }

    /// Create an empty registry (no built-in binaries)
    pub fn empty() -> Self {
        Self {
            bin_dir: PathBuf::new(),
            known: HashSet::new(),
        }
    }

    /// Get the bin directory path
    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    /// Check if a binary name is a known built-in
    pub fn has(&self, name: &str) -> bool {
        self.known.contains(name)
    }

    /// Get all known binary names
    pub fn all(&self) -> impl Iterator<Item = &str> {
        self.known.iter().map(|s| s.as_str())
    }

    /// Get count of registered binaries
    pub fn len(&self) -> usize {
        self.known.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }

    /// Add a binary name to the registry
    pub fn register(&mut self, name: impl Into<String>) {
        self.known.insert(name.into());
    }

    /// Remove a binary name from the registry
    pub fn unregister(&mut self, name: &str) -> bool {
        self.known.remove(name)
    }

    /// Build PATH string with bin_dir prepended to existing PATH
    ///
    /// Returns a PATH-compatible string: `{bin_dir}:{existing_path}` on Unix
    /// or `{bin_dir};{existing_path}` on Windows
    pub fn build_path_env(&self) -> String {
        let separator = if cfg!(target_os = "windows") {
            ";"
        } else {
            ":"
        };

        let bin_dir_str = self.bin_dir.display().to_string();

        match std::env::var("PATH") {
            Ok(existing) if !existing.is_empty() => {
                format!("{}{}{}", bin_dir_str, separator, existing)
            }
            _ => bin_dir_str,
        }
    }

    /// Scan a directory for executable files
    fn scan_binaries(dir: &Path) -> HashSet<String> {
        let mut binaries = HashSet::new();

        if !dir.exists() || !dir.is_dir() {
            return binaries;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Skip directories
                if path.is_dir() {
                    continue;
                }

                // Get the file name
                if let Some(name) = path.file_stem() {
                    let name_str = name.to_string_lossy().to_string();

                    // On Unix, check if file is executable
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(metadata) = path.metadata() {
                            let mode = metadata.permissions().mode();
                            // Check if any execute bit is set
                            if mode & 0o111 != 0 {
                                binaries.insert(name_str);
                            }
                        }
                    }

                    // On Windows, check for common executable extensions
                    #[cfg(windows)]
                    {
                        if let Some(ext) = path.extension() {
                            let ext_lower = ext.to_string_lossy().to_lowercase();
                            if matches!(ext_lower.as_str(), "exe" | "cmd" | "bat" | "ps1") {
                                binaries.insert(name_str);
                            }
                        }
                    }

                    // On other platforms, just include all files
                    #[cfg(not(any(unix, windows)))]
                    {
                        binaries.insert(name_str);
                    }
                }
            }
        }

        binaries
    }

    /// Rescan the bin directory for changes
    pub fn rescan(&mut self) {
        self.known = Self::scan_binaries(&self.bin_dir);
    }
}

impl Default for BuiltinBinaries {
    fn default() -> Self {
        Self::empty()
    }
}

/// Check if a binary is available (either built-in or in system PATH)
///
/// This is the unified check used by skill eligibility.
pub fn is_binary_available(name: &str, builtins: &BuiltinBinaries) -> bool {
    builtins.has(name) || which::which(name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_empty_registry() {
        let builtins = BuiltinBinaries::empty();
        assert!(builtins.is_empty());
        assert!(!builtins.has("anything"));
    }

    #[test]
    fn test_with_names() {
        let builtins = BuiltinBinaries::with_names(
            "/fake/bin",
            vec!["foo".to_string(), "bar".to_string()],
        );

        assert_eq!(builtins.len(), 2);
        assert!(builtins.has("foo"));
        assert!(builtins.has("bar"));
        assert!(!builtins.has("baz"));
    }

    #[test]
    fn test_register_unregister() {
        let mut builtins = BuiltinBinaries::empty();

        builtins.register("my-tool");
        assert!(builtins.has("my-tool"));

        builtins.unregister("my-tool");
        assert!(!builtins.has("my-tool"));
    }

    #[test]
    fn test_build_path_env() {
        let builtins = BuiltinBinaries::with_names("/my/bin", vec![]);
        let path = builtins.build_path_env();

        assert!(path.starts_with("/my/bin"));

        // Should include system PATH
        if let Ok(system_path) = std::env::var("PATH") {
            if !system_path.is_empty() {
                assert!(path.contains(&system_path));
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_scan_executable_files() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let bin_dir = temp.path();

        // Create an executable file
        let exec_path = bin_dir.join("my-exec");
        std::fs::write(&exec_path, "#!/bin/sh\necho hello").unwrap();
        std::fs::set_permissions(&exec_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Create a non-executable file
        let non_exec_path = bin_dir.join("not-exec");
        std::fs::write(&non_exec_path, "just data").unwrap();
        std::fs::set_permissions(&non_exec_path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let builtins = BuiltinBinaries::from_dir(bin_dir);

        assert!(builtins.has("my-exec"), "Should find executable file");
        assert!(!builtins.has("not-exec"), "Should not find non-executable file");
    }

    #[test]
    fn test_is_binary_available_builtin() {
        let builtins = BuiltinBinaries::with_names("/bin", vec!["custom-tool".to_string()]);

        assert!(is_binary_available("custom-tool", &builtins));
    }

    #[test]
    fn test_is_binary_available_system() {
        let builtins = BuiltinBinaries::empty();

        // "sh" should exist on any Unix system, "cmd" on Windows
        #[cfg(unix)]
        assert!(is_binary_available("sh", &builtins));

        #[cfg(windows)]
        assert!(is_binary_available("cmd", &builtins));
    }

    #[test]
    fn test_nonexistent_dir() {
        let builtins = BuiltinBinaries::from_dir("/nonexistent/path/12345");
        assert!(builtins.is_empty());
    }
}
