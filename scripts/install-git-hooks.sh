#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

chmod +x scripts/check.sh
chmod +x .githooks/pre-commit .githooks/pre-push

git config core.hooksPath .githooks
echo "✅ Git hooks installed successfully (core.hooksPath set to .githooks)."
