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
- the item it now sits on was already there (the block's item was deleted
  and the block was left behind).

These pass: renaming the item under a block, and rewriting an item's doc
while its old lines move to a new item. An item that takes over a deleted
item's doc passes once that doc is reworded for it.

Usage: doc-attach.py --base REV [--head REV] [--root DIR]
"""

import argparse
import re
import subprocess
import sys

HUNK = re.compile(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")
RAW_OPEN = re.compile(r'r(#*)"')


def git(root: str, *args: str) -> str:
    out = subprocess.run(["git", "-C", root, *args], capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {out.stderr.strip()}")
    return out.stdout


def bracket_delta(line: str, text: str | None) -> tuple[int, str | None]:
    """Net `[` minus `]` in the code on `line`, and the string left open.

    `text` is the terminator of a string that an earlier line left open:
    `"` for an ordinary string, `"#`.. for a raw one. Comments and strings
    hold no brackets that count.
    """
    delta = 0
    i = 0
    n = len(line)
    while i < n:
        if text is not None:
            if text == '"' and line[i] == "\\":
                i += 2
                continue
            if line.startswith(text, i):
                i += len(text)
                text = None
                continue
            i += 1
            continue
        if line.startswith("//", i):
            break
        raw = RAW_OPEN.match(line, i)
        if raw:
            text = '"' + raw.group(1)
            i = raw.end()
        elif line[i] == '"':
            text = '"'
            i += 1
        else:
            delta += (line[i] == "[") - (line[i] == "]")
            i += 1
    return delta, text


def classify(lines: list[str]) -> tuple[list[bool], list[bool]]:
    """Per line: is it a doc comment or attribute, and is it a plain comment."""
    marks = [False] * len(lines)
    plain = [False] * len(lines)
    depth = 0
    text: str | None = None
    in_block: str | None = None
    for i, raw in enumerate(lines):
        line = raw.strip()
        if in_block:
            (marks if in_block == "doc" else plain)[i] = True
            if "*/" in line:
                in_block = None
            continue
        if depth > 0 or text is not None:
            marks[i] = True
            delta, text = bracket_delta(line, text)
            depth = max(depth + delta, 0)
            continue
        if line.startswith("///"):
            marks[i] = True
        elif line.startswith("/*"):
            doc = line.startswith("/**") and not line.startswith("/***")
            (marks if doc else plain)[i] = True
            if "*/" not in line[2:]:
                in_block = "doc" if doc else "plain"
        elif line.startswith("//"):
            plain[i] = True
        elif line.startswith("#["):
            marks[i] = True
            delta, text = bracket_delta(line, None)
            depth = max(delta, 0)
    return marks, plain


class Side:
    def __init__(self, text: str):
        self.lines = text.split("\n")
        self.marks, self.plain = classify(self.lines)

    def skipped(self, i: int) -> bool:
        return self.marks[i] or self.plain[i] or not self.lines[i].strip()

    def target_under(self, i: int) -> int | None:
        """Index of the first code line below block line `i`."""
        for j in range(i + 1, len(self.lines)):
            if not self.skipped(j):
                return j
        return None

    def block_above(self, i: int) -> list[int]:
        """Indexes of the doc and attribute lines that attach to code line `i`."""
        block = []
        for j in range(i - 1, -1, -1):
            if not self.skipped(j):
                break
            if self.marks[j]:
                block.append(j)
        return block[::-1]

    def text(self, i: int) -> str:
        return self.lines[i].strip()


def is_doc(line: str) -> bool:
    return line.startswith("///") or line.startswith("/**") or line.startswith("*")


def unchanged_map(root: str, old_blob: str, new_blob: str, new_len: int) -> dict[int, int]:
    """Map each unchanged new line number to its old line number (0-based)."""
    out = subprocess.run(
        ["git", "-C", root, "diff", "--no-color", "-U0", old_blob, new_blob],
        capture_output=True,
        text=True,
    )
    mapping: dict[int, int] = {}
    pre = post = 0
    for line in out.stdout.split("\n"):
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
    while post < new_len:
        mapping[post] = pre
        pre += 1
        post += 1
    return mapping


def check_file(root: str, base: str, head: str, old_path: str, path: str) -> list[str]:
    old_blob, new_blob = f"{base}:{old_path}", f"{head}:{path}"
    old, new = Side(git(root, "show", old_blob)), Side(git(root, "show", new_blob))
    to_old = unchanged_map(root, old_blob, new_blob, len(new.lines))
    to_new = {o: n for n, o in to_old.items()}
    errors = []
    seen = set()
    for i, marked in enumerate(new.marks):
        if not marked or i not in to_old:
            continue
        was, now = old.target_under(to_old[i]), new.target_under(i)
        if was is None or now is None or old.text(was) == new.text(now):
            continue
        if (was, now) in seen:
            continue
        line = new.text(i)
        if was in to_new:
            # The item the line sat on is still there. It lost the line unless
            # its doc was rewritten: then every doc line it has is a new one.
            block = new.block_above(to_new[was])
            texts = [new.text(j) for j in block]
            docs = [j for j in block if is_doc(new.text(j))]
            handed_over = docs and not any(j in to_old for j in docs)
            if line not in texts and not (is_doc(line) and handed_over):
                seen.add((was, now))
                errors.append(
                    f"{path}:{i + 1}: this block sat on `{old.text(was)}` and now sits "
                    f"on `{new.text(now)}`. Move the new item above the block."
                )
        elif now in to_old:
            # The line's item is gone and the line fell on the next item.
            if line not in [old.text(j) for j in old.block_above(to_old[now])]:
                seen.add((was, now))
                errors.append(
                    f"{path}:{i + 1}: this block sat on `{old.text(was)}`, which is "
                    f"gone, and now sits on `{new.text(now)}`. Delete the block with "
                    "its item."
                )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--root", default=".")
    args = parser.parse_args()
    status = git(
        args.root, "diff", "--name-status", "-M", "--diff-filter=MR", args.base, args.head,
        "--", "*.rs",
    )
    errors = []
    for row in status.splitlines():
        fields = row.split("\t")
        errors.extend(check_file(args.root, args.base, args.head, fields[1], fields[-1]))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
