#!/usr/bin/env bash
# A doc comment or attribute must stay on the item it was written for
# (CONTRIBUTING.md code review checklist). Usage: doc-attach.sh --base REV
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$ROOT/scripts/doc-attach.py" --root "$ROOT" "$@"
