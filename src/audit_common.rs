//! Shared audit primitives for cross-cutting validation.
//!
//! These functions apply to all content types and domain crates,
//! not just instruction files. Moved from `instruction-files` to
//! enable reuse across the agent toolkit.

use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Configuration for instruction file discovery and auditing.
///
/// Different projects can customize behavior by providing different configs.
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Project root marker files, checked in order.
    /// agent-doc uses many (Cargo.toml, package.json, etc.); corky uses only Cargo.toml.
    pub root_markers: Vec<&'static str>,

    /// Whether to include CLAUDE.md in root-level discovery and agent file checks.
    /// agent-doc: true, corky: false.
    pub include_claude_md: bool,

    /// Source file extensions to check for staleness comparison.
    /// agent-doc: broad (rs, ts, py, etc.); corky: just "rs".
    pub source_extensions: Vec<&'static str>,

    /// Source directories to scan for staleness.
    /// agent-doc: ["src", "lib", "app", ...]; corky: just ["src"].
    pub source_dirs: Vec<&'static str>,

    /// Directories to skip when scanning for source files.
    pub skip_dirs: Vec<&'static str>,
}

impl AuditConfig {
    /// Config matching agent-doc's current behavior: broad project detection,
    /// includes CLAUDE.md, scans many source extensions.
    pub fn agent_doc() -> Self {
        Self {
            root_markers: vec![
                "Cargo.toml",
                "package.json",
                "pyproject.toml",
                "setup.py",
                "go.mod",
                "Gemfile",
                "pom.xml",
                "build.gradle",
                "CMakeLists.txt",
                "Makefile",
                "flake.nix",
                "deno.json",
                "composer.json",
            ],
            include_claude_md: true,
            source_extensions: vec![
                "rs", "ts", "tsx", "js", "jsx", "py", "go", "rb", "java", "kt", "c", "cpp", "h",
                "hpp", "cs", "swift", "zig", "hs", "ml", "ex", "exs", "clj", "scala", "lua",
                "php", "sh", "bash", "zsh",
            ],
            source_dirs: vec!["src", "lib", "app", "pkg", "cmd", "internal"],
            skip_dirs: vec![
                "node_modules",
                "target",
                "build",
                "dist",
                ".git",
                "__pycache__",
                ".venv",
                "vendor",
                ".next",
                "out",
            ],
        }
    }

    /// Config matching corky's current behavior: Cargo.toml-only root detection,
    /// excludes CLAUDE.md from audit, scans only .rs files.
    pub fn corky() -> Self {
        Self {
            root_markers: vec!["Cargo.toml"],
            include_claude_md: false,
            source_extensions: vec!["rs"],
            source_dirs: vec!["src"],
            skip_dirs: vec!["target", ".git"],
        }
    }
}

/// An issue found during auditing.
pub struct Issue {
    pub file: String,
    pub line: usize,
    pub end_line: usize,
    pub message: String,
    pub warning: bool,
}

/// Check if a file path refers to an agent instruction file.
pub fn is_agent_file(rel: &str, config: &AuditConfig) -> bool {
    let name = Path::new(rel)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name == "AGENTS.md" || name == "SKILL.md" {
        return true;
    }
    if config.include_claude_md && name == "CLAUDE.md" {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// Default line budget for combined instruction files.
pub const LINE_BUDGET: usize = 1000;

static MACHINE_LOCAL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)(?:~/|/home/\w+|/Users/\w+|/root/|/tmp/|C:\\Users\\)").unwrap()
});

/// Check instruction files for machine-local path references.
///
/// Flags paths like `~/`, `/home/user/`, `/Users/user/`, `/tmp/` that won't
/// resolve on other machines. These should use repo-relative paths or
/// declared dependency references instead.
pub fn check_context_invariant(rel: &str, content: &str, config: &AuditConfig) -> Vec<Issue> {
    if !is_agent_file(rel, config) {
        return vec![];
    }

    let mut issues = Vec::new();
    let mut in_code_fence = false;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Track code fences -- skip content inside them
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }

        // Check for machine-local paths
        if let Some(m) = MACHINE_LOCAL_RE.find(line) {
            issues.push(Issue {
                file: rel.to_string(),
                line: i + 1,
                end_line: 0,
                message: format!(
                    "Machine-local path \"{}\" \u{2014} use repo-relative path instead",
                    m.as_str()
                ),
                warning: true,
            });
        }
    }

    issues
}

