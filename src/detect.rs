//! Agent environment detection — identify which AI agent loop is active.
//!
//! Checks environment variables to determine the active agent environment,
//! then provides environment-specific paths and configuration.

use std::path::{Path, PathBuf};

/// The detected AI agent environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// Anthropic's Claude Code CLI.
    ClaudeCode,
    /// OpenCode CLI.
    OpenCode,
    /// OpenAI's Codex CLI.
    Codex,
    /// Cursor AI editor.
    Cursor,
    /// Unknown or generic environment.
    Generic,
}

impl Environment {
    /// Auto-detect the active agent environment from environment variables.
    pub fn detect() -> Self {
        detect_from(|key| std::env::var_os(key))
    }

    /// Return the skill file path pattern for this environment.
    ///
    /// Given a skill `name`, returns the relative path where the skill file
    /// should be installed. Each environment has its own convention:
    ///
    /// | Environment | Path |
    /// |-------------|------|
    /// | Claude Code | `.claude/skills/{name}/SKILL.md` |
    /// | OpenCode | `.opencode/skills/{name}/SKILL.md` |
    /// | Codex | `.codex/AGENTS.md` |
    /// | Cursor | `.cursor/rules/{name}.md` |
    /// | Generic | `AGENTS.md` |
    pub fn skill_rel_path(&self, name: &str) -> PathBuf {
        match self {
            Self::ClaudeCode => PathBuf::from(format!(".claude/skills/{name}/SKILL.md")),
            Self::OpenCode => PathBuf::from(format!(".opencode/skills/{name}/SKILL.md")),
            Self::Codex => PathBuf::from(".codex/AGENTS.md"),
            Self::Cursor => PathBuf::from(format!(".cursor/rules/{name}.md")),
            Self::Generic => PathBuf::from("AGENTS.md"),
        }
    }

    /// Return all skill file paths for installing across all environments.
    /// Used by `install --all` to write skill files for every supported harness.
    pub fn all_skill_rel_paths(name: &str) -> Vec<(Environment, PathBuf)> {
        vec![
            (Self::ClaudeCode, Self::ClaudeCode.skill_rel_path(name)),
            (Self::OpenCode, Self::OpenCode.skill_rel_path(name)),
            (Self::Codex, Self::Codex.skill_rel_path(name)),
            (Self::Cursor, Self::Cursor.skill_rel_path(name)),
        ]
    }

    /// Resolve the skill file path under a given root directory.
    ///
    /// When `root` is `None`, the returned path is relative to CWD.
    pub fn skill_path(&self, name: &str, root: Option<&Path>) -> PathBuf {
        let rel = self.skill_rel_path(name);
        match root {
            Some(r) => r.join(rel),
            None => rel,
        }
    }

    /// Return the rules/instruction file directory for this environment.
    ///
    /// | Environment | Directory |
    /// |-------------|-----------|
    /// | Claude Code | `.claude/` (CLAUDE.md lives at root) |
    /// | Cursor | `.cursor/rules/` |
    /// | Windsurf | `.windsurf/rules/` |
    /// | Others | `.agent/rules/` |
    pub fn rules_dir(&self) -> PathBuf {
        match self {
            Self::Cursor => PathBuf::from(".cursor/rules"),
            _ => PathBuf::from(".agent/rules"),
        }
    }

    /// Return the runbooks directory for this environment.
    ///
    /// Runbooks use `.agent/runbooks/` universally — they're tool-agnostic.
    pub fn runbooks_dir(&self) -> PathBuf {
        PathBuf::from(".agent/runbooks")
    }

    /// Return the memories directory for this environment.
    ///
    /// Memories use `.agent/memories/` universally — they're tool-agnostic.
    pub fn memories_dir(&self) -> PathBuf {
        PathBuf::from(".agent/memories")
    }

