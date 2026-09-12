# Team Collaboration, Conventions & GitHub Setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish professional team collaboration standards for `r2drive` by setting up dependency lockfiles, local validation scripts, version-controlled git hooks, team conventions documentation, GitHub Actions CI automation, and a dual-branch (`main` + `dev`) GitHub repository.

**Architecture:** A dual-branch Git workflow (`main` for tagged production releases, `dev` as the default integration branch for feature PRs). Automated pre-commit and pre-push hooks safeguard local commits, developer helper scripts (`scripts/check.sh`) standardize validation, and GitHub Actions CI enforces Rust and Svelte 5 test/lint/build passes before any PR can merge.

**Tech Stack:** Git, GitHub CLI (`gh`), Bash, GitHub Actions, Cargo (`clippy`, `fmt`, `test`), Node 22 / pnpm (`svelte-check`, `vitest`, `vite build`), Docker.

**Spec:** Brainstorming agreement on branching strategy, conventions, hooks, and team rules documented in `docs/superpowers/specs/2026-09-12-cors-diagnostic-and-upload-fallback.md` and `ROADMAP.md`.

## Global Constraints

- **No Secret Leaks:** Never track or commit `config.yaml`, `.env`, or SQLite database files (`*.db`, `*.db-wal`, `*.db-shm`).
- **Reproducible Binary Builds:** Commit `Cargo.lock` (remove from `.gitignore`) and ensure `web/pnpm-lock.yaml` remains tracked.
- **Strict Quality Gates:** All local and CI checks must enforce zero Clippy warnings (`-D warnings`), zero formatting diffs, 100% test pass rate (Rust + Vitest), and 0 `svelte-check` diagnostics.
- **Dual-Branch Flow:** `main` serves as the stable release branch; `dev` serves as the primary integration branch for ongoing team PRs.

---

### Task 1: Lock Dependencies & Refine `.gitignore`

**Files:**
- Modify: `.gitignore`
- Add to Git: `Cargo.lock`

**Interfaces:**
- Consumes: Existing `.gitignore` and `Cargo.lock`.
- Produces: Version-controlled `Cargo.lock` ensuring identical dependency trees across all developer machines, Docker builds, and CI pipelines.

- [ ] **Step 1: Inspect and update `.gitignore`**

Remove `Cargo.lock` from `.gitignore` so that the application binary lockfile is tracked. Ensure sensitive and build artifacts remain strictly ignored:

```gitignore
/target
/web/dist
/web/node_modules
*.db
*.db-wal
*.db-shm
.env
config.yaml
```

- [ ] **Step 2: Stage and verify git tracking for `Cargo.lock`**

Run: `git add .gitignore Cargo.lock`
Verify with: `git status --porcelain`
Expected: `.gitignore` modified and `Cargo.lock` staged for commit.

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: track Cargo.lock and refine .gitignore for binary reproducibility"
```

---

### Task 2: Developer Verification & Git Hook Automation

**Files:**
- Create: `scripts/check.sh`
- Create: `scripts/install-git-hooks.sh`
- Create: `.githooks/pre-commit`
- Create: `.githooks/pre-push`

**Interfaces:**
- Consumes: Local Cargo toolchain and pnpm in `web/`.
- Produces: Standardized local check runner and Git hooks matching `r2kit`'s established developer experience.

- [ ] **Step 1: Create `scripts/check.sh`**

Executable helper supporting `fast`, `test`, and `full` check tiers:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-fast}"

run_fast() {
    echo "==> [Rust] Checking code formatting..."
    cargo fmt --all -- --check

    echo "==> [Rust] Running Clippy linter..."
    cargo clippy --all-targets -- -D warnings

    echo "==> [Frontend] Running Svelte check..."
    (cd web && pnpm check)
}

run_test() {
    echo "==> [Rust] Running backend test suite..."
    cargo test --all-targets

    echo "==> [Frontend] Running Vitest suite..."
    (cd web && pnpm test)

    echo "==> [Frontend] Verifying production build..."
    (cd web && pnpm build)
}

case "$MODE" in
    fast)
        run_fast
        echo "✅ Fast checks passed!"
        ;;
    test)
        run_test
        echo "✅ Test suite passed!"
        ;;
    full)
        run_fast
        run_test
        echo "✅ Full validation suite passed!"
        ;;
    *)
        echo "Usage: $0 [fast|test|full]"
        exit 1
        ;;
esac
```

- [ ] **Step 2: Create `.githooks/pre-commit` and `.githooks/pre-push`**

Create `.githooks/pre-commit`:
```bash
#!/usr/bin/env bash
set -euo pipefail

echo "Running pre-commit checks..."
cargo fmt --all -- --check
(cd web && pnpm check)
```

