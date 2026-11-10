#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }
id() { "$ROOT/bin/worktree-id" "$@"; }

[ "$(id feature/m0-fondasi)" = "feature_m0_fondasi" ] || fail "id dasar"
[ "$(id Fix/UPPER.case)" = "fix_upper_case" ] || fail "huruf besar dan titik"
[ "$(id --folder feature/m0-fondasi)" = "feature-m0-fondasi" ] || fail "folder"
LONG="feature/$(printf 'a%.0s' $(seq 1 60))"
A="$(id "$LONG")"; B="$(id "$LONG")"
[ "${#A}" -eq 38 ] || fail "panjang id panjang = ${#A}"
[ "$A" = "$B" ] || fail "id tidak stabil"
[ "$(id "${LONG}x")" != "$A" ] || fail "dua branch panjang berbeda menghasilkan id sama"
for bad in staging release main production "bad..ref" "-x"; do
  if id "$bad" >/dev/null 2>&1; then fail "'$bad' diterima"; fi
done
echo "worktree_id_test: ok"