    /// Return the skills directory for this environment.
    ///
    /// | Environment | Directory |
    /// |-------------|-----------|
    /// | Claude Code | `.claude/skills/` |
    /// | OpenCode | `.opencode/skills/` |
    /// | Cursor | `.cursor/rules/` (skills map to rules) |
    /// | Others | `.agent/skills/` |
    pub fn skills_dir(&self) -> PathBuf {
        match self {
            Self::ClaudeCode => PathBuf::from(".claude/skills"),
            Self::OpenCode => PathBuf::from(".opencode/skills"),
            Self::Cursor => PathBuf::from(".cursor/rules"),
            _ => PathBuf::from(".agent/skills"),
        }
    }
}

impl Environment {
    /// Parse from a string name (for --harness flag).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "claude" | "claude-code" | "claudecode" => Some(Self::ClaudeCode),
            "opencode" | "open-code" => Some(Self::OpenCode),
            "codex" => Some(Self::Codex),
            "cursor" => Some(Self::Cursor),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClaudeCode => write!(f, "Claude Code"),
            Self::OpenCode => write!(f, "OpenCode"),
            Self::Codex => write!(f, "Codex"),
            Self::Cursor => write!(f, "Cursor"),
            Self::Generic => write!(f, "Generic"),
        }
    }
}

/// Auto-detect the active agent environment.
///
/// Convenience wrapper around [`Environment::detect`].
pub fn detect() -> Environment {
    Environment::detect()
}

