# Execution Operations Reference

Read this file when entering Phase 4 (Guarded Execution). It covers agent coordination,
worktree merge procedures, failure recovery, and agent role details.

## Table of Contents

1. [Coordination Options](#coordination) — Agent Teams vs Background Agents
2. [Option A: Agent Teams](#option-a-agent-teams) — TeamCreate-based coordination
3. [Option B: Background Agents](#option-b-background-agents) — fallback with lead-as-integrator
4. [Lead Orchestration Loop](#lead-orchestration-loop) — merge cycle for Option B
5. [Worktree Branch Divergence](#worktree-branch-divergence) — handling parallel wave merges
6. [Agent Failure Recovery](#agent-failure-recovery) — auth expiry, incomplete agents
7. [Agent Role Details](#agent-role-details) — Verifier, Integrator, Lead, Guardian

---

## Coordination

Choose based on tool availability:

| Option | When | Who Merges |
|--------|------|------------|
| **A: Agent Teams** | `TeamCreate` available | Dedicated Integrator agent |
| **B: Background Agents** | `TeamCreate` not available | Lead agent directly |

**Non-blocking requirement (both options):** All implementer agents MUST run in background (`run_in_background: true`). The user must be able to interact with the main agent at all times during execution. Never block on agent completion.

---

## Option A: Agent Teams

Use `TeamCreate` to create a refactor team, then spawn implementer teammates. This gives structured coordination via shared task lists and message passing.

1. Create team: `TeamCreate(team_name: "refactor-<scope>", description: "Refactoring <scope>")`
2. Create tasks for each work stream using `TaskCreate` — one task per agent assignment, with dependencies reflecting the ordering
3. Spawn each code-editing teammate through the pre-dispatch gate (see Phase 4 in SKILL.md). Every call MUST include both `isolation: "worktree"` and `run_in_background: true`:
   ```
   Agent(team_name: "refactor-<scope>", name: "<role>-<crate>",
         isolation: "worktree", run_in_background: true, ...)
   ```
4. Spawn the Integrator (works on the dev branch, not a worktree — it is the sole writer to the dev branch)
5. Spawn the Verifier (read-only — no `isolation: "worktree"` needed)
6. Teammates pick up tasks from the shared list, work in isolated worktrees, mark tasks complete, and go idle
7. As implementers complete, the lead creates merge tasks for the Integrator specifying branch and merge order
8. After each merge, the Integrator notifies the Verifier; the Verifier checks and reports to lead
9. After all work is done, send shutdown messages to all teammates and call `TeamDelete`

---

## Option B: Background Agents

If `TeamCreate` is not available, dispatch agents through the pre-dispatch gate with `run_in_background: true` and `isolation: "worktree"`. Track progress through agent completion notifications. The lead performs integration directly (no Integrator agent).

### Lead Orchestration Loop

When using background agents, the lead runs this cycle after each agent completes:

1. **Check the worktree commit** — `git -C <worktree-path> log --oneline -3` to see what was committed
2. **Verify parent** — the commit's parent should match the current HEAD. If not, the worktree diverged (see below) and needs cherry-pick instead of fast-forward
3. **Merge** — `git merge --ff-only <commit>` if parent matches HEAD, or `git cherry-pick <commit>` if diverged
4. **Run workspace tests** — `cargo nextest run --workspace` to verify no regressions from the merge
5. **Clean up** — `git worktree remove <path>; git worktree prune`
6. **Dispatch next wave** — only after all agents in the current wave complete and merge successfully

Track progress with a status table showing wave, status, commit hash, and test count. This helps the user see where things stand at a glance.

### Worktree Branch Divergence

When dispatching parallel agents (e.g., Waves 3a and 3b), both worktrees start from the same HEAD. But if Wave 3a completes and is merged before 3b, then 3b's commit has a stale parent. This is expected and harmless — `git cherry-pick` handles it cleanly since the waves touch different files (file disjointness is enforced by planning). The lead should:
- Prefer `git merge --ff-only` when possible (parent matches HEAD)
- Fall back to `git cherry-pick` when the parent doesn't match (parallel wave divergence)
- Always run workspace tests after cherry-pick to confirm clean merge

### Agent Failure Recovery

Long-running agents may lose API authentication mid-execution. When an agent reports failure (e.g., "Not logged in"), check the worktree for commits before re-dispatching — the agent often completed its work and committed before the auth expired. Recovery steps:
1. `git -C <worktree-path> log --oneline -3` — check if a commit exists
2. If committed: merge the commit normally, skip re-dispatch
3. If no commit: re-dispatch the agent after the user re-authenticates

---

## Agent Role Details

### The Verifier Agent

**Always staff a Verifier** when using parallel agents (Option A). In Option B, the lead absorbs this role.

The Verifier:

1. **After each implementer completes a task**: reviews the changes in that agent's worktree, runs `cargo check` and relevant tests, reports findings to the lead
2. **After merging**: runs full workspace verification (`cargo build --workspace`, `cargo nextest run --workspace`, `cargo test --doc --workspace`)
3. **Reports issues to lead**: the lead decides whether to fix now or defer, and assigns fixes to an idle implementer

The Verifier does NOT fix code — it only checks and reports. This separation prevents the verifier from introducing its own bugs.

### The Integrator Agent

**Always staff an Integrator** when using agent teams (Option A). In Option B, the lead performs integration directly.

Spawn with `run_in_background: true` but WITHOUT `isolation: "worktree"` — the Integrator is the sole writer to the dev branch and needs direct access to it.

The Integrator:

1. **Merges worktree branches** in dependency order as implementers complete their tasks. Does not wait for all implementers to finish — merges incrementally as work streams complete.
2. **Resolves merge conflicts** — understands the refactor plan and makes informed decisions about which side to keep. Reports ambiguous conflicts to the lead.
3. **Polishes the merged result:**
   - Fixes import ordering and dead imports left by the merge
   - Ensures consistent naming conventions across merged code from different agents
   - Removes leftover TODO/FIXME markers that agents resolved in their own branches but didn't clean up in others'
   - Runs `cargo fmt --all` after each merge
4. **Cleans up worktree artifacts** — removes merged worktree branches, prunes stale references
5. **Hands off to Verifier** — after each merge, notifies the Verifier to run checks on the merged state

**The Integrator works on the dev branch directly** (not in a worktree) since it is the only agent writing to it. No other agent edits the dev branch.

**Relationship to other roles:**
- Implementers/Builders/Migrators → produce branches in worktrees
- Integrator → merges those branches, resolves conflicts, polishes
- Verifier → checks the merged result, reports issues back to lead
- Lead → assigns merge order, triages Verifier findings

### Lead Agent Responsibilities

The lead agent (you) orchestrates:

1. **Task assignment**: create tasks, assign to agents, track dependencies
2. **Merge ordering**: tell the Integrator which branches to merge and in what order (Option A), or merge directly (Option B)
3. **Issue triage**: when the Verifier reports problems (or when workspace tests fail after merge), decide:
   - Which agent should fix it (prefer an idle agent with context, or the Integrator for polish issues)
   - Whether to fix now or batch fixes after all agents complete
4. **Unblocking**: if the Integrator hits an ambiguous conflict, make the call

### Guardian Role

Read the Guardian persona (from the team directory, see AGENTS.md). The Guardian runs as the lead agent's advisor:
- Pre-flight analysis (Phase 1)
- Migration checklist production for Migrators
- Post-validation pub-item diffing (Phase 5)

For small refactors, the lead agent can absorb Guardian duties directly.
