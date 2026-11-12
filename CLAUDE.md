# Project Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:1105d646 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->


## Build & Test

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features                      # also with --release (AC-19)
cargo build -p estado-core --no-default-features --target thumbv7em-none-eabihf   # AC-16
cargo +nightly miri test -p estado-core                    # AC-18
```

## Architecture Overview

Cargo workspace, crates in `crates/<name>`. `estado-core` is the only required crate and builds as `no_std` without `alloc`. The normative spec is `docs/spec.md`; section 5 (semantics) is the source of truth. Work follows the milestones of spec section 12 (M0-M6), tracked in Beads: epic `estado-99i`, milestones `estado-99i.1`..`estado-99i.7`.

## Conventions & Patterns

- **English everywhere:** code, comments, docs, commit messages, diagnostics.
- **Descriptive names:** full words, no single-letter identifiers outside tiny closures.
- **Small modules:** `lib.rs` mostly re-exports; split a file before it grows past ~400 lines.
- **Decisions:** every [OPEN] item from the spec gets an ADR in `docs/decisions/NNNN-title.md`; contradictions with section 5 go to `docs/spec-errata.md`.
- **Acceptance criteria:** the test name carries the id (`ac_01_...`, `bm_02_...`).
- **Commits:** small, focused, short imperative message, no AI attribution. Identity is repo-local (`M4nd0r4s`, GitHub noreply address). Push only when asked.
- **Beads:** search before creating a bead; claim before coding; close when the commit lands; review the epic and milestones at the end of each delivery.
