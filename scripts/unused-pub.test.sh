#!/usr/bin/env bash
# Contract: a plain `pub` item in src/ that nothing else names fails.
# Fixtures under lint/unused-pub/fixtures. Does not scan the repo.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="$ROOT/scripts/unused-pub.py"
FIX="$ROOT/lint/unused-pub/fixtures"
PASS=0
FAIL=0
source "$ROOT/scripts/contract-assert.sh"

names() {
  local out
  out="$(python3 "$PY" --root "$FIX/$1" 2>&1 || true)"
  grep -qF -- "$2" <<<"$out"
}

assert_fail_quiet "a pub fn nothing calls fails" \
  python3 "$PY" --root "$FIX/dead"
assert_ok "the error names the item, its line, and the fix" \
  names dead 'crates/a/src/lib.rs:1: `pub fn gone` is named nowhere else. Delete it'
assert_ok "a caller in another crate keeps it" \
  python3 "$PY" --root "$FIX/other-crate"
assert_ok "a caller under tests/ keeps it" \
  python3 "$PY" --root "$FIX/in-tests"
assert_ok "a caller under fuzz/ keeps it" \
  python3 "$PY" --root "$FIX/in-fuzz"
assert_ok "a use in its own file keeps it" \
  python3 "$PY" --root "$FIX/same-file"
assert_fail_quiet "a comment, a doc link, and a string are not uses" \
  python3 "$PY" --root "$FIX/comment"
assert_ok "a name captured by a format string is a use" \
  python3 "$PY" --root "$FIX/format-capture"
assert_ok "a path an attribute names as a string is a use" \
  python3 "$PY" --root "$FIX/attribute-path"
assert_ok "pub(crate) is left to rustc" \
  python3 "$PY" --root "$FIX/crate-private"
assert_fail_quiet "two unused items that share a name both fail" \
  python3 "$PY" --root "$FIX/shared-name"
assert_ok "one use of a shared name keeps both (name match, not resolution)" \
  python3 "$PY" --root "$FIX/shared-name-live"
for kind in 'fn a_const_fn' 'fn an_async_fn' 'fn an_unsafe_fn' 'static A_STATIC' \
  'type AType' 'trait ATrait' 'enum AnEnum' 'union AUnion'; do
  assert_ok "pub $kind is scanned" names kinds "\`pub $kind\`"
done

if [[ "$FAIL" -ne 0 ]]; then
  echo "unused-pub.test.sh: $PASS passed, $FAIL failed"
  exit 1
fi
echo "unused-pub.test.sh: $PASS passed"
