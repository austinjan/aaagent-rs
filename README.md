---
description: Some tools for this KM, missing-readme reports folder without README.md, generating-map create km map.
---

# km-tools

CLI tools and LLM provider library for the km project.

## Build

```bash
cargo build --release
```
The binary will be at `target/release/km-tools`.

## Usage

### CLI Tools

```bash
km-tools --help
km-tools missing-readme --path .
km-tools generate-map --path . --format tree
```
