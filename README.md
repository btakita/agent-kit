# agent-kit

Toolkit for CLI tools integrating with AI agent loops.

`agent-kit` provides shared infrastructure so CLI tools can install skill definitions, detect agent environments, coordinate across sessions, and integrate cleanly with any AI coding assistant — Claude Code, Codex, OpenCode, Pi, Grok, or plain API calls.

## Features

- **Skill Management** — Install, check, and uninstall SKILL.md files for agent environments
- **Environment Detection** — Auto-detect Claude Code, OpenCode, Codex, or generic environments
- **Hook System** (`hooks` feature) — File-based event coordination between multiple agent sessions
  - `HookRegistry` — fire/poll/gc events in `.agent-doc/hooks/` directories
  - `HookTransport` trait — abstract delivery (file, Unix socket, chain)
  - `FileTransport` — JSON file events, always available
  - `SocketTransport` — Unix domain socket delivery with ack protocol
  - `ChainTransport` — try transports in order until one succeeds
- **Instruction Audit** (`audit` feature) — Validate instruction files via `instruction-files` crate

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
agent-kit = "0.4"
# Optional features:
# agent-kit = { version = "0.4", features = ["hooks"] }
# agent-kit = { version = "0.4", features = ["audit"] }
```

### Skill Management

Bundle a `SKILL.md` in your crate and use `SkillConfig` to manage installation:

```rust
use agent_kit::skill::SkillConfig;

const BUNDLED_SKILL: &str = include_str!("../SKILL.md");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> anyhow::Result<()> {
    let config = SkillConfig::new("my-tool", BUNDLED_SKILL, VERSION);

    // Install to .claude/skills/my-tool/SKILL.md
    config.install(None)?;

    // Check if installed version matches bundled version
    let up_to_date = config.check(None)?;

    // Remove installed skill
    config.uninstall(None)?;

    Ok(())
}
```

### Hook System

Coordinate multiple agent sessions via file-based events:

```rust
use agent_kit::hooks::{HookRegistry, Event, FileTransport, SocketTransport, ChainTransport, HookTransport};

let registry = HookRegistry::new(".agent-doc/hooks");

// Fire an event
registry.fire("post_write", Event {
    file: "doc.md".into(),
    session_id: "abc123".into(),
    data: serde_json::json!({"patches": 3}),
})?;

// Poll for events
let events = registry.poll("post_write", last_poll_timestamp)?;

// Deliver via transport chain (socket first, file fallback)
let transport = ChainTransport::new(vec![
    Box::new(SocketTransport::from_project_root(Path::new("."))),
    Box::new(FileTransport::new(".agent-doc/hooks")),
]);
registry.fire_and_deliver("post_write", event, &transport, &target_sessions)?;
```

## Roadmap

- Structured output for agents (`--agent-output` flag support)
- Context injection (CLAUDE.md / AGENTS.md management)
- MCP tool description generation
- MCP transport for hook delivery

## License

MIT