Create `.githooks/pre-push`:
```bash
#!/usr/bin/env bash
set -euo pipefail

echo "Running pre-push test checks..."
cargo test --all-targets
(cd web && pnpm test)
```

- [ ] **Step 3: Create `scripts/install-git-hooks.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

chmod +x scripts/check.sh
chmod +x .githooks/pre-commit .githooks/pre-push

git config core.hooksPath .githooks
echo "✅ Git hooks installed successfully (core.hooksPath set to .githooks)."
```

- [ ] **Step 4: Make executable, install hooks, and test `scripts/check.sh fast`**

Run:
```bash
chmod +x scripts/check.sh scripts/install-git-hooks.sh .githooks/pre-commit .githooks/pre-push
./scripts/install-git-hooks.sh
./scripts/check.sh fast
```
Expected: All fast checks PASS cleanly with 0 errors.

- [ ] **Step 5: Commit**

```bash
git add scripts/ .githooks/
git commit -m "feat(dev): add standardized verification script and version-controlled git hooks"
```

---

### Task 3: Team Conventions, Pull Request Template & AI Agent Rules

**Files:**
- Create: `CONTRIBUTING.md`
- Create: `.github/pull_request_template.md`
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: Project architecture and team convention decisions.
- Produces: Complete onboarding documentation for human developers and explicit behavioral constraints for AI coding agents.

- [ ] **Step 1: Create `CONTRIBUTING.md`**

Document team branch rules, commit standards, testing guidelines, and environment setup:

```markdown
# Contributing to r2drive

Thank you for contributing to `r2drive`! This document outlines our development workflows, coding conventions, and quality gates.

---

## 1. Branching Strategy

We follow a structured Git branching model:

- **`main`**: Production-ready, stable releases (e.g. `v0.1.0`, `v0.2.0`). Direct pushes are forbidden. All code enters via pull requests from `dev`.
- **`dev`**: Primary integration branch. All feature branches branch off `dev` and merge back into `dev`.
- **`feat/<feature-name>`**: Feature development (e.g. `feat/cors-diagnostic`, `feat/trash-bin`).
- **`fix/<bug-name>`**: Bug fixes (e.g. `fix/multipart-etag-validation`).

---

## 2. Developer Setup

1. **Prerequisites**:
   - Rust 1.94+ (`rustup default stable`)
   - Node.js 22+ & pnpm 9+ (`corepack enable`)
   - SQLite 3 (embedded)

2. **Initialize Local Environment**:
   ```bash
   git clone <repo-url> && cd r2drive
   ./scripts/install-git-hooks.sh
   cp config.yaml.example config.yaml
   ```
   Edit `config.yaml` with your Cloudflare R2 credentials. Note: `config.yaml` is git-ignored and must never be committed.

3. **Install Frontend Dependencies**:
   ```bash
   cd web && pnpm install && cd ..
   ```

---

## 3. Local Verification

Run automated validation before committing or pushing:

```bash
./scripts/check.sh fast   # Format check (cargo fmt), Rust Clippy, and Svelte check
./scripts/check.sh test   # Full test suite (Rust cargo test + Vitest + build)
./scripts/check.sh full   # All of the above
```

---

## 4. Commit Message Conventions

We adhere to [Conventional Commits](https://www.conventionalcommits.org/):

- `feat(scope): ...` — New feature or capability
- `fix(scope): ...` — Bug fix
- `docs(scope): ...` — Documentation updates
- `test(scope): ...` — Adding or updating test cases
- `refactor(scope): ...` — Code improvement with no behavior change
- `chore(scope): ...` — Tooling, dependency, or config updates

Examples:
- `feat(web): add 1-click CORS json copy modal`
- `fix(server): handle missing ETag in multipart complete response`

---

## 5. Pull Request Standards

1. Always target **`dev`** for feature work, not `main`.
2. Ensure GitHub Actions CI passes completely.
3. Keep pull requests focused and right-sized.
4. Fill out the PR template completely with verification evidence.
```

- [ ] **Step 2: Create `.github/pull_request_template.md`**

```markdown
## Summary of Changes

<!-- Brief 2-3 sentence overview of what this PR accomplishes and why -->

## Issue / Task Reference

<!-- Closes #123, or relates to Phase 1/Phase 2 milestone -->

## Type of Change

- [ ] `feat`: New feature or capability
- [ ] `fix`: Bug fix
- [ ] `docs`: Documentation update
- [ ] `refactor`: Code refactoring without behavior change
- [ ] `test`: Test suite additions
- [ ] `chore`: Tooling or build configuration

## Verification Checklist

- [ ] `./scripts/check.sh fast` passes cleanly (cargo fmt, clippy, svelte-check)
- [ ] `./scripts/check.sh test` passes (all Rust tests and Vitest unit tests pass)
- [ ] No credentials, `.env`, or secrets are committed
- [ ] Added or updated unit/integration tests covering the changes
- [ ] Targeted branch is `dev` (or `main` for release promotion)
```

