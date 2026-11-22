#!/usr/bin/env bash
# Production src/ carries no inline `#[cfg(test)]` modules. Library tests
# live in each crate's tests/suite and link the normal rlib (TESTING.md).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$ROOT/scripts/src-test-modules.py" "$@"
