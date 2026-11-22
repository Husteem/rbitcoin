#!/usr/bin/env bash
# Hermetic pin for scripts/nightly-guard.sh (may this build move `nightly`).
# A real git history in a temp dir. `docker` is a stub on PATH that answers
# `buildx imagetools inspect IMAGE:nightly` per registry. No registry.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GUARD="$ROOT/scripts/nightly-guard.sh"
WF="$ROOT/.github/workflows/release.yml"
PASS=0
FAIL=0
# shellcheck source=contract-assert.sh
source "$ROOT/scripts/contract-assert.sh"
export GIT_AUTHOR_NAME=rbitcoin-nightly-guard-test
export GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=rbitcoin-nightly-guard-test
export GIT_COMMITTER_EMAIL=test@example.invalid
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/rbitcoin-nightly-guard.XXXXXX")"
cleanup() { rm -rf "$WORKDIR"; }
trap cleanup EXIT

GHCR=ghcr.io/rbitcoin-org/rbitcoin
HUB=docker.io/rbitcoin/rbitcoin

# Master is A - B - C. S branches from A: a commit this clone has that is
# neither older nor newer than B.
REPO="$WORKDIR/repo"
git init -q -b master "$REPO"
commit() {
  git -C "$REPO" commit -q --allow-empty -m "$1"
  git -C "$REPO" rev-parse HEAD
}
A="$(commit A)"
B="$(commit B)"
C="$(commit C)"
git -C "$REPO" switch -q -c side "$A"
S="$(commit S)"
git -C "$REPO" switch -q master
# 40 hex that no clone has, and a short id this clone does resolve.
UNKNOWN=0123456789abcdef0123456789abcdef01234567
SHORT_A="${A:0:12}"

