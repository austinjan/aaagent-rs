---
name: code-review
description: Guide for performing thorough code reviews. Use when the user asks you to review code, PRs, or wants feedback on their implementation.
metadata:
  short-description: Code review checklist and guidelines
---

# Code Review Skill

You are now in code review mode. Apply the following checklist and best practices when reviewing code.

## Review Checklist

### 1. Correctness
- Does the code do what it's supposed to do?
- Are there any logic errors or edge cases not handled?
- Are error conditions properly handled?

### 2. Security
- Are there any security vulnerabilities (injection, XSS, etc.)?
- Is user input properly validated and sanitized?
- Are secrets/credentials properly protected?

### 3. Performance
- Are there any obvious performance issues?
- Are there unnecessary loops or redundant operations?
- Is memory usage reasonable?

### 4. Readability
- Is the code easy to understand?
- Are variable and function names descriptive?
- Is the code properly formatted and consistent?

### 5. Maintainability
- Is the code modular and well-organized?
- Are there adequate comments for complex logic?
- Does it follow the project's coding standards?

### 6. Testing
- Are there adequate tests?
- Do the tests cover edge cases?
- Are the tests readable and maintainable?

## Response Format

Structure your review as follows:

```
## Summary
[One paragraph overview]

## Issues Found
- **[Critical/Major/Minor]**: [Description]

## Suggestions
- [Improvement suggestions]

## What's Good
- [Positive aspects of the code]
```

Be constructive and specific in your feedback.
