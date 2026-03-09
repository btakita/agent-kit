//! Skill management — install/check/uninstall SKILL.md files for agent environments.
//!
//! CLI tools bundle a SKILL.md via `include_str!` and use this module to install
//! it to the appropriate location for the active agent environment.

use crate::detect::Environment;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Configuration for a skill to be managed.
pub struct SkillConfig {
    /// The tool name (e.g., "agent-doc", "webmaster").
    pub name: String,
    /// The bundled SKILL.md content (typically from `include_str!`).
    pub content: String,
    /// The tool version (typically from `env!("CARGO_PKG_VERSION")`).
    pub version: String,
    /// The detected agent environment (used for path resolution).
    pub environment: Environment,
}

impl SkillConfig {
    /// Create a new skill config.
    /// Create a new skill config, auto-detecting the agent environment.
    pub fn new(name: impl Into<String>, content: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            version: version.into(),
            environment: Environment::detect(),
        }
    }

    /// Create a new skill config with an explicit environment.
    pub fn with_environment(
        name: impl Into<String>,
        content: impl Into<String>,
        version: impl Into<String>,
        environment: Environment,
    ) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            version: version.into(),
            environment,
        }
    }

    /// Resolve the skill file path under the given root (or CWD if None).
    /// Uses the detected environment to determine the path layout.
    pub fn skill_path(&self, root: Option<&Path>) -> PathBuf {
        self.environment.skill_path(&self.name, root)
    }

    /// Install the bundled SKILL.md to the project.
    /// When `root` is None, paths are relative to CWD.
    pub fn install(&self, root: Option<&Path>) -> Result<()> {
        let path = self.skill_path(root);

        // Check if already up to date
        if path.exists() {
            let existing = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            if existing == self.content {
                eprintln!("Skill already up to date (v{}).", self.version);
                return Ok(());
            }
        }

        // Create directories
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        // Write
        std::fs::write(&path, &self.content)
            .with_context(|| format!("failed to write {}", path.display()))?;
        eprintln!("Installed skill v{} → {}", self.version, path.display());

        Ok(())
    }

    /// Check if the installed skill matches the bundled version.
    /// When `root` is None, paths are relative to CWD.
    ///
    /// Returns `Ok(true)` if up to date, `Ok(false)` if outdated or not installed.
    pub fn check(&self, root: Option<&Path>) -> Result<bool> {
        let path = self.skill_path(root);

        if !path.exists() {
            eprintln!("Not installed. Run `{} skill install` to install.", self.name);
            return Ok(false);
        }

        let existing = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        if existing == self.content {
            eprintln!("Up to date (v{}).", self.version);
            Ok(true)
        } else {
            eprintln!(
                "Outdated. Run `{} skill install` to update to v{}.",
                self.name, self.version
            );
            Ok(false)
        }
    }

    /// Uninstall the skill file and its parent directory (if empty).
    pub fn uninstall(&self, root: Option<&Path>) -> Result<()> {
        let path = self.skill_path(root);

        if !path.exists() {
            eprintln!("Skill not installed.");
            return Ok(());
        }

        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;

        // Remove parent dir if empty
        if let Some(parent) = path.parent()
            && parent.read_dir().is_ok_and(|mut d| d.next().is_none())
        {
            let _ = std::fs::remove_dir(parent);
        }

        eprintln!("Uninstalled skill from {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SkillConfig {
        SkillConfig::with_environment(
            "test-tool",
            "# Test Skill\n\nSome content.\n",
            "1.0.0",
            crate::detect::Environment::ClaudeCode,
        )
    }

    #[test]
    fn skill_path_with_root() {
        let config = test_config();
        let path = config.skill_path(Some(Path::new("/project")));
        assert_eq!(path, PathBuf::from("/project/.claude/skills/test-tool/SKILL.md"));
    }

    #[test]
    fn skill_path_without_root() {
        let config = test_config();
        let path = config.skill_path(None);
        assert_eq!(path, PathBuf::from(".claude/skills/test-tool/SKILL.md"));
    }

    #[test]
    fn install_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config();

        config.install(Some(dir.path())).unwrap();

        let path = dir.path().join(".claude/skills/test-tool/SKILL.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, config.content);
    }

    #[test]
    fn install_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config();

        config.install(Some(dir.path())).unwrap();
        config.install(Some(dir.path())).unwrap();

        let path = dir.path().join(".claude/skills/test-tool/SKILL.md");
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, config.content);
    }

    #[test]
    fn install_overwrites_outdated() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config();

        let path = dir.path().join(".claude/skills/test-tool/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "old content").unwrap();

        config.install(Some(dir.path())).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, config.content);
    }

    #[test]
    fn check_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config();

        let result = config.check(Some(dir.path())).unwrap();
        assert!(!result);
    }

    #[test]
    fn check_up_to_date() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config();

        config.install(Some(dir.path())).unwrap();
        let result = config.check(Some(dir.path())).unwrap();
        assert!(result);
    }

    #[test]
    fn check_outdated() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config();

        let path = dir.path().join(".claude/skills/test-tool/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "old content").unwrap();

        let result = config.check(Some(dir.path())).unwrap();
        assert!(!result);
    }

    #[test]
    fn uninstall_removes_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config();

        config.install(Some(dir.path())).unwrap();
        config.uninstall(Some(dir.path())).unwrap();

        let path = dir.path().join(".claude/skills/test-tool/SKILL.md");
        assert!(!path.exists());
    }

    #[test]
    fn uninstall_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config();

        // Should not error
        config.uninstall(Some(dir.path())).unwrap();
    }
}
