---
name: feature-planning
description: Creates or updates feature implementation plan documents in doc/plan. Use when planning a new software feature, refining feature scope/name, tracking TODO/DONE progress, or marking a plan as achieved.
---

# Feature Planning

Plan file path: `doc/plan/{feature-name}-plan.md`  
Example: feature "storage interface" → `doc/plan/storage-interface-plan.md`

## Rules

- If `{feature-name}` is missing or too vague, propose 2–5 better names (kebab-case) and pick a default.
- Always use forward slashes in paths.
- The plan MUST include:
  - feature overview + scope
  - references (links/issues/docs)
  - TODO (unfinished steps)
  - DONE (completed steps)
- If the plan is fully completed, move it to: `doc/plan/achieved/{feature-name}-plan.md`
  - Leave a short pointer note in the original location (or replace content with a redirect note), depending on repo convention.

## Workflow

1. **Resolve feature name**
   - If user provided a name: normalize to kebab-case.
   - If name is ambiguous (e.g., "UI", "storage", "fix bug"): suggest better names and choose one.

2. **Locate plan**
   - Check whether `doc/plan/{feature-name}-plan.md` exists.
   - If not exists: create it from `plan-template.md` (or inline template if template file not available).
   - If exists: update it (append new info, re-scope, move items between TODO/DONE, keep changelog).

3. **Update content**
   - Fill/refresh: Goals, Non-goals, Requirements, Design, Milestones, Tasks.
   - Capture dependencies, risks, and acceptance criteria.
   - Maintain TODO/DONE lists with checkboxes.

4. **Achieve**
   - If TODO is empty and acceptance criteria are met:
     - Move plan file to `doc/plan/achieved/` wiht python script `archive_plan.py`
     ```
     python archive_plan.py {feature-name}
     ```
     - Add a completion stamp in the plan (date + summary)

## When to use this skill

- User asks to implement a new feature, plan a feature, or refine a feature scope.
- User asks to review progress of a feature.
- User asks to update/adjust an existing feature plan.
- User says a plan is done and should be archived as achieved.
