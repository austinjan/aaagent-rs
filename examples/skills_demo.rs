//! Skills System Demo
//!
//! This example demonstrates how to use the skills system in aaagent-rs.
//!
//! Skills are markdown files that provide specialized knowledge and workflows
//! to the LLM. They are loaded from:
//! - `.aaagent/skills/` in your project directory
//! - `~/.aaagent/skills/` for user-level skills
//!
//! Run with:
//! ```
//! cargo run --example skills_demo
//! ```

use aaagent::skills::{
    parse_skill_references, render_skills_section, SkillReference, SkillsManager,
};
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary skills directory for demo
    let temp_dir = tempfile::tempdir()?;
    let skills_dir = temp_dir.path().join("skills");
    fs::create_dir_all(&skills_dir)?;

    // Create a sample skill
    create_sample_skill(
        &skills_dir,
        "code-review",
        r#"---
name: code-review
description: Guide for performing thorough code reviews. Use when reviewing pull requests or code changes.
metadata:
  short-description: Code review guidelines
---

# Code Review Skill

## When to Use
Use this skill when reviewing code changes, pull requests, or performing code audits.

## Review Checklist

1. **Correctness**: Does the code do what it's supposed to do?
2. **Security**: Are there any security vulnerabilities?
3. **Performance**: Are there any performance issues?
4. **Readability**: Is the code easy to understand?
5. **Testing**: Are there adequate tests?

## Best Practices

- Be constructive and respectful
- Focus on the code, not the person
- Suggest improvements, don't just criticize
- Acknowledge good solutions
"#,
    )?;

    create_sample_skill(
        &skills_dir,
        "rust-patterns",
        r#"---
name: rust-patterns
description: Common Rust patterns and idioms. Use when writing or reviewing Rust code.
metadata:
  short-description: Rust best practices
---

# Rust Patterns Skill

## Error Handling

Use `Result<T, E>` for recoverable errors and `panic!` only for unrecoverable situations.

```rust
fn parse_config(path: &str) -> Result<Config, ConfigError> {
    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(ConfigError::Parse)
}
```

## Builder Pattern

```rust
struct Request {
    url: String,
    method: Method,
    headers: HashMap<String, String>,
}

impl Request {
    fn builder() -> RequestBuilder {
        RequestBuilder::default()
    }
}
```
"#,
    )?;

    // Create the skills manager
    let manager = SkillsManager::new(temp_dir.path().to_path_buf());

    // Load skills for current directory
    let cwd = std::env::current_dir()?;
    let outcome = manager.skills_for_cwd(&cwd);

    println!("=== Skills Demo ===\n");

    // Show loaded skills
    println!("Loaded {} skills:", outcome.skills.len());
    for skill in &outcome.skills {
        println!(
            "  - {} ({}): {}",
            skill.name,
            skill.scope,
            skill.display_description()
        );
    }
    println!();

    // Show any errors
    if !outcome.errors.is_empty() {
        println!("Errors:");
        for error in &outcome.errors {
            println!("  - {}", error);
        }
        println!();
    }

    // Demo: Parse skill references from text
    let user_input = "Please /skill:code-review this PR and apply /skill:rust-patterns";
    println!("User input: {}", user_input);

    let refs = parse_skill_references(user_input);
    println!("Parsed skill references:");
    for r in &refs {
        println!("  - {}", r.name);
    }
    println!();

    // Demo: Manual skill reference
    let manual_refs = vec![
        SkillReference::by_name("code-review"),
        SkillReference::by_name("rust-patterns"),
    ];

    println!(
        "Building injections for skills: {:?}",
        manual_refs.iter().map(|r| &r.name).collect::<Vec<_>>()
    );

    let injections = aaagent::build_skill_injections(&manual_refs, Some(&outcome));

    println!("Injected {} skills", injections.items.len());
    for item in &injections.items {
        println!("\n--- Skill: {} ---", item.name);
        // Show first 200 chars of content
        let preview: String = item.contents.chars().take(200).collect();
        println!("{}...", preview);
    }

    if !injections.warnings.is_empty() {
        println!("\nWarnings:");
        for w in &injections.warnings {
            println!("  - {}", w);
        }
    }

    // Demo: Render skills section for documentation
    println!("\n=== Skills Documentation ===\n");
    if let Some(section) = render_skills_section(&outcome.skills) {
        println!("{}", section);
    }

    println!("\n=== Demo Complete ===");

    Ok(())
}

fn create_sample_skill(
    skills_dir: &PathBuf,
    name: &str,
    content: &str,
) -> Result<(), std::io::Error> {
    let skill_dir = skills_dir.join(name);
    fs::create_dir_all(&skill_dir)?;
    fs::write(skill_dir.join("SKILL.md"), content)?;
    Ok(())
}
