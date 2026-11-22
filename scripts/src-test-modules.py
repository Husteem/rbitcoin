#!/usr/bin/env python3
"""Fail when `#[cfg(...test...)]` returns to a production `src/` file.

Library tests live in each crate's `tests/suite` and link the normal rlib
(TESTING.md). `rbitcoin-bench` is exempt because its `--lib` tests are the
optional host client, not a production crate.

Usage: src-test-modules.py [--root DIR]
"""

import argparse
import glob
import os
import re
import sys

CFG_START = re.compile(r"#\s*\[\s*cfg\s*\(")


def strip_comments_and_strings(src: str) -> str:
    out = []
    i = 0
    n = len(src)
    while i < n:
        if src.startswith("//", i):
            j = src.find("\n", i)
            if j < 0:
                j = n
            out.append(" " * (j - i))
            i = j
        elif src.startswith("/*", i):
            depth = 1
            i += 2
            while i < n and depth:
                if src.startswith("/*", i):
                    depth += 1
                    i += 2
                elif src.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    out.append("\n" if src[i] == "\n" else " ")
                    i += 1
        elif src[i] == '"':
            out.append(" ")
            i += 1
            while i < n:
                if src[i] == "\\" and i + 1 < n:
                    out.append("  ")
                    i += 2
                elif src[i] == '"':
                    out.append(" ")
                    i += 1
                    break
                else:
                    out.append("\n" if src[i] == "\n" else " ")
                    i += 1
        else:
            out.append(src[i])
            i += 1
    return "".join(out)


def scan(root: str):
    hits = []
    for path in glob.glob(os.path.join(root, "crates", "*", "src", "**", "*.rs"), recursive=True):
        parts = path.split(os.sep)
        if "rbitcoin-bench" in parts:
            continue
        with open(path, encoding="utf-8") as f:
            text = strip_comments_and_strings(f.read())
        for m in CFG_START.finditer(text):
            end = text.find("]", m.end())
            if end < 0:
                continue
            if re.search(r"\btest\b", text[m.end() : end]):
                rel = os.path.relpath(path, root)
                line_no = text.count("\n", 0, m.start()) + 1
                attr = " ".join(text[m.start() : end + 1].split())
                hits.append(f"{rel}:{line_no}: {attr[:120]}")
    return hits


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=None)
    args = parser.parse_args()
    root = os.path.abspath(args.root or os.path.join(os.path.dirname(__file__), ".."))
    hits = scan(root)
    if hits:
        print(
            "inline `#[cfg(test)]` in production src; move the test to the "
            "crate's tests/ suite (TESTING.md):",
            file=sys.stderr,
        )
        print("\n".join(hits), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
