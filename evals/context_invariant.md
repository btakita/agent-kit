# Eval: Context Invariant

**Status:** planned

## Goal

Verify that `check_context_invariant` catches all machine-local path patterns in agent instruction files.

## Test Cases

1. **Home tilde** -- `~/some/path` flagged as machine-local
2. **Linux home** -- `/home/brian/.config/foo` flagged
3. **macOS Users** -- `/Users/alice/project` flagged
4. **Root home** -- `/root/.bashrc` flagged
5. **Tmp dir** -- `/tmp/scratch` flagged
6. **Windows Users** -- `C:\Users\bob` flagged
7. **Code fence skip** -- Paths inside ``````` blocks are NOT flagged
8. **Non-agent file skip** -- Paths in README.md are NOT flagged (not an agent file)
9. **Clean file** -- Repo-relative paths like `src/main.rs` produce zero issues
10. **Multiple hits** -- File with 3 machine-local paths on separate lines produces 3 issues

## Pass Criteria

All machine-local patterns detected with correct line numbers. No false positives on code fences or non-agent files. Warning flag is `true` for all context-invariant issues.

## Coverage

Unit tests in `audit_common.rs` cover cases 1-6, 7, 8, 9. This eval adds case 10 (multiple hits) as acceptance criteria.
