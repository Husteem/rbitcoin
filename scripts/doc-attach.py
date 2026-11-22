#!/usr/bin/env python3
"""Fail when a change leaves a doc comment or attribute on the wrong item.

rustdoc attaches a `///` block or an attribute to whatever item follows it.
Insert a new item between an existing block and its item, or delete an item
and keep its block, and the block now describes something else. The
compiler accepts that, and rustdoc does not warn on private items.

For each Rust file that differs between BASE and HEAD, this takes every
unchanged doc or attribute line and compares the first code line under it
in both revisions. It fails when that line differs and either

- the item it used to sit on is still there and has lost it (a new item was
  inserted under the block), or
- the item it now sits on was already there with a doc of its own (the
  block's item was deleted and the block was left behind).

These pass: renaming the item under a block, rewriting an item's doc while
its old lines move to a new item, and an undocumented item taking the doc
of a deleted wrapper.

Usage: doc-attach.py --base REV [--head REV] [--root DIR]
"""

import argparse
import re
import subprocess
import sys

HUNK = re.compile(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")


def git(root: str, *args: str) -> str:
    out = subprocess.run(["git", "-C", root, *args], capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {out.stderr.strip()}")
    return out.stdout


def block_lines(lines: list[str]) -> list[bool]:
    """True for each line that is part of a doc comment or an attribute."""
    marks = [False] * len(lines)
    depth = 0
    for i, raw in enumerate(lines):
        line = raw.strip()
        if depth:
            marks[i] = True
            depth += line.count("[") - line.count("]")
            continue
        if line.startswith("///") or line.startswith("/**"):
            marks[i] = True
        elif line.startswith("#["):
            marks[i] = True
            depth = max(line.count("[") - line.count("]"), 0)
    return marks


def skipped(lines: list[str], marks: list[bool], i: int) -> bool:
    line = lines[i].strip()
    return marks[i] or not line or line.startswith("//")


def target_under(lines: list[str], marks: list[bool], i: int) -> int | None:
    """Index of the first code line below block line `i`."""
    for j in range(i + 1, len(lines)):
        if not skipped(lines, marks, j):
            return j
    return None


def block_above(lines: list[str], marks: list[bool], i: int) -> tuple[str, ...]:
    """The doc and attribute lines that attach to code line `i`."""
    block = []
    for j in range(i - 1, -1, -1):
        if not skipped(lines, marks, j):
            break
        if marks[j]:
            block.append(lines[j].strip())
    return tuple(reversed(block))


def is_doc(line: str) -> bool:
    return line.startswith("///") or line.startswith("/**")


def lost(line: str, block: tuple[str, ...]) -> bool:
    """The item that had `line` above it now has `block`. Did it lose `line`?

    An item whose doc was rewritten still has a doc, so its old lines moving
    to a new item is a hand-over, not a loss.
    """
    if is_doc(line):
        return not any(is_doc(b) for b in block)
    return line not in block


def stacked(line: str, block: tuple[str, ...]) -> bool:
    """`line` now sits above an item that had `block`. Is it a leftover?

    An item with no doc of its own may inherit the doc of a deleted wrapper.
    """
    if is_doc(line):
        return any(is_doc(b) for b in block) and line not in block
    return line not in block


def unchanged_map(root: str, base: str, head: str, path: str) -> dict[int, int]:
    """Map each unchanged HEAD line number to its BASE line number (0-based)."""
    diff = git(root, "diff", "--no-color", "-U0", base, head, "--", path)
    post_len = len(git(root, "show", f"{head}:{path}").split("\n"))
    mapping: dict[int, int] = {}
    pre = post = 0
    for line in diff.split("\n"):
        m = HUNK.match(line)
        if not m:
            continue
        pre_start, pre_n = int(m.group(1)), int(m.group(2) or 1)
        post_start, post_n = int(m.group(3)), int(m.group(4) or 1)
        # With -U0 a pure insertion names the line before it, and so does a
        # pure deletion on the other side.
        pre_first = pre_start if pre_n else pre_start + 1
        post_first = post_start if post_n else post_start + 1
        while post < post_first - 1:
            mapping[post] = pre
            pre += 1
            post += 1
        pre = pre_first - 1 + pre_n
        post = post_first - 1 + post_n
    while post < post_len:
        mapping[post] = pre
        pre += 1
        post += 1
    return mapping


def check_file(root: str, base: str, head: str, path: str) -> list[str]:
    old = git(root, "show", f"{base}:{path}").split("\n")
    new = git(root, "show", f"{head}:{path}").split("\n")
    old_marks, new_marks = block_lines(old), block_lines(new)
    to_old = unchanged_map(root, base, head, path)
    to_new = {o: n for n, o in to_old.items()}
    errors = []
    seen = set()
    for i, marked in enumerate(new_marks):
        if not marked or i not in to_old:
            continue
        was = target_under(old, old_marks, to_old[i])
        now = target_under(new, new_marks, i)
        if was is None or now is None or old[was].strip() == new[now].strip():
            continue
        if (was, now) in seen:
            continue
        line = new[i].strip()
        if was in to_new:
            if lost(line, block_above(new, new_marks, to_new[was])):
                seen.add((was, now))
                errors.append(
                    f"{path}:{i + 1}: this block sat on `{old[was].strip()}` and now sits "
                    f"on `{new[now].strip()}`. Move the new item above the block."
                )
        elif now in to_old:
            if stacked(line, block_above(old, old_marks, to_old[now])):
                seen.add((was, now))
                errors.append(
                    f"{path}:{i + 1}: this block sat on `{old[was].strip()}`, which is "
                    f"gone, and now sits on `{new[now].strip()}`. Delete the block with "
                    "its item."
                )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--root", default=".")
    args = parser.parse_args()
    names = git(
        args.root, "diff", "--name-only", "--diff-filter=M", args.base, args.head, "--", "*.rs"
    )
    errors = []
    for path in names.split():
        errors.extend(check_file(args.root, args.base, args.head, path))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
