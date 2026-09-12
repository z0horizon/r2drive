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
