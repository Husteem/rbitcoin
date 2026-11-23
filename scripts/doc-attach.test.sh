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
assert_fail_quiet "a deleted item's doc left on an undocumented item fails" check wrapper

change wrapper-reworded <<'EOF2'
/// Hashes from the fork point to `candidate`.
pub fn connecting() {}

#[doc(hidden)]
pub fn connecting_n() {}
====
/// Hashes from the fork point to `candidate`, at most `walk_max` back.
#[doc(hidden)]
pub fn connecting_n() {}
EOF2
assert_ok "an item may take a deleted item's doc once it is reworded" check wrapper-reworded

change comment-bracket <<'EOF2'
#[cfg_attr(
    // ] is not the end
    feature = "x",
    inline
)]
fn a() {}
====
#[cfg_attr(
    // ] is not the end
    feature = "x",
    inline
)]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a bracket in a comment inside an attribute does not close it" \
  check comment-bracket

change negative-depth <<'EOF2'
#[cfg_attr(
    // ]] closes nothing
    feature = "x",
    inline
)]
fn first() {}

/// Docs for a.
fn a() {}
====
#[cfg_attr(
    // ]] closes nothing
    feature = "x",
    inline
)]
fn first() {}

/// Docs for a.
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "stray closing brackets do not hide the rest of the file" \
  check negative-depth

change multiline-string <<'EOF2'
#[doc = "start [
 end"]
fn a() {}
====
#[doc = "start [
 end"]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a bracket in a string that crosses lines does not count" \
  check multiline-string

change nested-comment <<'EOF2'
#[doc(
    /* outer /* inner */
    ] */
    hidden
)]
fn a() {}
====
#[doc(
    /* outer /* inner */
    ] */
    hidden
)]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a bracket in a nested block comment does not count" check nested-comment

change raw-backslash <<'EOF2'
#[doc = r"foo\"]
fn a() {}
====
#[doc = r"foo\"]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a backslash does not escape the end of a raw string" check raw-backslash

change raw-c-string <<'EOF2'
#[doc = cr"foo\"]
fn a() {}
====
#[doc = cr"foo\"]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a raw C string ends at its quote, as a raw string does" check raw-c-string

change raw-byte-string <<'EOF2'
#[doc = br"foo\"]
fn a() {}
====
#[doc = br"foo\"]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a raw byte string ends at its quote too" check raw-byte-string

change char-literal <<'EOF2'
#[custom(']', b']', '"')]
fn a() {}
====
#[custom(']', b']', '"')]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a bracket or quote in a char literal does not count" check char-literal

change raw-string <<'EOF2'
#[doc = r#"a "quoted" [ bracket"#]
fn a() {}
====
#[doc = r#"a "quoted" [ bracket"#]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a bracket in a raw string does not count" check raw-string

change split <<'EOF2'
/// Docs for a.
/// More about a.
fn a() {}
====
/// Docs for a.
fn b() {}

/// More about a.
fn a() {}
EOF2
assert_fail_quiet "an item inserted inside a doc block fails" check split

change block-doc <<'EOF2'
/**
 * Docs for a.
 */
fn a() {}
====
/**
 * Docs for a.
 */
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "an item inserted under a /** */ doc fails" check block-doc

change bracket-in-string <<'EOF2'
#[doc = "Array ["]
fn a() {}
====
#[doc = "Array ["]
fn b() {}

fn a() {}
EOF2
assert_fail_quiet "a bracket inside an attribute string does not hide the insert" \
  check bracket-in-string

change block-comment <<'EOF2'
/// Docs for a.
fn a() {}
====
/// Docs for a.
/* Read by the metrics page only. */
fn a() {}
EOF2
assert_ok "a block comment between a block and its item passes" check block-comment

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

change moved-file <<'EOF2'
/// Docs for a.
fn a() {}

fn filler_one() {}

fn filler_two() {}

fn filler_three() {}
====
/// Docs for a.
fn b() {}

fn a() {}

fn filler_one() {}

fn filler_two() {}

fn filler_three() {}
EOF2
git -C "$WORK/moved-file" reset -q --hard HEAD^
git -C "$WORK/moved-file" mv src/lib.rs src/moved.rs
sed -i 's|^fn a() {}$|fn b() {}\n\nfn a() {}|' "$WORK/moved-file/src/moved.rs"
git -C "$WORK/moved-file" commit -q -am after
assert_fail_quiet "an insert in a file the change also renames fails" check moved-file

if [[ "$FAIL" -ne 0 ]]; then
  echo "doc-attach.test.sh: $PASS passed, $FAIL failed"
  exit 1
fi
echo "doc-attach.test.sh: $PASS passed"
