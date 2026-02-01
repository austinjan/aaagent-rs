# Built-in Binaries

This document describes how to provide custom binaries to the agent that are available for shell execution and skill eligibility checks.

## Overview

The **Built-in Binaries** feature allows you to:

1. **Provide custom executables** in a `bin/` directory that the agent can use
2. **Make shell commands aware** of these binaries via PATH injection
3. **Enable skills** that depend on these binaries to pass eligibility checks

This is useful when you want to:
- Bundle custom CLI tools with your agent
- Provide platform-specific utilities
- Include tools that aren't installed system-wide

## Directory Structure

By default, place your binaries in a `bin/` directory:

```
project_root/
├── bin/                    # Built-in binaries directory
│   ├── my-tool             # Custom executable (Unix)
│   ├── my-tool.exe         # Custom executable (Windows)
│   └── helper-script       # Shell scripts work too
├── skills/
│   └── my-skill/
│       └── SKILL.md        # Can require "my-tool" binary
└── src/
```

### Binary Requirements

**Unix (Linux/macOS):**
- Files must have execute permission (`chmod +x`)
- File extension is optional

**Windows:**
- Files must have `.exe`, `.cmd`, `.bat`, or `.ps1` extension

## API Reference

### BuiltinBinaries

The main struct for managing built-in binaries.

#### Creating from Directory

```rust
use aaagent::tools::BuiltinBinaries;

// Scan bin/ directory for executables
let builtins = BuiltinBinaries::from_dir("bin/");

// Check what was found
println!("Found {} binaries", builtins.len());
for name in builtins.all() {
    println!("  - {}", name);
}
```

#### Creating with Explicit Names

```rust
// Useful for testing or when binaries are embedded
let builtins = BuiltinBinaries::with_names(
    "bin/",
    vec!["tool-a".to_string(), "tool-b".to_string()],
);
```

#### Creating Empty

```rust
// No built-in binaries (default behavior)
let builtins = BuiltinBinaries::empty();
```

#### Checking Binary Availability

```rust
// Check if a specific binary is registered
if builtins.has("my-tool") {
    println!("my-tool is available");
}

// Check if available (built-in OR system PATH)
use aaagent::tools::is_binary_available;

if is_binary_available("my-tool", &builtins) {
    println!("my-tool is available (built-in or system)");
}
```

#### Modifying at Runtime

```rust
let mut builtins = BuiltinBinaries::from_dir("bin/");

// Add a binary
builtins.register("new-tool");

// Remove a binary
builtins.unregister("old-tool");

// Rescan directory for changes
builtins.rescan();
```

### Integration with ShellTool

The `ShellTool` can inject the bin directory into PATH when executing commands.

```rust
use aaagent::tools::{ShellTool, BuiltinBinaries};

let builtins = BuiltinBinaries::from_dir("bin/");

// Method 1: Use with_builtin_binaries
let shell = ShellTool::new()
    .with_builtin_binaries(&builtins);

// Method 2: Use with_extra_path directly
let shell = ShellTool::new()
    .with_extra_path(std::path::PathBuf::from("bin/"));

// Now shell commands can use binaries from bin/
let result = shell.execute_command("my-tool --version").await;
```

**How it works:**
- The bin directory is prepended to the `PATH` environment variable
- Commands see: `PATH=bin/:$ORIGINAL_PATH`
- Built-in binaries take precedence over system binaries with the same name

### Integration with Skill Eligibility

Skills can declare binary requirements in their `SKILL.md` frontmatter:

```yaml
---
name: my-skill
description: A skill that uses my-tool
metadata:
  openclaw:
    requires:
      bins:
        - my-tool      # Required binary
        - other-tool
      anyBins:
        - tool-a       # At least one of these
        - tool-b
---
```

To make the eligibility check consider built-in binaries:

