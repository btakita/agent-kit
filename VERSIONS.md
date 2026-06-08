# Versions

## 0.4.1

- Restore the `skill` module export for compatibility with callers using `agent_kit::skill::SkillConfig`
- Keep recursive instruction-file discovery from descending into skipped directories such as `node_modules`, `.venv`, `target`, and `vendor`
- Remove the now-unused direct `glob` dependency

## 0.4.0

- `find_instruction_files()` now prunes `skip_dirs` during recursive discovery, so `audit-docs` no longer walks vendored/cache trees like `node_modules`, `.venv`, `target`, or `vendor` before filtering results
- `audit_common` — shared audit primitives (moved from `instruction-files`)
- `Environment` path methods: `rules_dir()`, `runbooks_dir()`, `memories_dir()`, `skills_dir()`
- Add Spec/Contracts/Evals convention
- Remove `skill` module (moved to `agent-skills` crate)
- Windows compatibility: gate `SocketTransport` behind `#[cfg(unix)]`

## 0.3.1

- Multi-harness `skill install` support
- Cursor IDE support

## 0.3.0 (2026-04-01)

- `hooks` feature — file-based event coordination between agent sessions
  - `HookRegistry` — fire/poll/gc events with TTL-based expiry
  - `HookTransport` trait — abstract delivery mechanism
  - `FileTransport` — JSON file events (always available)
  - `SocketTransport` — Unix domain socket with ack protocol
  - `ChainTransport` — composable transport fallback chain
  - `fire_and_deliver` — fire locally + deliver to target sessions
  - 10 unit tests (fire/poll round trip, TTL expiry, transport chain, delivery)

## 0.2.0 (2026-03-09)

- `detect::Environment` — auto-detect agent environment (Claude Code, OpenCode, Codex, Generic)
- `SkillConfig` uses detected environment for path resolution
- `audit` feature — re-exports `instruction-files` crate for instruction file auditing
- 21 unit tests

## 0.1.0 (2026-03-09)

Initial release.

- `SkillConfig` — install, check, uninstall SKILL.md files
- Idempotent installation with content comparison
- Targets Claude Code `.claude/skills/<name>/SKILL.md` layout
- 10 unit tests
