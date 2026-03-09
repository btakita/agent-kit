//! Instruction file auditing — re-exports `instruction-files` with preset configs.
//!
//! Provides ready-made audit configurations for CLI tools that bundle SKILL.md files
//! and want to validate their instruction files (staleness, tree paths, line budgets,
//! actionable content).

pub use instruction_files::{
    check_actionable, check_line_budget, check_staleness, check_tree_paths,
    find_instruction_files, find_root, run, AuditConfig, Issue,
};