```rust
use aaagent::skills::{SkillsManager, SkillsConfig};
use aaagent::tools::BuiltinBinaries;

let builtins = BuiltinBinaries::from_dir("bin/");

// Create manager with built-in binaries
let manager = SkillsManager::with_config_and_builtins(
    &app_home,
    &cwd,
    SkillsConfig::default(),
    builtins,
);

// Skills requiring "my-tool" will now be eligible
// if my-tool exists in bin/
for skill in manager.skills() {
    println!("Eligible: {}", skill.name);
}
```

#### Lower-Level Eligibility Functions

```rust
use aaagent::skills::{
    check_eligibility_with_builtins,
    filter_eligible_with_config_and_builtins,
};
use aaagent::tools::BuiltinBinaries;

let builtins = BuiltinBinaries::from_dir("bin/");

// Check single skill
let result = check_eligibility_with_builtins(&skill, &builtins);

// Filter multiple skills
let (eligible, errors) = filter_eligible_with_config_and_builtins(
    skills,
    &config,
    &builtins,
);
```

## Complete Example

Here's a complete example showing all components working together:

```rust
use aaagent::skills::{SkillsManager, SkillsConfig};
use aaagent::tools::{BuiltinBinaries, ShellTool};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_home = PathBuf::from(std::env::var("HOME")?).join(".aaagent");
    let cwd = std::env::current_dir()?;

    // 1. Create built-in binaries registry
    let builtins = BuiltinBinaries::from_dir("bin/");
    println!("Loaded {} built-in binaries", builtins.len());

    // 2. Create SkillsManager with built-in binaries
    //    Skills requiring these binaries will now be eligible
    let manager = SkillsManager::with_config_and_builtins(
        &app_home,
        &cwd,
        SkillsConfig::default(),
        builtins.clone(),
    );

    println!("\nEligible skills:");
    for skill in manager.skills() {
        println!("  - {}", skill.name);
    }

    // 3. Create ShellTool with PATH injection
    //    Shell commands can now use built-in binaries
    let shell = ShellTool::new()
        .with_builtin_binaries(&builtins);

    // 4. Execute a command using a built-in binary
    match shell.execute_command("my-tool --help").await {
        Ok(output) => println!("\nmy-tool output:\n{}", output),
        Err(e) => println!("\nError: {}", e),
    }

    Ok(())
}
```

## Backwards Compatibility

All existing APIs continue to work without changes:

```rust
// These still work (use empty BuiltinBinaries internally)
let manager = SkillsManager::new(&app_home, &cwd);
let shell = ShellTool::new();
check_eligibility(&skill)?;
filter_eligible_with_config(skills, &config);
```

To use built-in binaries, use the new `*_with_builtins` variants or builders.

## Best Practices

1. **Use absolute paths** when creating `BuiltinBinaries` in production:
   ```rust
   let bin_dir = std::env::current_dir()?.join("bin");
   let builtins = BuiltinBinaries::from_dir(&bin_dir);
   ```

2. **Share the same `BuiltinBinaries` instance** between `SkillsManager` and `ShellTool` to ensure consistency.

3. **Use `rescan()`** if binaries are added/removed at runtime:
   ```rust
   builtins.rescan();
   manager.set_builtins(builtins);
   ```

4. **Check for errors** in skill loading to see which skills failed eligibility:
   ```rust
   for error in manager.errors() {
       log::warn!("Skill loading issue: {}", error);
   }
   ```

## Troubleshooting

### Binary not found

1. Check the file exists in the bin directory
2. Verify execute permissions (Unix): `chmod +x bin/my-tool`
3. Verify file extension (Windows): must be `.exe`, `.cmd`, `.bat`, or `.ps1`
4. Run `builtins.rescan()` if files were added after initialization

### Skill still not eligible

1. Check `manager.errors()` for the specific error message
2. Verify the binary name in `SKILL.md` matches exactly (case-sensitive)
3. Ensure you're using `with_config_and_builtins()` not just `with_config()`

### Shell command can't find binary

1. Verify you used `with_builtin_binaries()` or `with_extra_path()`
2. Check the PATH is being set: add debug logging
3. Ensure the binary is executable