# The stub checks the call shape, so a changed --format cannot pass against
# canned answers written for the old one. It answers from $STUB/<image>.
BIN="$WORKDIR/bin"
STUB="$WORKDIR/stub"
mkdir -p "$BIN"
cat >"$BIN/docker" <<'EOF'
#!/usr/bin/env bash
set -u
printf '%s\n' "$4" >>"$NIGHTLY_GUARD_STUB/calls"
format='{{json (index .Image "linux/amd64").Config.Labels}}'
if [[ $# -ne 6 || "$1 $2 $3 $5" != "buildx imagetools inspect --format" || "$6" != "$format" ]]; then
  echo "docker stub: unexpected call: $*" >&2
  exit 97
fi
key="${4//[\/:]/_}"
[[ -f "$NIGHTLY_GUARD_STUB/$key.rc" ]] || { echo "docker stub: no answer for $4" >&2; exit 98; }
rc="$(cat "$NIGHTLY_GUARD_STUB/$key.rc")"
if [[ "$rc" == 0 ]]; then
  cat "$NIGHTLY_GUARD_STUB/$key.out"
else
  cat "$NIGHTLY_GUARD_STUB/$key.out" >&2
fi
exit "$rc"
EOF
chmod +x "$BIN/docker"
export NIGHTLY_GUARD_STUB="$STUB"

# answer IMAGE RC TEXT: what inspect of IMAGE:nightly prints, and its exit.
answer() {
  local key="${1//[\/:]/_}_nightly"
  printf '%s\n' "$3" >"$STUB/$key.out"
  printf '%s\n' "$2" >"$STUB/$key.rc"
}
# The Dockerfile's labels, with REVISION as given.
published() {
  answer "$1" 0 "$(printf '{"org.opencontainers.image.description":"Experimental Bitcoin full node (rbitcoin-node) and CLI","org.opencontainers.image.licenses":"MIT OR Apache-2.0","org.opencontainers.image.revision":"%s","org.opencontainers.image.source":"https://github.com/rbitcoin-org/rbitcoin","org.opencontainers.image.title":"rbitcoin","org.opencontainers.image.version":"nightly-20261111"}' "$2")"
}
# buildx's line when the tag itself does not exist.
missing() { answer "$1" 1 "ERROR: $1:nightly: not found"; }
fresh() {
  rm -rf "$STUB"
  mkdir -p "$STUB"
  : >"$STUB/calls"
}

# check NAME WANT NEEDLE HEAD IMAGE...: run the guard at HEAD against the
# answers set since fresh. WANT is push, skip, or stop. NEEDLE must be in
# its stderr, so a stop for the wrong reason (a stub miss) does not pass.
# An empty NEEDLE wants empty stderr.
check() {
  local name="$1" want="$2" needle="$3" head="$4" out rc=0 got said
  shift 4
  git -C "$REPO" checkout -q --detach "$head"
  out="$(cd "$REPO" && PATH="$BIN:$PATH" bash "$GUARD" "$@" 2>"$WORKDIR/err")" || rc=$?
  if [[ "$rc" -ne 0 ]]; then
    got=stop
  elif [[ "$out" == push || "$out" == skip ]]; then
    got="$out"
  else
    got="exit 0 with '$out'"
  fi
  if [[ -n "$needle" ]]; then
    grep -q -F -- "$needle" "$WORKDIR/err" && said=1 || said=0
  else
    [[ ! -s "$WORKDIR/err" ]] && said=1 || said=0
  fi
  if [[ "$got" == "$want" && "$said" == 1 ]]; then
    echo "ok - $name"
    PASS=$((PASS + 1))
  else
    echo "not ok - $name (want $want, got $got; stderr: $(tr '\n' ' ' <"$WORKDIR/err"))"
    FAIL=$((FAIL + 1))
  fi
}
read_both() {
  assert_ok "$1 reads both registries" \
    test "$(cat "$STUB/calls")" = "$(printf '%s\n%s' "$GHCR:nightly" "$HUB:nightly")"
}

assert_ok "script exists" test -f "$GUARD"

# Re-runs keep their run's SHA (GITHUB_RUN_ATTEMPT > 1). A first attempt
# can also be late: GitHub orders a concurrency group by when a run starts
# waiting, not by trigger time. The guard must not care which.
export GITHUB_RUN_ATTEMPT=1

fresh; missing "$GHCR"; missing "$HUB"
check "first nightly ever: tag missing in every registry" push "" "$B" "$GHCR" "$HUB"
read_both "first nightly"

fresh; missing "$GHCR"
check "first nightly, GHCR only" push "" "$B" "$GHCR"

fresh; published "$GHCR" "$B"; published "$HUB" "$B"
check "newer commit moves nightly forward" push "" "$C" "$GHCR" "$HUB"
read_both "newer commit"

# Pins today's behavior: the same commit republishes. The guard only
# refuses a move back.
GITHUB_RUN_ATTEMPT=2
fresh; published "$GHCR" "$B"; published "$HUB" "$B"
check "same commit (re-run of the current nightly) republishes" push "" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "$C"; published "$HUB" "$C"
check "older commit: re-run after master moved is skipped" skip "$GHCR:nightly ${C:0:12} is newer than ${B:0:12}" "$B" "$GHCR" "$HUB"
GITHUB_RUN_ATTEMPT=1

fresh; published "$GHCR" "$C"; published "$HUB" "$C"
check "older commit: late first attempt is skipped" skip "$GHCR:nightly ${C:0:12} is newer than ${B:0:12}" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "$C"
check "older commit, GHCR only, is skipped" skip "$GHCR:nightly ${C:0:12} is newer than ${B:0:12}" "$B" "$GHCR"

# Pins today's behavior: a commit that is neither older nor newer (master
# rewritten) is overwritten. The guard only refuses a move back.
fresh; published "$GHCR" "$S"; published "$HUB" "$S"
check "commit off master's line is overwritten" push "" "$B" "$GHCR" "$HUB"

# Revisions the guard cannot place.
fresh; published "$GHCR" "$UNKNOWN"; published "$HUB" "$B"
check "unknown 40-hex revision stops" stop "$GHCR:nightly names ${UNKNOWN:0:12}, which this clone does not have" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "$SHORT_A"; published "$HUB" "$B"
check "short commit id as revision stops" stop "$GHCR:nightly revision is not a commit: '$SHORT_A'" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" master; published "$HUB" "$B"
check "branch name as revision stops" stop "$GHCR:nightly revision is not a commit: 'master'" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "$A~0"; published "$HUB" "$B"
check "commit id with a suffix git would resolve stops" stop "$GHCR:nightly revision is not a commit: '$A~0'" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "x$A"; published "$HUB" "$B"
check "commit id with a prefix stops" stop "$GHCR:nightly revision is not a commit: 'x$A'" "$B" "$GHCR" "$HUB"

fresh
answer "$GHCR" 0 '{"org.opencontainers.image.title":"rbitcoin","org.opencontainers.image.version":"nightly-20261111"}'
published "$HUB" "$B"
check "no revision label stops" stop "$GHCR:nightly revision is not a commit: ''" "$B" "$GHCR" "$HUB"

fresh; answer "$GHCR" 0 null; published "$HUB" "$B"
check "no labels at all stops" stop "$GHCR:nightly revision is not a commit: ''" "$B" "$GHCR" "$HUB"

# Reads that fail for another reason than the tag missing.
fresh; missing "$HUB"
answer "$GHCR" 1 'ERROR: failed to authorize: failed to fetch anonymous token: unexpected status from GET request to https://ghcr.io/token?scope=repository%3Arbitcoin-org%2Frbitcoin%3Apull&service=ghcr.io: 401 Unauthorized'
check "registry auth failure stops" stop "cannot read $GHCR:nightly: ERROR: failed to authorize" "$B" "$GHCR" "$HUB"

fresh; missing "$HUB"
answer "$GHCR" 1 'ERROR: failed to do request: Head "https://ghcr.io/v2/rbitcoin-org/rbitcoin/manifests/nightly": dial tcp: lookup ghcr.io on 127.0.0.53:53: i/o timeout'
check "registry network failure stops" stop "cannot read $GHCR:nightly: ERROR: failed to do request" "$B" "$GHCR" "$HUB"

# The tag exists but a manifest under it does not. Both end in ": not
# found", which is not the tag missing.
fresh; missing "$HUB"
answer "$GHCR" 1 "ERROR: content at https://ghcr.io/v2/rbitcoin-org/rbitcoin/manifests/sha256:$UNKNOWN$UNKNOWN not found: not found"
check "missing child manifest under an existing tag stops" stop "cannot read $GHCR:nightly: ERROR: content at" "$B" "$GHCR" "$HUB"

fresh; missing "$HUB"
answer "$GHCR" 1 "ERROR: $GHCR@sha256:${UNKNOWN}${UNKNOWN:0:24}: not found"
check "missing child manifest by digest stops" stop "cannot read $GHCR:nightly: ERROR: $GHCR@sha256:" "$B" "$GHCR" "$HUB"

fresh; missing "$GHCR"
answer "$HUB" 1 "ERROR: $GHCR:nightly: not found"
check "another image's missing tag is not this one's" stop "cannot read $HUB:nightly: ERROR: $GHCR:nightly: not found" "$B" "$GHCR" "$HUB"

# Registries that disagree. The push retags every registry, so any one
# newer skips it, and any one unreadable stops it.
fresh; published "$GHCR" "$A"; published "$HUB" "$C"
check "GHCR older, Docker Hub newer: skipped" skip "$HUB:nightly ${C:0:12} is newer" "$B" "$GHCR" "$HUB"

fresh; missing "$GHCR"; published "$HUB" "$C"
check "GHCR missing, Docker Hub newer: skipped" skip "$HUB:nightly ${C:0:12} is newer" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "$C"; missing "$HUB"
check "GHCR newer, Docker Hub missing: skipped" skip "$GHCR:nightly ${C:0:12} is newer" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "$A"; published "$HUB" "$B"
check "GHCR older, Docker Hub same: pushed" push "" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "$A"
answer "$HUB" 1 'ERROR: failed to do request: Head "https://registry-1.docker.io/v2/rbitcoin/rbitcoin/manifests/nightly": net/http: TLS handshake timeout'
check "GHCR older, Docker Hub unreadable: stops" stop "cannot read $HUB:nightly: ERROR: failed to do request" "$B" "$GHCR" "$HUB"

fresh; published "$GHCR" "$A"; published "$HUB" "$UNKNOWN"
check "GHCR older, Docker Hub unknown revision: stops" stop "$HUB:nightly names ${UNKNOWN:0:12}, which this clone does not have" "$B" "$GHCR" "$HUB"

fresh
check "no image is a usage error" stop "usage:" "$B"

# The workflow runs this script and has no inline copy beside it.
# shellcheck disable=SC2016 # literal workflow text
assert_ok "release.yml calls nightly-guard.sh" \
  grep -q -F 'verdict="$(./scripts/nightly-guard.sh "${images[@]}")"' "$WF"
# shellcheck disable=SC2016 # literal workflow text
assert_fail "release.yml has no inline nightly read" \
  grep -q -F 'imagetools inspect "$i:nightly"' "$WF"

echo
echo "passed=$PASS failed=$FAIL"
test "$FAIL" -eq 0
