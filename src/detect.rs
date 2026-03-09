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
    /// should be installed (e.g., `.claude/skills/{name}/SKILL.md`).
    pub fn skill_rel_path(&self, name: &str) -> PathBuf {
        // All environments currently use the Claude Code layout.
        // Future: adapt per environment as conventions emerge.
        PathBuf::from(format!(".claude/skills/{name}/SKILL.md"))
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
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClaudeCode => write!(f, "Claude Code"),
            Self::OpenCode => write!(f, "OpenCode"),
            Self::Codex => write!(f, "Codex"),
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
    fn skill_path_with_root() {
        let env = Environment::Generic;
        let path = env.skill_path("my-tool", Some(Path::new("/project")));
        assert_eq!(path, PathBuf::from("/project/.claude/skills/my-tool/SKILL.md"));
    }

    #[test]
    fn skill_path_without_root() {
        let env = Environment::Generic;
        let path = env.skill_path("my-tool", None);
        assert_eq!(path, PathBuf::from(".claude/skills/my-tool/SKILL.md"));
    }

    #[test]
    fn display_variants() {
        assert_eq!(Environment::ClaudeCode.to_string(), "Claude Code");
        assert_eq!(Environment::OpenCode.to_string(), "OpenCode");
        assert_eq!(Environment::Codex.to_string(), "Codex");
        assert_eq!(Environment::Generic.to_string(), "Generic");
    }
}
