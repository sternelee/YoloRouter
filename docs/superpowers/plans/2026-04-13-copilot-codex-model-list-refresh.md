# Copilot And Codex Model List Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the hardcoded default model lists for `github_copilot` and `codex_oauth` to current official models.

**Architecture:** Keep the existing static fallback design in `src/provider/models.rs`. Only replace the two hardcoded arrays and update the assertions that lock in the expected defaults.

**Tech Stack:** Rust, Cargo test

---

### Task 1: Refresh Copilot And Codex Defaults

**Files:**
- Modify: `src/provider/models.rs`

- [ ] **Step 1: Update the GitHub Copilot hardcoded list**

Replace the `"github_copilot" => Ok(vec![ ... ])` entries with:

```rust
        "github_copilot" => Ok(vec![
            "gpt-5.4".to_string(),
            "gpt-5.4-mini".to_string(),
            "gpt-5.3-codex".to_string(),
            "gpt-5.3-codex-spark".to_string(),
            "claude-sonnet-4.6".to_string(),
            "claude-opus-4.6".to_string(),
            "gemini-2.5-pro".to_string(),
            "gemini-3.1-pro".to_string(),
            "grok-code-fast-1".to_string(),
        ]),
```

- [ ] **Step 2: Update the Codex OAuth hardcoded list**

Replace the `"codex_oauth" => Ok(vec![ ... ])` entries with:

```rust
        "codex_oauth" => Ok(vec![
            "gpt-5.4".to_string(),
            "gpt-5.4-mini".to_string(),
            "gpt-5.3-codex".to_string(),
            "gpt-5.3-codex-spark".to_string(),
            "gpt-5.2".to_string(),
        ]),
```

- [ ] **Step 3: Update tests to assert the new defaults**

Adjust the existing tests in `src/provider/models.rs`:

```rust
    #[test]
    fn test_codex_oauth_returns_hardcoded() {
        let cfg = make_cfg("codex_oauth", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("gpt-5.4")));
    }

    #[test]
    fn test_github_copilot_has_expected_models() {
        let cfg = make_cfg("github_copilot", None, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(fetch_provider_models(&cfg)).unwrap();
        assert!(models.contains(&"gpt-5.4".to_string()));
        assert!(models.contains(&"gpt-5.3-codex".to_string()));
        assert!(models.contains(&"claude-sonnet-4.6".to_string()));
        assert_eq!(models.len(), 9);
    }
```

- [ ] **Step 4: Run targeted verification**

Run: `cargo test provider::models::tests -- --nocapture`
Expected: all provider model tests pass.

- [ ] **Step 5: Run full verification**

Run: `cargo test`
Expected: full suite passes.

- [ ] **Step 6: Commit**

```bash
git add src/provider/models.rs docs/superpowers/plans/2026-04-13-copilot-codex-model-list-refresh.md
git commit -m "feat(provider): refresh default Copilot and Codex model lists"
```
