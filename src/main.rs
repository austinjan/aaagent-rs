use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;

use km_tools::{find_missing_readme, format_map_as_markdown, generate_map};

const DEFAULT_IGNORE_PATTERNS: &[&str] = &[".*", "node_modules"];

fn read_gitignore_patterns(dir: &Path) -> Vec<String> {
    let mut patterns: Vec<String> = DEFAULT_IGNORE_PATTERNS
        .iter()
        .map(|s| s.to_string())
        .collect();

    let gitignore_path = dir.join(".gitignore");
    if let Ok(content) = fs::read_to_string(&gitignore_path) {
        patterns.extend(
            content
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
                .map(|s| s.to_string()),
        );
    }

    patterns
}

#[derive(Parser)]
#[command(name = "km-tools")]
#[command(author, version, about = "A collection of CLI tools for km project", long_about = None)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check for missing README files in directories
    MissingReadme {
        /// The directory to scan
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Output as JSON
        #[arg(long, conflicts_with = "mk")]
        json: bool,

        /// Output as Markdown
        #[arg(long, conflicts_with = "json")]
        mk: bool,
    },
    /// Generate a hierarchical map of directories and their contents, enabling LLMs to progressively focus on relevant areas.
    GenerateMap {
        /// The root directory to map
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Maximum depth to traverse (0 = unlimited)
        #[arg(short, long, default_value = "3")]
        depth: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        println!("Verbose mode enabled");
    }

    match cli.command {
        Commands::MissingReadme { path, json, mk } => {
            let dir = Path::new(&path);
            let patterns = read_gitignore_patterns(dir);
            let pattern_refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
            match find_missing_readme(dir, &pattern_refs) {
                Ok(missing) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&missing).unwrap());
                    } else if mk {
                        println!("# Directories missing README.md\n");
                        for p in &missing {
                            println!("- `{}`", p);
                        }
                    } else {
                        for p in &missing {
                            println!("{}", p);
                        }
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Commands::GenerateMap { path, depth, json } => {
            let dir = Path::new(&path);
            let patterns = read_gitignore_patterns(dir);
            let pattern_refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();

            match generate_map(dir, &pattern_refs, depth) {
                Ok(map) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&map).unwrap());
                    } else {
                        // Output as markdown for LLM consumption
                        let markdown = format_map_as_markdown(&map);
                        println!("{}", markdown);
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }
}
