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
import importlib.util
import os
import re
import subprocess
import sys

HUNK = re.compile(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")
RAW_OPEN = re.compile(r'(?:br|rb|r)(#*)"')


def git(root: str, *args: str) -> str:
    out = subprocess.run(["git", "-C", root, *args], capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {out.stderr.strip()}")
    return out.stdout


def load_unwrap():
    path = os.path.join(os.path.dirname(__file__), "unwrap-ratchet.py")
    spec = importlib.util.spec_from_file_location("unwrap_ratchet", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# Past a char literal that starts at an index, or None for a lifetime.
char_end = load_unwrap()._char_end


def code_only(src: str) -> str:
    """`src` with comments, strings, and char literals spaced out.

    Newlines stay, so line numbers hold. A string may cross lines, and block
    comments nest.
    """
    out = list(src)
    n = len(src)

    def blank(start: int, end: int) -> int:
        for k in range(start, min(end, n)):
            if out[k] != "\n":
                out[k] = " "
        return end

    i = 0
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            end = src.find("\n", i)
            i = blank(i, n if end < 0 else end)
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            i = blank(i, j)
        elif c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            i = blank(i, j + 1)
        elif c in "rb" and not (i and (src[i - 1].isalnum() or src[i - 1] == "_")):
            raw = RAW_OPEN.match(src, i)
            if raw:
                closer = '"' + raw.group(1)
                end = src.find(closer, raw.end())
                i = blank(i, n if end < 0 else end + len(closer))
            else:
                i += 1
        elif c == "'":
            end = char_end(src, i)
            i = blank(i, end) if end is not None else i + 1
        else:
            i += 1
    return "".join(out)


def classify(text: str) -> list[str | None]:
    """Per line: "doc", "attr", "comment" (a plain one), or None for code."""
    raw_lines = text.split("\n")
    code_lines = code_only(text).split("\n")
    kinds: list[str | None] = [None] * len(raw_lines)
    attr_depth = 0
    doc_depth = 0
    for i, raw in enumerate(raw_lines):
        line = raw.strip()
        code = code_lines[i].strip()
        if doc_depth:
            kinds[i] = "doc"
            doc_depth += line.count("/*") - line.count("*/")
        elif attr_depth:
            kinds[i] = "attr"
            attr_depth = max(attr_depth + code.count("[") - code.count("]"), 0)
        elif line.startswith("///") and not line.startswith("////"):
            kinds[i] = "doc"
        elif line.startswith("/**") and not line.startswith(("/***", "/**/")):
            kinds[i] = "doc"
            doc_depth = line.count("/*") - line.count("*/")
        elif line and not code:
            kinds[i] = "comment"
        elif code.startswith("#["):
            kinds[i] = "attr"
            attr_depth = max(code.count("[") - code.count("]"), 0)
    return kinds


class Side:
    def __init__(self, text: str):
        self.lines = text.split("\n")
        self.kinds = classify(text)

    def skipped(self, i: int) -> bool:
        return self.kinds[i] is not None or not self.lines[i].strip()

    def in_block(self, i: int) -> bool:
        return self.kinds[i] in ("doc", "attr")

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
            if self.in_block(j):
                block.append(j)
        return block[::-1]

    def text(self, i: int) -> str:
        return self.lines[i].strip()


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
    for i in range(len(new.lines)):
        if not new.in_block(i) or i not in to_old:
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
            docs = [j for j in block if new.kinds[j] == "doc"]
            handed_over = docs and not any(j in to_old for j in docs)
            kept = line in [new.text(j) for j in block]
            if not kept and not (new.kinds[i] == "doc" and handed_over):
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
