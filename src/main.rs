use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;

use km_tools::find_missing_readme;

const DEFAULT_IGNORE_PATTERNS: &[&str] = &[".*"];

fn read_gitignore_patterns(dir: &Path) -> Vec<String> {
    let mut patterns: Vec<String> = DEFAULT_IGNORE_PATTERNS.iter().map(|s| s.to_string()).collect();

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
    /// Generate a map of the project structure
    GenerateMap {
        /// The root directory to map
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Output format (text, json, tree)
        #[arg(short, long, default_value = "tree")]
        format: String,
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
        Commands::GenerateMap { path, format } => {
            println!("Generating map for: {} (format: {})", path, format);
            // TODO: Implement map generation logic
        }
    }
}
