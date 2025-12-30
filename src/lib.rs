use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

// check specific file exist in the input path, if not return path name
//
pub fn check_file_exist(file_name: &str, path: &Path) -> io::Result<Option<PathBuf>> {
    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.file_name().unwrap() == file_name {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

// Build a Gitignore matcher from patterns (compatible with .gitignore syntax)
//
pub fn build_gitignore(root: &Path, patterns: &[&str]) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    for pattern in patterns {
        let _ = builder.add_line(None, pattern);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

// Find all directories that don't have a README.md file
// ignore_patterns: list of patterns compatible with .gitignore syntax
//
pub fn find_missing_readme(dir: &Path, ignore_patterns: &[&str]) -> io::Result<Vec<String>> {
    let gitignore = build_gitignore(dir, ignore_patterns);
    let mut result = Vec::new();

    for entry in jwalk::WalkDir::new(dir)
        .skip_hidden(false)
        .process_read_dir(move |_, _, _, children| {
            // Filter out entries that match gitignore patterns
            children.retain(|child| {
                if let Ok(dir_entry) = child {
                    let path = dir_entry.path();
                    // Check if this path or any parent should be ignored
                    !gitignore.matched(&path, path.is_dir()).is_ignore()
                } else {
                    true
                }
            });
        })
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process directories
        if !path.is_dir() {
            continue;
        }

        // Skip the root directory itself
        if path == dir {
            continue;
        }

        // Check if README.md exists in this directory
        if check_file_exist("README.md", &path)?.is_none() {
            result.push(path.to_string_lossy().to_string());
        }
    }

    Ok(result)
}

#[derive(Debug)]
pub enum FrontMatterError {
    Io(io::Error),
    NotFound,
}

impl From<io::Error> for FrontMatterError {
    fn from(e: io::Error) -> Self {
        FrontMatterError::Io(e)
    }
}

/// Reads YAML front matter (`--- ... ---`) from the start of a markdown file.
/// Returns the raw YAML block (without the `---` lines).
pub fn read_front_matter<P: AsRef<Path>>(path: P) -> Result<String, FrontMatterError> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut line = String::new();

    // First line must be ---
    reader.read_line(&mut line)?;
    if line.trim() != "---" {
        return Err(FrontMatterError::NotFound);
    }

    let mut yaml = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            // EOF before closing ---
            return Err(FrontMatterError::NotFound);
        }

        if line.trim() == "---" {
            break;
        }

        yaml.push_str(&line);
    }

    Ok(yaml)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryInfo {
    pub path: String,
    pub depth: usize,
    pub yaml_matter: Option<HashMap<String, serde_yaml::Value>>,
}

/// Generate a map of the directory structure with README.md YAML front matter
///
/// # Arguments
/// * `dir` - Root directory to scan
/// * `ignore_patterns` - Patterns to ignore (gitignore syntax)
/// * `max_depth` - Maximum depth to traverse (0 = unlimited)
///
/// # Returns
/// Vector of DirectoryInfo containing path, depth, and YAML front matter
pub fn generate_map(
    dir: &Path,
    ignore_patterns: &[&str],
    max_depth: usize,
) -> io::Result<Vec<DirectoryInfo>> {
    let gitignore = build_gitignore(dir, ignore_patterns);
    let mut result = Vec::new();
    let root_depth = dir.components().count();

    for entry in jwalk::WalkDir::new(dir)
        .skip_hidden(false)
        .max_depth(if max_depth == 0 {
            usize::MAX
        } else {
            max_depth
        })
        .process_read_dir(move |_, _, _, children| {
            children.retain(|child| {
                if let Ok(dir_entry) = child {
                    let path = dir_entry.path();
                    !gitignore.matched(&path, path.is_dir()).is_ignore()
                } else {
                    true
                }
            });
        })
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process directories
        if !path.is_dir() {
            continue;
        }

        // Calculate depth relative to root
        let current_depth = path.components().count() - root_depth;

        // Try to extract YAML front matter if README.md exists
        let yaml_matter = if let Ok(Some(readme_path)) = check_file_exist("README.md", &path) {
            match read_front_matter(&readme_path) {
                Ok(yaml_str) => {
                    // Parse YAML into a HashMap
                    serde_yaml::from_str::<HashMap<String, serde_yaml::Value>>(&yaml_str).ok()
                }
                Err(_) => None,
            }
        } else {
            None
        };

        // Add all directories to the result, regardless of README.md presence
        result.push(DirectoryInfo {
            path: path.to_string_lossy().to_string(),
            depth: current_depth,
            yaml_matter,
        });
    }

    Ok(result)
}

/// Format the directory map as markdown for LLM consumption
pub fn format_map_as_markdown(dirs: &[DirectoryInfo], root_path: &Path) -> String {
    let mut output = String::new();

    output.push_str("# Project Structure Map\n\n");

    // Get absolute root path for display
    let abs_root = if let Ok(abs_path) = root_path.canonicalize() {
        abs_path.to_string_lossy().to_string()
    } else {
        root_path.to_string_lossy().to_string()
    };

    for dir_info in dirs {
        // For root directory (depth 0), show absolute path
        if dir_info.depth == 0 {
            output.push_str(&format!("`{}`", abs_root));

            if let Some(yaml) = &dir_info.yaml_matter {
                if let Some(desc) = yaml.get("description").and_then(|v| match v {
                    serde_yaml::Value::String(s) => Some(s.as_str()),
                    _ => None,
                }) {
                    output.push_str(&format!(" - {}", desc));
                }
            }
            output.push_str("\n");
            continue;
        }

        // Create tree-like prefix based on depth
        let prefix = format!("{}", "  ".repeat(dir_info.depth - 1));
        let tree_char = "├─ ";

        // Extract folder name from path
        let folder_name = std::path::Path::new(&dir_info.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&dir_info.path);

        // Format directory entry
        if let Some(yaml) = &dir_info.yaml_matter {
            // Directory with metadata
            let name = yaml
                .get("name")
                .and_then(|v| match v {
                    serde_yaml::Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or(folder_name);

            let description = yaml.get("description").and_then(|v| match v {
                serde_yaml::Value::String(s) => Some(s.as_str()),
                _ => None,
            });

            output.push_str(&format!("{}{}`{}`", prefix, tree_char, name));

            if let Some(desc) = description {
                output.push_str(&format!(" - {}", desc));
            }

            output.push_str("\n");

            // Add other metadata if present
            let mut other_fields = Vec::new();
            for (key, value) in yaml {
                if key != "name" && key != "description" {
                    let value_str = match value {
                        serde_yaml::Value::String(s) => s.clone(),
                        serde_yaml::Value::Number(n) => n.to_string(),
                        serde_yaml::Value::Bool(b) => b.to_string(),
                        serde_yaml::Value::Sequence(seq) => seq
                            .iter()
                            .filter_map(|v| match v {
                                serde_yaml::Value::String(s) => Some(s.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(", "),
                        _ => format!("{:?}", value),
                    };
                    other_fields.push(format!(
                        "    {}{}- {}: {}",
                        prefix, tree_char, key, value_str
                    ));
                }
            }
            if !other_fields.is_empty() {
                for field in other_fields {
                    output.push_str(&format!("{}\n", field));
                }
            }
        } else {
            // Directory without metadata
            output.push_str(&format!("{}{}`{}`\n", prefix, tree_char, folder_name));
        }
    }

    output
}
