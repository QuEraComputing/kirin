# Rationalization Table

Common temptations and why they're wrong. Read this when you feel the urge to skip a step.

| Temptation | Rationalization | Reality |
|-----------|----------------|---------|
| Skip pre-flight | "This refactor is simple, I know what needs to move" | 'Simple' refactors have hidden consumers. Pre-flight takes 5 minutes; debugging a missed re-export takes 30. |
| Skip triage-review | "I already know the code well enough" / "I already know the problem areas" | You know the code. The Formalism reviewer catches abstraction issues. The Soundness Adversary catches invariant violations. Fresh eyes find what familiarity hides. User knowledge is input to the review, not a substitute. |
| Pre-flight before review on a broad target | "I'll figure out the scope first, then review" | Without review, pre-flight is guessing. For broad targets, you don't know what's wrong yet — the review discovers the refactoring scope. |
| Start coding before plan approval | "I'll adjust the plan based on what I find" | Code-first planning produces sunk-cost pressure to keep bad decisions. Plan approval costs 2 minutes; reworking a wrong approach costs hours. |
| Edit the same file from two agents | "The changes are in different functions" | Git merges on function granularity, not line granularity. Two agents touching the same file creates merge conflicts that require manual resolution. |
| Let the verifier fix issues | "It's faster than dispatching back to the implementer" | The verifier lacks the implementer's context. Verifier fixes introduce new bugs at a higher rate. Report to lead, let the right agent fix it. |
| Skip exploration budget | "I need to read one more file to understand" | The budget exists because unbounded exploration delays the actual work. If 20 reads aren't enough, the scope is wrong — simplify it. |
| Dispatch agents in foreground | "I need to wait for them anyway before merging" | Foreground dispatch blocks the user. Refactors take minutes per agent — the user should be free to ask questions or provide context while agents work in background. |
| Write plans directly instead of delegating | "It's faster than dispatching a whole planning team" | The lead agent's context is for orchestration, not codebase exploration. Writing plans directly pollutes it with details from 10+ files per finding, degrading orchestration quality for later phases. Per-finding isolation also prevents cross-contamination between plans. The planning team runs in background — the user isn't waiting. |
| Skip the Plan Reviewer | "The user already read the plans, they look fine" | Human review catches content issues. The Plan Reviewer catches structural issues: file overlaps between plans in the same wave, dependency cycles, missing findings. Skipping it risks merge conflicts during execution — far more expensive than the 30-second review. |
| Skip implementation notes | "The code is self-documenting" | Design gaps, workarounds, and wrong assumptions are invisible in the final code. An agent discovered `Vec<ResultValue>` isn't supported and silently used a single `ResultValue` — without notes, the next person to touch SCF won't know this was a constraint, not a choice. |
| Re-dispatch failed agent without checking worktree | "Agent failed, need to redo" | Long-running agents often complete their work before auth expires. The commit exists in the worktree — re-dispatching wastes 10+ minutes repeating work that's already done. Always `git log` the worktree first. |
