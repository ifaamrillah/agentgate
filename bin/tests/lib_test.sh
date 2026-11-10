#!/usr/bin/env bash
# Tes bin/_lib.sh: helper port dan gagal-tertutup env loader.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }

# shellcheck disable=SC1091
. "$ROOT/bin/_lib.sh"

# ag_free_port melewati port yang sedang di-listen.
PORT=$(ag_free_port 47310)
python3 -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT
for _ in 1 2 3 4 5 6 7 8 9 10; do ag_port_in_use "$PORT" && break; sleep 0.2; done
ag_port_in_use "$PORT" || fail "listener tidak naik"
GOT=$(ag_free_port "$PORT")
[ "$GOT" != "$PORT" ] || fail "ag_free_port mengembalikan port terpakai $PORT"

# ag_load_env di dalam worktrees/ tanpa .worktree harus berhenti.
TMP="$(mktemp -d)"
mkdir -p "$TMP/repo/worktrees/x/bin"
cp "$ROOT/bin/_lib.sh" "$TMP/repo/worktrees/x/bin/"
if (. "$TMP/repo/worktrees/x/bin/_lib.sh"; ag_load_env) 2>/dev/null; then
  fail "ag_load_env tidak berhenti tanpa .worktree"
fi

# Dengan .worktree: database dan project compose diturunkan dari id.
printf 'WORKTREE_ID=feature_x\nWORKTREE_PORT=5151\n' >"$TMP/repo/worktrees/x/.worktree"
OUT=$(. "$TMP/repo/worktrees/x/bin/_lib.sh"; ag_load_env; echo "$AG_DB_DEV $AG_DB_TEST $COMPOSE_PROJECT_NAME $AG_PREFERRED_PORT $SESSION_COOKIE_SUFFIX")
[ "$OUT" = "agentgate_dev_feature_x agentgate_test_feature_x agentgate-rs-feature-x 5151 _feature_x" ] || fail "env worktree salah: $OUT"
rm -rf "$TMP"
echo "lib_test: ok"
