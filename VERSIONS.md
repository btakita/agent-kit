# Versions

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
