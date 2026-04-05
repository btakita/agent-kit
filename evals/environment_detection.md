# Eval: Environment Detection

**Status:** planned

## Goal

Verify that `Environment::detect()` correctly identifies each supported environment from environment variables.

## Test Cases

1. **ClaudeCode via CLAUDE_CODE** -- Set `CLAUDE_CODE=1`, expect `Environment::ClaudeCode`
2. **ClaudeCode via CLAUDE_CODE_ENTRYPOINT** -- Set `CLAUDE_CODE_ENTRYPOINT=/usr/bin/claude`, expect `Environment::ClaudeCode`
3. **OpenCode** -- Set `OPENCODE=1`, expect `Environment::OpenCode`
4. **Codex via CODEX_CLI** -- Set `CODEX_CLI=1`, expect `Environment::Codex`
5. **Codex via CODEX** -- Set `CODEX=1`, expect `Environment::Codex`
6. **Cursor via CURSOR_SESSION_ID** -- Set `CURSOR_SESSION_ID=abc`, expect `Environment::Cursor`
7. **Cursor via CURSOR** -- Set `CURSOR=1`, expect `Environment::Cursor`
8. **Generic fallback** -- No env vars set, expect `Environment::Generic`
9. **Priority: ClaudeCode over OpenCode** -- Set both `CLAUDE_CODE=1` and `OPENCODE=1`, expect `Environment::ClaudeCode`
10. **Path consistency** -- After detection, all path methods (`skill_rel_path`, `rules_dir`, etc.) return paths matching the detected environment

## Pass Criteria

All 10 test cases pass. No false positives (detecting wrong environment). No panics on missing env vars.

## Coverage

Unit tests in `detect.rs` already cover cases 1-9. This eval adds case 10 (path consistency after detection) and serves as the acceptance test spec.