/// Check if instruction files are older than source code.
pub fn check_staleness(files: &[PathBuf], root: &Path, config: &AuditConfig) -> Vec<Issue> {
    let mut newest_mtime = std::time::SystemTime::UNIX_EPOCH;
    let mut newest_src = PathBuf::new();

    fn scan_sources(
        dir: &Path,
        extensions: &[&str],
        skip_dirs: &[&str],
        newest: &mut std::time::SystemTime,
        newest_path: &mut PathBuf,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str())
                        && skip_dirs.contains(&name)
                    {
                        continue;
                    }
                    scan_sources(&path, extensions, skip_dirs, newest, newest_path);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && extensions.contains(&ext)
                    && let Ok(meta) = path.metadata()
                    && let Ok(mtime) = meta.modified()
                    && mtime > *newest
                {
                    *newest = mtime;
                    *newest_path = path;
                }
            }
        }
    }

    let mut found_any = false;
    for source_dir in &config.source_dirs {
        let dir = root.join(source_dir);
        if dir.exists() {
            found_any = true;
            scan_sources(
                &dir,
                &config.source_extensions,
                &config.skip_dirs,
                &mut newest_mtime,
                &mut newest_src,
            );
        }
    }

    if !found_any {
        return vec![];
    }

    let mut issues = Vec::new();
    for doc in files {
        if let Ok(meta) = doc.metadata()
            && let Ok(doc_mtime) = meta.modified()
            && doc_mtime < newest_mtime
        {
            let rel = doc.strip_prefix(root).unwrap_or(doc).to_string_lossy().to_string();
            let src_rel = newest_src
                .strip_prefix(root)
                .unwrap_or(&newest_src)
                .to_string_lossy()
                .to_string();
            issues.push(Issue {
                file: rel,
                line: 0,
                end_line: 0,
                message: format!("Older than {} \u{2014} may be stale", src_rel),
                warning: false,
            });
        }
    }
    issues
}

/// Check combined line count against budget.
///
/// Only counts agent instruction files (AGENTS.md, SKILL.md, optionally CLAUDE.md).
/// Reference docs (README.md, SPEC.md) are listed but excluded from the budget.
pub fn check_line_budget(
    files: &[PathBuf],
    root: &Path,
    config: &AuditConfig,
) -> (Vec<Issue>, Vec<(String, usize)>, usize) {
    let mut counts = Vec::new();
    let mut total = 0;
    for f in files {
        if let Ok(content) = std::fs::read_to_string(f) {
            let n = content.lines().count();
            let rel = f.strip_prefix(root).unwrap_or(f).to_string_lossy().to_string();
            if is_agent_file(&rel, config) {
                total += n;
            }
            counts.push((rel, n));
        }
    }
    let mut issues = Vec::new();
    if total > LINE_BUDGET {
        issues.push(Issue {
            file: "(all)".to_string(),
            line: 0,
            end_line: 0,
            message: format!("Over line budget: {} lines (max {})", total, LINE_BUDGET),
            warning: false,
        });
    }
    (issues, counts, total)
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Find the project root by walking up from CWD.
///
/// Strategy depends on config:
/// - Pass 1: Check `config.root_markers` in order
/// - Pass 2: Check for `.git` directory
/// - Pass 3: Fall back to CWD
pub fn find_root(config: &AuditConfig) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Pass 1: Look for project marker files
    let mut dir = cwd.as_path();
    loop {
        for marker in &config.root_markers {
            if dir.join(marker).exists() {
                return dir.to_path_buf();
            }
        }
        match dir.parent() {
            Some(p) if p != dir => dir = p,
            _ => break,
        }
    }

    // Pass 2: Look for .git directory
    dir = cwd.as_path();
    loop {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(p) if p != dir => dir = p,
            _ => break,
        }
    }

    // Pass 3: Fall back to CWD
    eprintln!("Warning: no project root marker found, using current directory");
    cwd
}

