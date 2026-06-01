# Public Release Checklist

Use this checklist before making the GitHub repository public.

## Required

- Rotate any provider key that was ever committed.
- Remove local-only files from Git: `merab.toml`, `.env*`, logs, SQLite databases, `.claude/`, `.codex/`, and temporary scripts.
- Keep `merab.example.toml` and `.env.example` free of real-looking credentials.
- Run a secret scan against the working tree and all Git refs.
- Rewrite Git history or publish from a fresh clean repository if a real secret ever appeared in commits.
- Run the verification suite:

```bash
cargo build --workspace
cargo test --workspace
```

## GitHub Settings

- Enable secret scanning if available.
- Enable Dependabot alerts.
- Protect the default branch once the project has external contributors.
- Use private vulnerability reporting or GitHub Security Advisories for security reports.

## After Publishing

- Add repository topics such as `rust`, `ai-agents`, `mcp`, `json-rpc`, and `local-first`.
- Confirm the README quick start works from a fresh clone.
- Keep local provider keys in user config or environment variables only.