/// Internal detection logic, parameterized for testability.
fn detect_from<F, V>(var: F) -> Environment
where
    F: Fn(&str) -> Option<V>,
    V: AsRef<std::ffi::OsStr>,
{
    if var("CLAUDE_CODE").is_some() || var("CLAUDE_CODE_ENTRYPOINT").is_some() {
        return Environment::ClaudeCode;
    }
    if var("OPENCODE").is_some() {
        return Environment::OpenCode;
    }
    if var("CODEX_CLI").is_some() || var("CODEX").is_some() {
        return Environment::Codex;
    }
    if var("CURSOR_SESSION_ID").is_some() || var("CURSOR").is_some() {
        return Environment::Cursor;
    }
    Environment::Generic
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::ffi::OsString;

    fn env_with(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    #[test]
    fn detects_claude_code_via_claude_code_var() {
        let detect = detect_from(env_with(&[("CLAUDE_CODE", "1")]));
        assert_eq!(detect, Environment::ClaudeCode);
    }

    #[test]
    fn detects_claude_code_via_entrypoint() {
        let detect = detect_from(env_with(&[("CLAUDE_CODE_ENTRYPOINT", "/usr/bin/claude")]));
        assert_eq!(detect, Environment::ClaudeCode);
    }

    #[test]
    fn detects_opencode() {
        let detect = detect_from(env_with(&[("OPENCODE", "1")]));
        assert_eq!(detect, Environment::OpenCode);
    }

    #[test]
    fn detects_codex_cli() {
        let detect = detect_from(env_with(&[("CODEX_CLI", "1")]));
        assert_eq!(detect, Environment::Codex);
    }

    #[test]
    fn detects_codex_var() {
        let detect = detect_from(env_with(&[("CODEX", "1")]));
        assert_eq!(detect, Environment::Codex);
    }

    #[test]
    fn falls_back_to_generic() {
        let detect = detect_from(env_with(&[]));
        assert_eq!(detect, Environment::Generic);
    }

    #[test]
    fn claude_code_takes_priority_over_others() {
        let detect = detect_from(env_with(&[("CLAUDE_CODE", "1"), ("OPENCODE", "1")]));
        assert_eq!(detect, Environment::ClaudeCode);
    }

    #[test]
    fn skill_rel_path_format() {
        let env = Environment::ClaudeCode;
        assert_eq!(
            env.skill_rel_path("agent-doc"),
            PathBuf::from(".claude/skills/agent-doc/SKILL.md")
        );
    }

    #[test]
    fn skill_path_claude_with_root() {
        let env = Environment::ClaudeCode;
        let path = env.skill_path("my-tool", Some(Path::new("/project")));
        assert_eq!(path, PathBuf::from("/project/.claude/skills/my-tool/SKILL.md"));
    }

    #[test]
    fn skill_path_generic_without_root() {
        let env = Environment::Generic;
        let path = env.skill_path("my-tool", None);
        assert_eq!(path, PathBuf::from("AGENTS.md"));
    }

    #[test]
    fn skill_path_per_environment() {
        assert_eq!(
            Environment::ClaudeCode.skill_rel_path("tool"),
            PathBuf::from(".claude/skills/tool/SKILL.md")
        );
        assert_eq!(
            Environment::OpenCode.skill_rel_path("tool"),
            PathBuf::from(".opencode/skills/tool/SKILL.md")
        );
        assert_eq!(
            Environment::Codex.skill_rel_path("tool"),
            PathBuf::from(".codex/AGENTS.md")
        );
        assert_eq!(
            Environment::Cursor.skill_rel_path("tool"),
            PathBuf::from(".cursor/rules/tool.md")
        );
        assert_eq!(
            Environment::Generic.skill_rel_path("tool"),
            PathBuf::from("AGENTS.md")
        );
    }

    #[test]
    fn from_name_parses_variants() {
        assert_eq!(Environment::from_name("claude"), Some(Environment::ClaudeCode));
        assert_eq!(Environment::from_name("claude-code"), Some(Environment::ClaudeCode));
        assert_eq!(Environment::from_name("opencode"), Some(Environment::OpenCode));
        assert_eq!(Environment::from_name("codex"), Some(Environment::Codex));
        assert_eq!(Environment::from_name("cursor"), Some(Environment::Cursor));
        assert_eq!(Environment::from_name("generic"), Some(Environment::Generic));
        assert_eq!(Environment::from_name("unknown"), None);
    }

    #[test]
    fn all_skill_rel_paths_returns_four() {
        let paths = Environment::all_skill_rel_paths("tool");
        assert_eq!(paths.len(), 4);
    }

    #[test]
    fn display_variants() {
        assert_eq!(Environment::ClaudeCode.to_string(), "Claude Code");
        assert_eq!(Environment::OpenCode.to_string(), "OpenCode");
        assert_eq!(Environment::Codex.to_string(), "Codex");
        assert_eq!(Environment::Cursor.to_string(), "Cursor");
        assert_eq!(Environment::Generic.to_string(), "Generic");
    }

    #[test]
    fn rules_dir_cursor_specific() {
        assert_eq!(Environment::Cursor.rules_dir(), PathBuf::from(".cursor/rules"));
    }

    #[test]
    fn rules_dir_generic() {
        assert_eq!(Environment::ClaudeCode.rules_dir(), PathBuf::from(".agent/rules"));
        assert_eq!(Environment::Generic.rules_dir(), PathBuf::from(".agent/rules"));
    }

    #[test]
    fn runbooks_dir_universal() {
        assert_eq!(Environment::ClaudeCode.runbooks_dir(), PathBuf::from(".agent/runbooks"));
        assert_eq!(Environment::Cursor.runbooks_dir(), PathBuf::from(".agent/runbooks"));
        assert_eq!(Environment::Generic.runbooks_dir(), PathBuf::from(".agent/runbooks"));
    }

    #[test]
    fn memories_dir_universal() {
        assert_eq!(Environment::ClaudeCode.memories_dir(), PathBuf::from(".agent/memories"));
        assert_eq!(Environment::Generic.memories_dir(), PathBuf::from(".agent/memories"));
    }

    #[test]
    fn skills_dir_per_environment() {
        assert_eq!(Environment::ClaudeCode.skills_dir(), PathBuf::from(".claude/skills"));
        assert_eq!(Environment::OpenCode.skills_dir(), PathBuf::from(".opencode/skills"));
        assert_eq!(Environment::Cursor.skills_dir(), PathBuf::from(".cursor/rules"));
        assert_eq!(Environment::Generic.skills_dir(), PathBuf::from(".agent/skills"));
    }
}
