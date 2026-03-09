# agent-kit

Toolkit for CLI tools integrating with AI agent loops.

`agent-kit` provides shared infrastructure so CLI tools can install skill definitions, detect agent environments, and integrate cleanly with any AI coding assistant — Claude Code, Codex, OpenCode, Pi, Grok, or plain API calls.

## Features

- **Skill Management** — Install, check, and uninstall SKILL.md files for agent environments
- **Environment-Aware Placement** — Currently targets Claude Code (`.claude/skills/<name>/SKILL.md`); more adapters planned

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
agent-kit = "0.1"
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

## Roadmap

- Environment detection (Claude Code, OpenCode, generic)
- Structured output for agents (`--agent-output` flag support)
- Context injection (CLAUDE.md / AGENTS.md management)
- MCP tool description generation

## License

MIT
