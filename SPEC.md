# agent-kit Spec

Shared infrastructure for CLI tools integrating with AI agent loops.

## Environment Detection

`detect::Environment` auto-detects which AI coding assistant is active by checking environment variables.

### Environment Enum

| Variant | Detection |
|---------|-----------|
| `ClaudeCode` | `CLAUDE_CODE` or `CLAUDE_CODE_ENTRYPOINT` set |
| `OpenCode` | `OPENCODE` set |
| `Codex` | `CODEX_CLI` or `CODEX` set |
| `Cursor` | `CURSOR_SESSION_ID` or `CURSOR` set |
| `Generic` | Fallback when no environment variable matches |

Priority order: ClaudeCode > OpenCode > Codex > Cursor > Generic.

### Path Methods

Each environment resolves agent file paths differently:

| Method | Purpose |
|--------|---------|
| `skill_rel_path(name)` | Relative path to install a skill file |
| `skill_path(name, root)` | Absolute skill path under a project root |
| `rules_dir()` | Rules/instruction file directory |
| `runbooks_dir()` | Runbooks directory (`.agent/runbooks/` universally) |
| `memories_dir()` | Memories directory (`.agent/memories/` universally) |
| `skills_dir()` | Skills directory (environment-specific) |

Skill path layout per environment:

| Environment | `skill_rel_path("tool")` |
|-------------|--------------------------|
| ClaudeCode | `.claude/skills/tool/SKILL.md` |
| OpenCode | `.opencode/skills/tool/SKILL.md` |
| Codex | `.codex/AGENTS.md` |
| Cursor | `.cursor/rules/tool.md` |
| Generic | `AGENTS.md` |

## Audit Primitives

Shared validation checks for agent instruction files. Available via `audit_common` (always) and `audit` feature (re-exports `instruction-files`).

### AuditConfig

Preset configurations control discovery scope and validation behavior:

| Preset | Root markers | CLAUDE.md | Source extensions | Source dirs |
|--------|-------------|-----------|-------------------|-------------|
| `agent_doc()` | Cargo.toml, package.json, pyproject.toml, ... (12 markers) | included | rs, ts, py, go, ... (26 extensions) | src, lib, app, pkg, cmd, internal |
| `corky()` | Cargo.toml | excluded | rs | src |

### Checks

| Check | What it validates |
|-------|-------------------|
| `check_context_invariant` | Flags machine-local paths (`~/`, `/home/user/`, `/Users/user/`, `/tmp/`, `C:\Users\`) in agent files. Skips code fences. |
| `check_staleness` | Compares instruction file mtime against newest source file mtime |
| `check_line_budget` | Combined line count of agent files against 1000-line budget. SPEC.md and README.md listed but excluded from budget. |

### Discovery

| Function | Purpose |
|----------|---------|
| `find_root(config)` | Walk up from CWD looking for root markers, then `.git`, then fall back to CWD |
| `find_instruction_files(root, config)` | Discover AGENTS.md, SKILL.md, CLAUDE.md, runbooks, SPEC.md, README.md; recursive walks prune `config.skip_dirs` before descending |

## Hook System

File-based event coordination between multiple agent sessions. Behind the `hooks` feature flag.

### HookRegistry

Central API for fire/poll/gc operations. Events are JSON files in `.agent-doc/hooks/<event_name>/` directories.

- `fire(name, event)` -- write event file, return event_id
- `poll(name, since_secs)` -- read new events since timestamp, clean expired
- `list_hooks()` -- list hook names with events
- `gc()` -- clean expired events across all hooks
- `fire_and_deliver(name, event, transport, targets)` -- fire locally and deliver to sessions

Default TTL: 60 seconds. Configurable via `with_ttl`.

### HookTransport Trait

Abstract delivery mechanism for routing events to target sessions.

| Transport | Description |
|-----------|-------------|
| `FileTransport` | Writes JSON to a watched directory. Always available. |
| `SocketTransport` | Unix domain socket with ack protocol. Low latency. Unix-only. |
| `ChainTransport` | Tries transports in order until one succeeds. |

### Event Types

- `pre_write` -- before writing to a document
- `post_write` -- after writing (notify linked documents)
- `post_commit` -- after committing (notify dependents)
- `claim` -- when a file is claimed by a session
- `layout_change` -- when tmux layout changes

## Agentic Contracts

Promises that agents using agent-kit must uphold.

### Environment Detection

When using agent-kit for environment detection, the agent promises to:
- Detect the environment once at session start and use consistent paths throughout the session
- Never mix path conventions from different environments in a single session
- Fall back to `Generic` gracefully when no environment is detected

### Audit Primitives

When using audit primitives, the agent promises to:
- Report all issues found without silently suppressing any
- Respect the line budget (1000 lines for agent files)
- Flag machine-local paths and recommend repo-relative alternatives
- Use the appropriate AuditConfig preset for the project type
- Never count reference docs (SPEC.md, README.md) toward the line budget

### Hook System

When using hooks, the agent promises to:
- Fire events atomically (one JSON file per event, no partial writes)
- Poll with appropriate TTL to avoid stale event accumulation
- Clean up expired events during poll (handled automatically by `poll()`)
- Never delete another session's unexpired events
- Use `ChainTransport` for reliable delivery (socket with file fallback)
