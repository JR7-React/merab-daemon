# Security Policy

Merab is a local agent runtime. Some commands can run tools, touch files, call shells, and forward requests to LLM providers, so security issues can have local-system impact.

## Reporting A Vulnerability

Please do not open a public issue for suspected vulnerabilities or leaked credentials.

Use GitHub Security Advisories if they are enabled for the repository. If not, contact the maintainer privately and include:

- A short description of the issue.
- Reproduction steps.
- Affected commit, version, or branch.
- Any logs or payloads needed to understand the issue, with secrets redacted.

## Secrets

Never commit real API keys, provider tokens, local `merab.toml`, `.env` files, SQLite databases, logs, or tool-state directories.

If a secret is committed:

1. Revoke or rotate it with the provider.
2. Remove it from the current tree.
3. Rewrite the affected Git history before making the repository public.
4. Re-run a secret scan on the working tree and all refs.

## Supported Versions

Merab is pre-1.0 and experimental. Security fixes are handled on the default branch until a formal release policy exists.
