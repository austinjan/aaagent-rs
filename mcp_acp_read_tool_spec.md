# mcp__acp__Read Tool Specification

This is the complete specification of the `mcp__acp__Read` tool as exposed to Claude Code:

## Function Name
`mcp__acp__Read`

## Description
Reads the content of the given file in the project.

In sessions with mcp__acp__Read always use it instead of Read as it contains the most up-to-date contents.

Reads a file from the local filesystem. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

## Usage Notes
- The file_path parameter must be an absolute path, not a relative path
- By default, it reads up to 2000 lines starting from the beginning of the file
- You can optionally specify a line offset and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters
- Any files larger than 50000 bytes will be truncated
- This tool allows Claude Code to read images (eg PNG, JPG, etc). When reading an image file the contents are presented visually as Claude Code is a multimodal LLM.
- This tool can only read files, not directories. To read a directory, use an ls command via the mcp__acp__Bash tool.
- You have the capability to call multiple tools in a single response. It is always better to speculatively read multiple files as a batch that are potentially useful.

## Parameters (JSONSchema)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "properties": {
    "file_path": {
      "description": "The absolute path to the file to read",
      "type": "string"
    },
    "limit": {
      "default": 2000,
      "description": "The number of lines to read. Only provide if the file is too large to read at once.",
      "type": "number"
    },
    "offset": {
      "default": 1,
      "description": "The line number to start reading from. Only provide if the file is too large to read at once",
      "type": "number"
    }
  },
  "required": ["file_path"],
  "type": "object"
}
```

## Required Parameters
- `file_path` (string): The absolute path to the file to read

## Optional Parameters
- `limit` (number): The number of lines to read. Default: 2000
- `offset` (number): The line number to start reading from. Default: 1

## Key Constraints
1. File path MUST be absolute, not relative
2. Maximum 50000 bytes will be read (larger files are truncated)
3. Default line limit is 2000 lines
4. Can read images (PNG, JPG, etc.) and present them visually
5. Cannot read directories (use bash ls instead)
6. Supports batch reading (multiple files in one response)

## When to Use
- Reading source code files
- Reading configuration files
- Reading documentation
- Reading images
- Inspecting file contents before editing

## When NOT to Use
- Reading directories (use `mcp__acp__Bash` with ls)
- When you need to search content across files (use `Grep`)
- When you need to find files by pattern (use `Glob`)
