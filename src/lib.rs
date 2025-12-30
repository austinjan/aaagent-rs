use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// Go though all children folders, execute F on each directory and collect results
//
pub fn walk_dir<T, F>(dir: &Path, f: &mut F) -> io::Result<Vec<T>>
where
    F: FnMut(&Path) -> io::Result<Option<T>>,
{
    let mut result = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(item) = f(&path)? {
                result.push(item);
            }
            result.extend(walk_dir(&path, f)?);
        }
    }
    Ok(result)
}

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

    fn walk_with_ignore<'a>(
        dir: &Path,
        gitignore: &'a Gitignore,
    ) -> io::Result<Vec<String>> {
        let mut result = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                // Check if path matches any ignore pattern
                if gitignore.matched(&path, true).is_ignore() {
                    continue;
                }

                if check_file_exist("README.md", &path)?.is_none() {
                    result.push(path.to_string_lossy().to_string());
                }
                result.extend(walk_with_ignore(&path, gitignore)?);
            }
        }
        Ok(result)
    }

    walk_with_ignore(dir, &gitignore)
}
