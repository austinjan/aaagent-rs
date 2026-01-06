---
name: managing-feature-plans
description: Creates or updates feature implementation plan documents in doc/plan. Use when planning a new software feature, refining feature scope/name, tracking TODO/DONE progress, or marking a plan as achieved.
---

## When to use this skill

- User asks to **implement a new feature**, plan a feature, or refine a feature scope.
- User asks to **review progress of a feature**.
- User asks to **review an existing feature plan's requirements, references, and TODOs**.
- User says a plan is done and should be **archived as achieved**.

This skill is idea for:
- Creating/Review feature plans as part of development process.
- Establish trackable, meaningful feature progress and documentation.
- Automating the **feature development process**.

## Key Features
1. Feature Plan Creation
- Creates properly formatted feature plan documents in `doc/plan/{feature-name}-plan.md` see template file `plan-template.md`
- Writes Plan content to file path: `doc/plan/{feature-name}-plan.md`
- Enforces naming conventions, Plan file name format: `{feature-name}-plan.md` example `storage-interface-plan.md`

2. Review Feature Plan
- Confirms incomplete items, missing references, and TODOs.
- Modifies requirements and implementation tasks, TODOs.
- Updates existing feature plan documents with new information.

3. Feature Plan Archiving
- Verifies feature plan completeness.
- Archives completed feature plans, move the plan to `doc/plan/archived` folder example: `doc/plan/archived/{feature-name}-plan.md`.





## How it Works

1. **Resolve feature name**
   - If user provided a name: normalize to kebab-case.
   - If name is ambiguous (e.g., "UI", "storage", "fix bug"): suggest better names and choose one.

2. **Locate plan**
   - Check whether `doc/plan/{feature-name}-plan.md` exists.
   - If not exists: create it from `plan-template.md` (or inline template if template file not available).
   - If exists: update it (append new info, re-scope, move items between TODO/DONE, keep changelog).

3. **Review Content**
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

## Requirements
- Python 3.7+ for archive_plan.py script.
- Write access to skill creation plan, and archived folder.
- Write access to create and update feature plan documents.
- Read access to feature plan documents.
