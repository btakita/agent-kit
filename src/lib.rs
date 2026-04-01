//! Agent Kit — toolkit for CLI tools integrating with AI agent loops.
//!
//! Provides shared infrastructure for:
//! - Skill management (install/check/uninstall SKILL.md files)
//! - Agent environment detection (Claude Code, OpenCode, etc.)
//! - Instruction file auditing (via `instruction-files` crate, behind `audit` feature)
//! - Hook-based event coordination between sessions (behind `hooks` feature)

#[cfg(feature = "audit")]
pub mod audit;
pub mod detect;
#[cfg(feature = "hooks")]
pub mod hooks;
pub mod skill;
