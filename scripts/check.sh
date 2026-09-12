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
