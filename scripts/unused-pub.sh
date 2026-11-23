#!/usr/bin/env bash
# A plain `pub` item in src/ that nothing names is dead. rustc does not
# report it (CONTRIBUTING.md principle 11).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$ROOT/scripts/unused-pub.py" "$@"
