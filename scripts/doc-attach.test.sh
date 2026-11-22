#!/usr/bin/env bash
# Contract: a change that leaves a doc comment or attribute on another item
# fails. Each case commits a before and an after file in a scratch repo.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="$ROOT/scripts/doc-attach.py"
PASS=0
FAIL=0
source "$ROOT/scripts/contract-assert.sh"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# change NAME: reads the before file, then `====`, then the after file, on stdin.
change() {
  local repo="$WORK/$1" text
  text="$(cat)"
  mkdir -p "$repo/src"
  git -C "$repo" init -q
  git -C "$repo" config user.name t
  git -C "$repo" config user.email t@example.invalid
  git -C "$repo" config commit.gpgsign false
  printf '%s\n' "${text%%$'\n'====*}" >"$repo/src/lib.rs"
  git -C "$repo" add -A
  git -C "$repo" commit -q -m before
  printf '%s\n' "${text#*====$'\n'}" >"$repo/src/lib.rs"
  git -C "$repo" commit -q -am after
}

check() {
  python3 "$PY" --root "$WORK/$1" --base HEAD^
}

says() {
  local out
  out="$(check "$1" 2>&1 || true)"
  grep -qF -- "$2" <<<"$out"
}

change insert <<'EOF2'
/// Takes the orphans that wait on `parent`.
fn take_orphan_children() {}
====
/// Takes the orphans that wait on `parent`.
fn orphans_waiting_only_on() {}

fn take_orphan_children() {}
EOF2
assert_fail_quiet "an item inserted under another item's doc fails" check insert
assert_ok "the error names both items and the fix" says insert \
  'src/lib.rs:1: this block sat on `fn take_orphan_children() {}` and now sits on `fn orphans_waiting_only_on() {}`. Move the new item above the block.'

change delete <<'EOF2'
/// Median of the last eleven block times.
fn median_time_past() {}

/// Bits the next header must carry.
fn next_work_bits() {}
====
/// Median of the last eleven block times.
/// Bits the next header must carry.
fn next_work_bits() {}
EOF2
assert_fail_quiet "a doc left behind by a deleted item fails" check delete
assert_ok "the error says to delete the block with its item" says delete \
  'which is gone, and now sits on `fn next_work_bits() {}`. Delete the block with its item.'

change attribute <<'EOF2'
#[doc(hidden)]
pub fn parse_operator_flags() {}
====
#[doc(hidden)]
fn print_stdout() {}

pub fn parse_operator_flags() {}
EOF2
assert_fail_quiet "an attribute that moved to an inserted item fails" check attribute

change field <<'EOF2'
struct Row {
    /// Bytes on disk.
    len: u64,
}
====
struct Row {
    /// Bytes on disk.
    cap: u64,
    len: u64,
}
EOF2
assert_fail_quiet "a field inserted under another field's doc fails" check field

change hand-over <<'EOF2'
/// Parses hex and reverses it.
///
/// `0x` is stripped here.
fn parse_display() {}
====
/// Parses hex as written.
///
/// `0x` is stripped here.
fn parse_plain() {}

/// Parses hex and reverses it.
///
/// Same input rules as `parse_plain`.
fn parse_display() {}
EOF2
assert_ok "an item whose doc is rewritten may hand lines to a new item" check hand-over

change wrapper <<'EOF2'
/// Hashes from the fork point to `candidate`.
pub fn connecting() {}

#[doc(hidden)]
pub fn connecting_n() {}
====
/// Hashes from the fork point to `candidate`.
#[doc(hidden)]
pub fn connecting_n() {}
EOF2
assert_ok "an undocumented item may take a deleted wrapper's doc" check wrapper

change rename <<'EOF2'
/// Bits the next header must carry.
fn next_bits() {}
====
/// Bits the next header must carry.
fn next_work_bits() {}
EOF2
assert_ok "renaming the item under its doc passes" check rename

change above <<'EOF2'
/// Bits the next header must carry.
fn next_work_bits() {}
====
fn helper() {}

/// Bits the next header must carry.
fn next_work_bits() {}
EOF2
assert_ok "an item inserted above the block passes" check above

change both-gone <<'EOF2'
/// Median of the last eleven block times.
fn median_time_past() {}

/// Bits the next header must carry.
fn next_work_bits() {}
====
/// Bits the next header must carry.
fn next_work_bits() {}
EOF2
assert_ok "deleting an item with its block passes" check both-gone

change same-attribute <<'EOF2'
#[test]
fn first() {}

#[test]
fn second() {}

#[test]
fn third() {}
====
#[test]
fn first() {}

#[test]
fn third() {}
EOF2
assert_ok "deleting one of several items that carry the same attribute passes" \
  check same-attribute

change comment <<'EOF2'
#[doc(hidden)]
pub fn totals() {}
====
#[doc(hidden)]
// Read by the metrics page only.
pub fn totals() {}
EOF2
assert_ok "a plain comment between a block and its item passes" check comment

change longer-doc <<'EOF2'
/// Bits the next header must carry.
fn next_work_bits() {}
====
/// Bits the next header must carry.
/// Zero below the first retarget.
fn next_work_bits() {}
EOF2
assert_ok "adding a line to the block passes" check longer-doc

if [[ "$FAIL" -ne 0 ]]; then
  echo "doc-attach.test.sh: $PASS passed, $FAIL failed"
  exit 1
fi
echo "doc-attach.test.sh: $PASS passed"
