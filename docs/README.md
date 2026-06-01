# Merab Documentation

This directory contains project notes for Merab contributors and users.

## Start Here

- [CLI command reference](cli-commands.md)
- [Landing page](index.html)
- [Promotion plan](promotion-plan.md)
- [Project rules](RULES.md)
- [Public release checklist](public-release-checklist.md)
- [Sprint history](sprints/README.md)

## Planning Notes

The `plans/` directory at the repository root contains sprint-level plans for recently implemented and pending work. Older implemented sprint notes live under `docs/sprints/`.

## Local Configuration

Keep real local config out of Git:

- Use `merab.example.toml` as a template.
- Keep the real `merab.toml` ignored.
- Store provider keys through `merab config set proxy.api_key replace-with-your-provider-key` or `MERAB_PROXY__API_KEY`.