- [ ] **Step 3: Update `AGENTS.md`**

Ensure `AGENTS.md` provides explicit rules and commands for all AI coding agents:

```markdown
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
```

- [ ] **Step 4: Verify formatting and commit**

Run: `cargo fmt --all -- --check`
Run:
```bash
git add CONTRIBUTING.md .github/pull_request_template.md AGENTS.md
git commit -m "docs: add team contributing guidelines, PR template, and agent rules"
```

---

### Task 4: GitHub Actions CI Pipeline

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Repository code, `Cargo.lock`, and `web/pnpm-lock.yaml`.
- Produces: Automated CI build and test validation on every PR and push to `main` or `dev`.

- [ ] **Step 1: Create `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [ main, dev ]
  pull_request:
    branches: [ main, dev ]

jobs:
  rust-check:
    name: Rust (Format, Clippy, Tests)
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.94.1
          components: rustfmt, clippy

      - name: Rust Cache
        uses: Swatinem/rust-cache@v2

      - name: Install Node & pnpm (for embedded asset compile check)
        uses: pnpm/action-setup@v4
        with:
          version: 9

      - name: Setup Node 22
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: 'pnpm'
          cache-dependency-path: web/pnpm-lock.yaml

      - name: Build WebConsole assets
        run: |
          cd web
          pnpm install --frozen-lockfile
          pnpm build

      - name: Check Rust formatting
        run: cargo fmt --all -- --check

      - name: Run Clippy
        run: cargo clippy --all-targets -- -D warnings

      - name: Run Backend Tests
        run: cargo test --all-targets

  frontend-check:
    name: Frontend (Svelte-check, Vitest, Vite Build)
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 9

      - name: Setup Node 22
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: 'pnpm'
          cache-dependency-path: web/pnpm-lock.yaml

      - name: Install dependencies
        run: cd web && pnpm install --frozen-lockfile

      - name: Run Svelte check
        run: cd web && pnpm check

      - name: Run Vitest unit tests
        run: cd web && pnpm test

      - name: Verify production build
        run: cd web && pnpm build

  docker-check:
    name: Docker Build
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Build Docker Image
        uses: docker/build-push-action@v5
        with:
          context: .
          push: false
          tags: r2drive:ci-test
```

- [ ] **Step 2: Validate YAML syntax**

Run: `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/ci.yml"))'`
Expected: Exits with code 0 (valid YAML).

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add GitHub Actions workflow for Rust, Frontend, and Docker validation"
```

---

### Task 5: Branch Alignment, Full Test Pass & GitHub Remote Integration

**Files:**
- Local Git branches: `feat/phase-1-core-drive`, `main`, `dev`
- Remote GitHub repo: `r2drive`

**Interfaces:**
- Consumes: Completed local commits across Tasks 1-4.
- Produces: Cleanly merged `main` (Phase 1 release baseline) and `dev` (active integration branch), pushed to GitHub with tracking.

- [ ] **Step 1: Run full local validation before branch integration**

Run: `./scripts/check.sh full`
Expected:
- All format, Clippy, and Svelte checks PASS.
- All 106 backend tests and 71 frontend tests PASS.
- Production build succeeds with 0 errors.

- [ ] **Step 2: Merge into `main` to establish Phase 1 MVP release baseline**

Run:
```bash
git checkout main
git merge feat/phase-1-core-drive --ff-only
```
Expected: Fast-forward merge succeeds, `main` now reflects the complete, verified Phase 1 codebase.

- [ ] **Step 3: Create and checkout `dev` branch**

Run:
```bash
git checkout -b dev
```
Expected: `dev` is branched from the latest commit of `main`.

- [ ] **Step 4: Create GitHub Remote Repository via `gh repo create`**

Run:
```bash
gh repo create zer0horizon/r2drive --public --source=. --remote=origin --push || gh repo create r2drive --public --source=. --remote=origin --push
```
*Note: If `zer0horizon` org is preferred or personal account is selected, `gh repo create` sets up the remote `origin`.*

- [ ] **Step 5: Push both `main` and `dev` branches to GitHub**

Run:
```bash
git push -u origin main
git push -u origin dev
```

- [ ] **Step 6: Set `dev` as default branch on GitHub**

Run:
```bash
gh repo edit --default-branch dev
```
Expected: GitHub default branch set to `dev` for team pull requests.

- [ ] **Step 7: Verify remote status**

Run:
```bash
git remote -v
git branch -vv
gh repo view
```
Expected: Remote `origin` linked, tracking configured for `dev` and `main`.
