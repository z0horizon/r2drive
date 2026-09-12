# Agent Guidelines: r2drive

This document guides AI coding assistants working in the `r2drive` repository.

## Branching & Workflow Rules

- **Integration Branch:** The primary working branch is `dev`. New features must branch from `dev` (`feat/<name>`). Releases merge from `dev` to `main`.
- **Secret Safety:** Never commit `.env`, `config.yaml`, or `*.db`. Never print raw `R2_SECRET_ACCESS_KEY` or tokens to terminal output.
- **Verification First:** Always run `./scripts/check.sh fast` before committing, and `./scripts/check.sh test` before concluding any task.

## Issue Tracking & Triage

GitHub issues live in this repo using the `gh` CLI. See `docs/agents/issue-tracker.md`.
Use the five canonical triage labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

## Domain Language

Domain architecture follows single-context layout (`CONTEXT.md` + `docs/adr/`). Refer to `CONTEXT.md` for ubiquitous terminology (`StorageNode`, `WebConsole`, `PresignedTransfer`, `MetadataStore`, `BucketProfile`).
