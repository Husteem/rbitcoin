#!/usr/bin/env bash
# Contract: production src/ carries no inline `#[cfg(...test...)]` items.
# Fixtures under lint/src-test-modules/fixtures. Does not scan the repo.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="$ROOT/scripts/src-test-modules.py"
FIX="$ROOT/lint/src-test-modules/fixtures"
PASS=0
FAIL=0
source "$ROOT/scripts/contract-assert.sh"

assert_fail "a cfg(test) module in src fails" \
  python3 "$PY" --root "$FIX/bare"
assert_fail "cfg(all(test, ...)) in src fails" \
  python3 "$PY" --root "$FIX/cfg-all"
assert_fail "a wrapped cfg(test) attribute fails" \
  python3 "$PY" --root "$FIX/cfg-wrapped"
assert_ok "comments and strings are not test modules" \
  python3 "$PY" --root "$FIX/comment"
assert_ok "debug_assertions hooks are not test modules" \
  python3 "$PY" --root "$FIX/outside"
assert_ok "the same attribute under tests/ is not scanned" \
  python3 "$PY" --root "$FIX/in-tests"
assert_ok "rbitcoin-bench keeps its --lib tests" \
  python3 "$PY" --root "$FIX/bench"

if [[ "$FAIL" -ne 0 ]]; then
  echo "src-test-modules.test.sh: $PASS passed, $FAIL failed"
  exit 1
fi
echo "src-test-modules.test.sh: $PASS passed"
