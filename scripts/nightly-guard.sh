#!/usr/bin/env bash
# May this build move the `nightly` image tag? Run from the clone being
# published. For each IMAGE, reads the revision label of IMAGE:nightly with
# `docker buildx imagetools inspect` and compares it with HEAD.
#
# A late nightly (a re-run, or one GitHub queued behind a newer one) must
# not move `nightly` back in any registry. Only the tag itself missing is a
# first nightly there. Any other read error, or a revision this clone cannot
# resolve, stops here rather than guess. The caller's concurrency group
# must hold these reads and its push together.
#
# Prints `push` or `skip` on stdout and exits 0. Exits non-zero to stop the
# publish. Reasons go to stderr.
#
# Usage: ./scripts/nightly-guard.sh IMAGE...
set -euo pipefail

[[ $# -ge 1 ]] || { echo "usage: $0 IMAGE..." >&2; exit 2; }

head="$(git rev-parse HEAD)"
for i in "$@"; do
  if out="$(docker buildx imagetools inspect "$i:nightly" \
    --format '{{json (index .Image "linux/amd64").Config.Labels}}' 2>&1)"; then
    cur="$(jq -r '."org.opencontainers.image.revision" // empty' <<<"$out")"
    [[ "$cur" =~ ^[0-9a-f]{40}$ ]] || { echo "$i:nightly revision is not a commit: '$cur'" >&2; exit 1; }
    git cat-file -e "$cur^{commit}" 2>/dev/null ||
      { echo "$i:nightly names ${cur:0:12}, which this clone does not have" >&2; exit 1; }
    if [[ "$cur" != "$head" ]] && git merge-base --is-ancestor "$head" "$cur"; then
      echo "$i:nightly ${cur:0:12} is newer than ${head:0:12}; not moving it back" >&2
      echo skip
      exit 0
    fi
  elif [[ "$out" != "ERROR: $i:nightly: not found" ]]; then
    echo "cannot read $i:nightly: $out" >&2
    exit 1
  fi
done
echo push
