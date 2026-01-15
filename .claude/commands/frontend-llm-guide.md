---
description: Apply the frontend-llm-guide rules to generate or refactor frontend code with strict constraints.
This command enforces consistency, prevents design drift, and minimizes output noise.
---

You are executing the command: frontend-llm-guide.

Apply the rules defined in `frontend-llm-guide-quick-rules` as HARD CONSTRAINTS.

Treat all MUST rules as non-negotiable.
Treat SHOULD rules as defaults unless explicitly overridden.
Never violate DO NOT rules.

Architecture rules:
- Use React + Tailwind v4 (CSS-first).
- Use shadcn/ui components instead of raw HTML elements.
- Use lucide-react for all icons.
- Use CSS variables with opacity for all colors.

Design rules:
- Composition-first components.
- No hardcoded colors, spacing, or layout assumptions.
- UI components must be stateless or thin; keep domain logic outside.

Behavior rules:
- If the receiver (frontend / client) is implied slow, design scrollable or chunked UI.
- Prefer predictable, boring UI over fancy visuals.

Output rules:
- Output only the minimal required code:
  - diffs
  - new components
  - modified components
- Do not restate or explain the rules.
- Do not explain basic frontend concepts.
- Assume Tailwind and shadcn/ui are already configured.
- Do not generate project boilerplate unless explicitly requested.

If information is missing:
- Make reasonable assumptions.
- Proceed without asking clarification unless ambiguity blocks correctness.

Failure handling:
- If a requirement conflicts with the rules, prioritize the rules.
- If compliance is impossible, state the conflict briefly and stop.