/// Discover all instruction files under the given root.
///
/// Searches for:
/// - Root-level: AGENTS.md, README.md, SPEC.md, and optionally CLAUDE.md
/// - Glob patterns: .claude/**/SKILL.md, .agents/**/SKILL.md, .agents/**/AGENTS.md, src/**/AGENTS.md
/// - If `config.include_claude_md`: also .claude/**/CLAUDE.md, src/**/CLAUDE.md
pub fn find_instruction_files(root: &Path, config: &AuditConfig) -> Vec<PathBuf> {
    let mut root_patterns = vec!["AGENTS.md", "README.md", "SPEC.md"];
    if config.include_claude_md {
        root_patterns.push("CLAUDE.md");
    }

    let mut found = std::collections::HashSet::new();

    for pattern in &root_patterns {
        let path = root.join(pattern);
        if path.exists() {
            found.insert(path);
        }
    }

    // Common glob patterns
    let mut glob_patterns = vec![
        ".claude/**/SKILL.md",
        ".agents/**/SKILL.md",
        ".agents/**/AGENTS.md",
        "src/**/AGENTS.md",
        ".agent/runbooks/*.md",
        ".claude/skills/**/runbooks/*.md",
    ];

    if config.include_claude_md {
        glob_patterns.push(".claude/**/CLAUDE.md");
        glob_patterns.push("src/**/CLAUDE.md");
    }

    for pattern in &glob_patterns {
        if let Ok(entries) = glob::glob(&root.join(pattern).to_string_lossy()) {
            for entry in entries.flatten() {
                found.insert(entry);
            }
        }
    }

    let mut result: Vec<PathBuf> = found.into_iter().collect();
    result.sort();
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // --- is_agent_file ---

    #[test]
    fn is_agent_file_with_claude() {
        let config = AuditConfig::agent_doc();
        assert!(is_agent_file("AGENTS.md", &config));
        assert!(is_agent_file("SKILL.md", &config));
        assert!(is_agent_file("CLAUDE.md", &config));
        assert!(is_agent_file("src/AGENTS.md", &config));
        assert!(is_agent_file(".claude/skills/email/SKILL.md", &config));
        assert!(is_agent_file("nested/path/CLAUDE.md", &config));
    }

    #[test]
    fn is_agent_file_without_claude() {
        let config = AuditConfig::corky();
        assert!(is_agent_file("AGENTS.md", &config));
        assert!(is_agent_file("SKILL.md", &config));
        assert!(!is_agent_file("CLAUDE.md", &config));
    }

    #[test]
    fn is_agent_file_rejects() {
        let config = AuditConfig::agent_doc();
        assert!(!is_agent_file("README.md", &config));
        assert!(!is_agent_file("agents.md", &config));
        assert!(!is_agent_file("CHANGELOG.md", &config));
        assert!(!is_agent_file("src/main.rs", &config));
    }

    // --- check_context_invariant ---

    #[test]
    fn context_invariant_flags_home_tilde() {
        let config = AuditConfig::agent_doc();
        let content = "# Doc\n\nSee ~/some/path for config.\n";
        let issues = check_context_invariant("CLAUDE.md", content, &config);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("Machine-local path"));
        assert!(issues[0].warning);
    }

    #[test]
    fn context_invariant_flags_home_absolute() {
        let config = AuditConfig::agent_doc();
        let content = "# Doc\n\nConfig at /home/brian/.config/foo.\n";
        let issues = check_context_invariant("AGENTS.md", content, &config);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("/home/brian"));
    }

    #[test]
    fn context_invariant_flags_macos_users() {
        let config = AuditConfig::agent_doc();
        let content = "# Doc\n\nSee /Users/alice/project.\n";
        let issues = check_context_invariant("CLAUDE.md", content, &config);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("/Users/alice"));
    }

    #[test]
    fn context_invariant_skips_code_fences() {
        let config = AuditConfig::agent_doc();
        let content = "# Doc\n\n```bash\nexistence --ontology ~/path\n```\n";
        let issues = check_context_invariant("CLAUDE.md", content, &config);
        assert!(issues.is_empty());
    }

    #[test]
    fn context_invariant_clean_file() {
        let config = AuditConfig::agent_doc();
        let content = "# Doc\n\nUse `src/main.rs` for the entry point.\n";
        let issues = check_context_invariant("AGENTS.md", content, &config);
        assert!(issues.is_empty());
    }

    #[test]
    fn context_invariant_skips_non_agent_files() {
        let config = AuditConfig::agent_doc();
        let content = "# Doc\n\nSee ~/config.\n";
        let issues = check_context_invariant("README.md", content, &config);
        assert!(issues.is_empty());
    }

    // --- check_line_budget ---

    #[test]
    fn check_line_budget_under() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("AGENTS.md"), "line1\nline2\nline3\n").unwrap();

        let config = AuditConfig::corky();
        let files = vec![root.join("AGENTS.md")];
        let (issues, counts, total) = check_line_budget(&files, root, &config);
        assert!(issues.is_empty());
        assert_eq!(total, 3);
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].0, "AGENTS.md");
        assert_eq!(counts[0].1, 3);
    }

    #[test]
    fn check_line_budget_over() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let content = "line\n".repeat(1001);
        fs::write(root.join("AGENTS.md"), &content).unwrap();

        let config = AuditConfig::corky();
        let files = vec![root.join("AGENTS.md")];
        let (issues, _, total) = check_line_budget(&files, root, &config);
        assert_eq!(total, 1001);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("Over line budget"));
    }

    #[test]
    fn check_line_budget_multiple_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("AGENTS.md"), "a\nb\n").unwrap();
        fs::write(root.join("SKILL.md"), "c\nd\ne\n").unwrap();

        let config = AuditConfig::corky();
        let files = vec![root.join("AGENTS.md"), root.join("SKILL.md")];
        let (_, counts, total) = check_line_budget(&files, root, &config);
        assert_eq!(total, 5);
        assert_eq!(counts.len(), 2);
    }

    #[test]
    fn check_line_budget_excludes_non_agent_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("AGENTS.md"), "a\nb\n").unwrap();
        let big_spec = "line\n".repeat(2000);
        fs::write(root.join("SPEC.md"), &big_spec).unwrap();
        fs::write(root.join("README.md"), "readme\n").unwrap();

        let config = AuditConfig::corky();
        let files = vec![
            root.join("AGENTS.md"),
            root.join("SPEC.md"),
            root.join("README.md"),
        ];
        let (issues, counts, total) = check_line_budget(&files, root, &config);
        // Only AGENTS.md counts toward budget (2 lines)
        assert_eq!(total, 2);
        assert!(issues.is_empty());
        // All files listed in counts
        assert_eq!(counts.len(), 3);
    }

    // --- check_staleness ---

    #[test]
    fn check_staleness_doc_newer_than_src() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(root.join("CLAUDE.md"), "# Doc").unwrap();

        let config = AuditConfig::agent_doc();
        let files = vec![root.join("CLAUDE.md")];
        let issues = check_staleness(&files, root, &config);
        assert!(issues.is_empty());
    }

    #[test]
    fn check_staleness_doc_older_than_src() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(root.join("CLAUDE.md"), "# Doc").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();

        let config = AuditConfig::agent_doc();
        let files = vec![root.join("CLAUDE.md")];
        let issues = check_staleness(&files, root, &config);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("may be stale"));
    }

    #[test]
    fn check_staleness_no_src_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("CLAUDE.md"), "# Doc").unwrap();

        let config = AuditConfig::agent_doc();
        let files = vec![root.join("CLAUDE.md")];
        let issues = check_staleness(&files, root, &config);
        assert!(issues.is_empty());
    }

    // --- find_instruction_files ---

    #[test]
    fn find_instruction_files_root_patterns_with_claude() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("CLAUDE.md"), "# Doc").unwrap();
        fs::write(root.join("README.md"), "# Readme").unwrap();
        fs::write(root.join("AGENTS.md"), "# Agents").unwrap();

        let config = AuditConfig::agent_doc();
        let files = find_instruction_files(root, &config);
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.ends_with("CLAUDE.md")));
        assert!(files.iter().any(|f| f.ends_with("README.md")));
        assert!(files.iter().any(|f| f.ends_with("AGENTS.md")));
    }

    #[test]
    fn find_instruction_files_root_patterns_without_claude() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("CLAUDE.md"), "# Doc").unwrap();
        fs::write(root.join("README.md"), "# Readme").unwrap();
        fs::write(root.join("AGENTS.md"), "# Agents").unwrap();

        let config = AuditConfig::corky();
        let files = find_instruction_files(root, &config);
        assert_eq!(files.len(), 2);
        assert!(!files.iter().any(|f| f.ends_with("CLAUDE.md")));
    }

    #[test]
    fn find_instruction_files_glob_patterns() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join(".claude/skills/email")).unwrap();
        fs::write(root.join(".claude/skills/email/SKILL.md"), "# Skill").unwrap();

        fs::create_dir_all(root.join(".claude/settings")).unwrap();
        fs::write(root.join(".claude/settings/CLAUDE.md"), "# Claude").unwrap();

        fs::create_dir_all(root.join("src/agent")).unwrap();
        fs::write(root.join("src/agent/CLAUDE.md"), "# Agent").unwrap();
        fs::write(root.join("src/agent/AGENTS.md"), "# Agents").unwrap();

        let config = AuditConfig::agent_doc();
        let files = find_instruction_files(root, &config);
        assert_eq!(files.len(), 4);
    }

    #[test]
    fn find_instruction_files_empty() {
        let tmp = TempDir::new().unwrap();
        let config = AuditConfig::agent_doc();
        let files = find_instruction_files(tmp.path(), &config);
        assert!(files.is_empty());
    }

    #[test]
    fn find_instruction_files_sorted() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("README.md"), "# R").unwrap();
        fs::write(root.join("CLAUDE.md"), "# C").unwrap();
        fs::write(root.join("AGENTS.md"), "# A").unwrap();

        let config = AuditConfig::agent_doc();
        let files = find_instruction_files(root, &config);
        let names: Vec<_> = files.iter().map(|f| f.file_name().unwrap()).collect();
        assert!(names.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn find_instruction_files_discovers_spec_md() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("SPEC.md"), "# Spec").unwrap();
        fs::write(root.join("AGENTS.md"), "# Agents").unwrap();

        let config = AuditConfig::corky();
        let files = find_instruction_files(root, &config);
        assert!(files.iter().any(|f| f.ends_with("SPEC.md")));
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn find_instruction_files_discovers_runbooks() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join(".agent/runbooks")).unwrap();
        fs::write(root.join(".agent/runbooks/precommit.md"), "# Precommit").unwrap();
        fs::write(
            root.join(".agent/runbooks/prerelease.md"),
            "# Prerelease",
        )
        .unwrap();

        fs::create_dir_all(root.join(".claude/skills/email/runbooks")).unwrap();
        fs::write(
            root.join(".claude/skills/email/runbooks/send.md"),
            "# Send",
        )
        .unwrap();

        let config = AuditConfig::corky();
        let files = find_instruction_files(root, &config);
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.ends_with("precommit.md")));
        assert!(files.iter().any(|f| f.ends_with("prerelease.md")));
        assert!(files.iter().any(|f| f.ends_with("send.md")));
    }

    #[test]
    fn find_instruction_files_deduplicates() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("CLAUDE.md"), "# Doc").unwrap();

        let config = AuditConfig::agent_doc();
        let files = find_instruction_files(root, &config);
        assert_eq!(files.len(), 1);
    }
}
