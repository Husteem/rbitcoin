#!/usr/bin/env python3
"""Fail when a plain `pub` item in a crate's `src/` is named nowhere else.

rustc reports a dead private item. It does not report a dead `pub` item in
a library crate, so a change can remove the last caller and leave the item
behind (CONTRIBUTING.md principle 11). This script counts every identifier
in the Rust files under `crates/`, `fuzz/`, and `script-oracle/`. A `pub`
fn, const, static, struct, enum, trait, type, or union whose name appears
only where it is declared has no caller in production or in a test.

Comments and strings do not count as a use, with two exceptions read from
the raw text: a name inside format braces (`"{NAME}"`, `"{:NAME$}"`), and a
path assigned as a string, as in `#[serde(serialize_with = "path")]`.

What this cannot see. The match is by name, with no name resolution, so an
item that shares its name with a live one is not reported, and neither is
one that only a test, a re-export, or a brace in a comment still names.
`pub(crate)` and narrower are rustc's to report.

Usage: unused-pub.py [--root DIR]
"""

import argparse
import collections
import importlib.util
import os
import re
import sys

SCAN_DIRS = ("crates", "fuzz", "script-oracle")
PUB_ITEM = re.compile(
    r"^[ \t]*pub[ \t]+(?:(?:const|async|unsafe|extern)[ \t]+)*"
    r"(fn|struct|enum|trait|type|const|static|union)[ \t]+(?:mut[ \t]+)?"
    r"(?:r#)?([A-Za-z_][A-Za-z0-9_]*)",
    re.M,
)
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
FORMAT_CAPTURE = re.compile(
    r"\{([A-Za-z_][A-Za-z0-9_]*)[:}]|[:.]([A-Za-z_][A-Za-z0-9_]*)\$"
)
STRING_PATH = re.compile(
    r"=\s*r?#*\"(?:[A-Za-z_][A-Za-z0-9_]*::)*([A-Za-z_][A-Za-z0-9_]*)\""
)


def load_unwrap():
    path = os.path.join(os.path.dirname(__file__), "unwrap-ratchet.py")
    spec = importlib.util.spec_from_file_location("unwrap_ratchet", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def rust_files(root: str):
    for top in SCAN_DIRS:
        for dirpath, dirnames, filenames in os.walk(os.path.join(root, top)):
            dirnames[:] = [d for d in dirnames if d != "target"]
            for name in filenames:
                if name.endswith(".rs"):
                    yield os.path.join(dirpath, name)


def unused(root: str) -> list[str]:
    blank_non_code = load_unwrap().blank_non_code
    uses: collections.Counter[str] = collections.Counter()
    declared: dict[str, list[tuple[str, int, str]]] = collections.defaultdict(list)
    for path in rust_files(root):
        with open(path, encoding="utf-8") as f:
            src = f.read()
        code = blank_non_code(src)
        uses.update(IDENT.findall(code))
        for braced, sized in FORMAT_CAPTURE.findall(src):
            uses[braced or sized] += 1
        uses.update(STRING_PATH.findall(src))
        rel = os.path.relpath(path, root).replace(os.sep, "/")
        parts = rel.split("/")
        if parts[0] != "crates" or "src" not in parts:
            continue
        for m in PUB_ITEM.finditer(code):
            line = code.count("\n", 0, m.start()) + 1
            declared[m.group(2)].append((rel, line, m.group(1)))
    errors = []
    for name, sites in declared.items():
        if uses[name] > len(sites):
            continue
        for rel, line, kind in sites:
            errors.append(
                f"{rel}:{line}: `pub {kind} {name}` is named nowhere else. "
                "Delete it, or drop `pub` if an attribute is its only user "
                "(CONTRIBUTING.md principle 11)."
            )
    return sorted(errors)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=None)
    args = parser.parse_args()
    root = os.path.abspath(args.root or os.path.join(os.path.dirname(__file__), ".."))
    errors = unused(root)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
