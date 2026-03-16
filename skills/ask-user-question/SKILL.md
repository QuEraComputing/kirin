---
name: ask-user-question
description: Run a structured interview using the AskUserQuestion tool to gather requirements one question at a time. Use when the user says "interview me", "ask me questions about", "use the ask user skill", "use your interview skill", or when requirements are ambiguous and need a guided, option-based questionnaire before proceeding.
---

# Ask User Question

## Overview

Use a structured interview to collect missing context before planning or executing work. Ask informed, option-based questions with clear tradeoffs, and wait for answers before proceeding.

## Interview Workflow

1. Explore before asking.
   - Inspect the codebase and existing patterns.
   - Note constraints, conventions, and likely decision points.
2. Identify critical decisions.
   - Focus on choices that impact architecture, scope, or rework risk.
   - Skip implementation details until direction is clear.
3. Design structured questions.
   - Ask one question at a time by default.
   - Provide 2-4 mutually exclusive options with tradeoffs.
   - Lead with a recommended option based on discovery.
4. Ask and wait.
   - Use the AskUserQuestion tool if available.
   - Do not proceed until the user answers or declines.
5. Iterate.
   - If answers create new unknowns, ask the next question.
   - When clarity is sufficient, move to planning and execution.

## Question Design Rules

- Keep question headers at 12 characters or less.
- use a small example to explain the question if needed.
- Make options concrete and actionable, not open-ended.
- Explain why each option fits and its tradeoffs.
- Ground every question in discovery.
- Prefer single-question turns unless batching is required.

## AskUserQuestion Tool Usage

Use this structure:

Question:

- `text`: specific question with context
- `header`: short tag, max 12 characters
- `options`: 2-4 items, each with `label` and `description` (tradeoff)
- `multiSelect`: `false` unless multiple options can legitimately apply

If the tool is unavailable, present the same structure in markdown and ask the user to reply with the option label or "Other".

## Example Prompts That Trigger This Skill

- "Interview me about this project."
- "Ask me questions about the requirements."
- "Use the ask user skill."
- "Use your interview skill."
