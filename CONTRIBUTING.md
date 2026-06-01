# Contributing

Thanks for taking a look at Merab. This project is still experimental, so small, focused pull requests are easiest to review.

## Local Setup

```bash
cargo build --workspace
cargo test --workspace
```

If you want to exercise LLM-backed commands, configure your provider key locally:

```bash
merab config set proxy.api_key replace-with-your-provider-key
```

Do not put real keys in `merab.toml`, `.env`, examples, tests, logs, or docs.

## Code Style

- Use Rust edition 2024.
- Avoid `unwrap()` in production code. Use `?`, `map_err`, or explicit error handling.
- Library crates should prefer `thiserror`.
- Binary crates should prefer `anyhow`.
- Use `tracing::info!`, `tracing::warn!`, and `tracing::error!` in daemon/store code.
- Keep new files focused and under roughly 500 lines.
- Add tests for behavior that can regress.

## Before Opening A PR

```bash
cargo build --workspace
cargo test --workspace
```

Also check that your diff does not include local state:

```bash
git status --short
git diff --check
```

## Repository Hygiene

Ignored local files include `merab.toml`, `.env*`, SQLite databases, logs, `.claude/`, `.codex/`, and `target/`. If a credential is accidentally committed, rotate it immediately and tell the maintainer so the Git history can be cleaned before release.
