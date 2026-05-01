//! Agent Kit — shared infrastructure for CLI tools integrating with AI agent loops.
//!
//! Provides:
//! - Agent environment detection (Claude Code, OpenCode, Codex, Cursor, Generic)
//! - Instruction file auditing (via `instruction-files` crate, behind `audit` feature)
//! - Hook-based event coordination between sessions (behind `hooks` feature)
//!
//! Skill management has moved to the `agent-skills` crate.

#[cfg(feature = "audit")]
pub mod audit;
pub mod audit_common;
pub mod detect;
#[cfg(feature = "hooks")]
pub mod hooks;
pub mod skill;
